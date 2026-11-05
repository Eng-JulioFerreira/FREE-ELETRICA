// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Roteamento: o grafo estrito da instalação, o caminho do quadro até cada ponto e, dele,
//! comprimentos 3D e momentos elétricos para a queda de tensão.
//!
//! **Regra do grafo:** os nós são só blocos elétricos (quadro, tomada, luminária, interruptor,
//! caixa). Cada eletroduto é uma aresta que liga exatamente dois blocos: os das pontas dele.
//! Duas linhas que se cruzam em X ou em T sem um bloco ali **não** se ligam, e um bloco encostado
//! no meio de um eletroduto também não: para derivar, é preciso uma caixa (ELEELETRODUTO divide o
//! trecho e insere a caixa).
//!
//! Comprimento de um trecho: 2D + |cota do eletroduto − Z do bloco A| + |cota − Z do bloco B|.
//! A cota vem da instalação: teto (pé-direito), piso (0) ou parede (na altura do bloco A).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use cadcraft_doc::geom::{Arc as GArc, Polyline, Vec2};
use cadcraft_doc::{Drawing, EntityKind, Handle};
use serde::{Deserialize, Serialize};

use crate::ponto::{PontoEletrico, TipoPonto};
use crate::setup::Setup;
use crate::xdata;

type P = [f64; 2];

/// Chaves do XDATA de um eletroduto (versão 2 do esquema).
pub const CHAVE_DE: &str = "from_handle";
pub const CHAVE_PARA: &str = "to_handle";
pub const CHAVE_INSTALACAO: &str = "install_elevation";
pub const CHAVE_DIAMETRO: &str = "nominal_diameter_mm";
pub const CHAVE_TRAVA_DIAMETRO: &str = "manual_diameter_lock";
pub const CHAVE_FIOS: &str = "passing_wires";
pub const CHAVE_TAG: &str = "tag_handle";
pub const CHAVE_TIPO: &str = "conduit_type";
/// O perfil de eletroduto do trecho, pelo nome: dele vem a tabela de diâmetros.
pub const CHAVE_PERFIL: &str = "conduit_profile";
pub const CHAVE_ESQUEMA: &str = "schema_ver";
pub const VERSAO_ESQUEMA: &str = "2";

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Onde o trecho horizontal do eletroduto corre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Instalacao {
    #[default]
    Teto,
    Piso,
    Parede,
}

impl Instalacao {
    pub const TODAS: [Instalacao; 3] = [Self::Teto, Self::Piso, Self::Parede];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Teto => "Teto",
            Self::Piso => "Piso",
            Self::Parede => "Parede",
        }
    }

    /// O código gravado no XDATA.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Teto => "CEILING",
            Self::Piso => "FLOOR",
            Self::Parede => "WALL",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim();
        Self::TODAS.into_iter().find(|i| i.codigo().eq_ignore_ascii_case(t) || i.nome().eq_ignore_ascii_case(t))
    }
}

/// O tipo de um eletroduto: entra na ocupação, nos materiais e nas propriedades.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoEletroduto {
    CorrugadoLeve,
    CorrugadoReforcado,
    /// O tipo dos eletrodutos desenhados antes de existir esta escolha.
    #[default]
    PvcRigido,
    PeadCorrugado,
    AcoEletrolitico,
    AcoFogo,
}

impl TipoEletroduto {
    pub const TODOS: [TipoEletroduto; 6] =
        [Self::CorrugadoLeve, Self::CorrugadoReforcado, Self::PvcRigido, Self::PeadCorrugado, Self::AcoEletrolitico, Self::AcoFogo];

    /// O código gravado no XDATA.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::CorrugadoLeve => "PVC_CORR_LIGHT",
            Self::CorrugadoReforcado => "PVC_CORR_HEAVY",
            Self::PvcRigido => "PVC_RIGID",
            Self::PeadCorrugado => "HDPE_CORR",
            Self::AcoEletrolitico => "STEEL_ELECTRO",
            Self::AcoFogo => "STEEL_HOT_DIP",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::CorrugadoLeve => "PVC flexível corrugado leve (amarelo, parede)",
            Self::CorrugadoReforcado => "PVC flexível corrugado reforçado (laranja, laje)",
            Self::PvcRigido => "PVC rígido (roscável ou soldável)",
            Self::PeadCorrugado => "PEAD flexível corrugado (subterrâneo, piso)",
            Self::AcoEletrolitico => "Aço galvanizado eletrolítico (médio, leve)",
            Self::AcoFogo => "Aço galvanizado a fogo (pesado)",
        }
    }

    /// O nome curto, para tabelas e propriedades.
    pub fn abreviado(self) -> &'static str {
        match self {
            Self::CorrugadoLeve => "PVC-CL",
            Self::CorrugadoReforcado => "PVC-CR",
            Self::PvcRigido => "PVC-R",
            Self::PeadCorrugado => "PEAD",
            Self::AcoEletrolitico => "AG-E",
            Self::AcoFogo => "AG-F",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim();
        Self::TODOS.into_iter().find(|i| i.codigo().eq_ignore_ascii_case(t) || i.abreviado().eq_ignore_ascii_case(t))
    }

    /// (nominal, polegadas, interno) em mm: valores típicos de catálogo, a conferir com o
    /// fabricante. `None`: o PVC rígido usa a tabela editável das configurações.
    pub fn tamanhos_tipicos(self) -> Option<&'static [(f64, &'static str, f64)]> {
        Some(match self {
            Self::PvcRigido => return None,
            Self::CorrugadoLeve => &[(20.0, "1/2\"", 15.4), (25.0, "3/4\"", 19.0), (32.0, "1\"", 25.0)],
            Self::CorrugadoReforcado => &[(20.0, "1/2\"", 15.0), (25.0, "3/4\"", 18.6), (32.0, "1\"", 24.6)],
            Self::PeadCorrugado => &[
                (40.0, "1 1/4\"", 32.0),
                (50.0, "1 1/2\"", 40.0),
                (63.0, "2\"", 50.0),
                (75.0, "2 1/2\"", 61.0),
                (90.0, "3\"", 75.0),
                (110.0, "4\"", 93.0),
            ],
            Self::AcoEletrolitico => &[
                (20.0, "1/2\"", 16.0),
                (25.0, "3/4\"", 21.0),
                (32.0, "1\"", 27.0),
                (40.0, "1 1/4\"", 35.5),
                (50.0, "1 1/2\"", 41.0),
                (60.0, "2\"", 52.5),
            ],
            Self::AcoFogo => &[
                (20.0, "1/2\"", 15.8),
                (25.0, "3/4\"", 20.9),
                (32.0, "1\"", 26.6),
                (40.0, "1 1/4\"", 35.1),
                (50.0, "1 1/2\"", 40.9),
                (60.0, "2\"", 52.5),
                (75.0, "2 1/2\"", 62.7),
                (85.0, "3\"", 77.9),
            ],
        })
    }
}

/// Um nó do grafo: um bloco elétrico.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct No {
    #[serde(serialize_with = "hex")]
    pub handle: Handle,
    pub tipo: TipoPonto,
    pub pos: P,
    /// Altura de instalação do bloco (m).
    pub z: f64,
}

/// Uma aresta do grafo: um eletroduto entre dois blocos.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aresta {
    #[serde(serialize_with = "hex")]
    pub eletroduto: Handle,
    pub de: usize,
    pub para: usize,
    pub instalacao: Instalacao,
    pub tipo: TipoEletroduto,
    /// O perfil de eletroduto do trecho.
    pub perfil: Option<String>,
    /// Comprimento em planta (m).
    pub comprimento_2d: f64,
    /// Comprimento com as subidas e descidas até os blocos (m).
    pub comprimento: f64,
    /// Meio do eletroduto (coordenadas do desenho) e o ângulo dele ali (radianos).
    pub meio: P,
    pub angulo: f64,
}

/// Um eletroduto que não liga dois blocos: fica fora do grafo e é apontado na conferência.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solto {
    #[serde(serialize_with = "hex")]
    pub eletroduto: Handle,
    pub motivo: String,
    pub meio: P,
    pub angulo: f64,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rede {
    pub nos: Vec<No>,
    pub arestas: Vec<Aresta>,
    pub soltos: Vec<Solto>,
    /// Metros por unidade do desenho.
    pub metros: f64,
    #[serde(skip)]
    adj: Vec<Vec<(usize, usize)>>,
    #[serde(skip)]
    indice: HashMap<Handle, usize>,
}

/// A curva de um eletroduto em pedaços retos. `None`: a entidade não é um traçado aberto.
/// `tol` é o desvio máximo, nas unidades do desenho.
pub fn tracado(kind: &EntityKind, tol: f64) -> Option<Vec<P>> {
    let v: Vec<Vec2> = match kind {
        EntityKind::Line(l) => vec![l.a.xy(), l.b.xy()],
        EntityKind::LwPolyline(pl) if !pl.closed => Polyline { vertices: pl.vertices.clone(), closed: false }.tessellate(tol),
        EntityKind::Arc(a) => {
            let mut out = Vec::new();
            GArc::new(a.center.xy(), a.radius, a.start, a.end).tessellate(tol, &mut out);
            out
        }
        EntityKind::Spline(sp) if !sp.closed => sp.tessellate(tol),
        _ => return None,
    };
    let p: Vec<P> = v.iter().filter(|q| q.x.is_finite() && q.y.is_finite()).map(|q| [q.x, q.y]).collect();
    (p.len() >= 2).then_some(p)
}

/// Comprimento de um traçado.
pub fn comprimento_tracado(p: &[P]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// O meio de um traçado (pelo comprimento) e o ângulo do pedaço onde ele cai.
pub fn meio_do_tracado(p: &[P]) -> (P, f64) {
    let metade = comprimento_tracado(p) / 2.0;
    let mut andado = 0.0;
    for w in p.windows(2) {
        let c = dist(w[0], w[1]);
        if c > 0.0 && andado + c >= metade {
            let t = (metade - andado) / c;
            return ([w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t], (w[1][1] - w[0][1]).atan2(w[1][0] - w[0][0]));
        }
        andado += c;
    }
    (p.first().copied().unwrap_or([0.0, 0.0]), 0.0)
}

/// O ponto do traçado mais perto de `q`: (ponto, distância, índice do pedaço).
pub fn perto_do_tracado(p: &[P], q: P) -> Option<(P, f64, usize)> {
    p.windows(2)
        .enumerate()
        .map(|(i, w)| {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let l2 = dx * dx + dy * dy;
            let t = if l2 <= 0.0 { 0.0 } else { (((q[0] - w[0][0]) * dx + (q[1] - w[0][1]) * dy) / l2).clamp(0.0, 1.0) };
            let f = [w[0][0] + t * dx, w[0][1] + t * dy];
            (f, dist(f, q), i)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Altura de instalação do ponto (m).
pub fn altura(p: &PontoEletrico, setup: &Setup) -> f64 {
    p.altura.unwrap_or_else(|| p.altura_legenda(setup))
}

/// A instalação de um eletroduto: a gravada nele ou, sem ela, a da camada.
pub fn instalacao_de(xd: &[(String, String)], camada: &str, setup: &Setup) -> Instalacao {
    let gravada = xd.iter().find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_INSTALACAO)).and_then(|(_, v)| Instalacao::de_codigo(v));
    gravada.unwrap_or_else(|| {
        if setup.e_camada_piso(camada) {
            Instalacao::Piso
        } else if camada.to_uppercase().contains("PAREDE") {
            Instalacao::Parede
        } else {
            Instalacao::Teto
        }
    })
}

impl Rede {
    /// Monta o grafo: um nó por ponto elétrico e uma aresta por eletroduto cujas duas pontas
    /// caem em blocos (o gravado em `from_handle`/`to_handle`, se a ponta encosta nele, ou o bloco
    /// mais perto dentro do raio de captura). O meio de um eletroduto nunca liga nada.
    pub fn montar(doc: &Drawing, setup: &Setup, pontos: &[PontoEletrico], metros: f64) -> Rede {
        let metros = if metros > 0.0 { metros } else { 1.0 };
        let raio = setup.raio_captura / metros;
        let mut rede = Rede { metros, ..Rede::default() };
        for p in pontos {
            rede.indice.insert(p.handle, rede.nos.len());
            rede.nos.push(No { handle: p.handle, tipo: p.tipo, pos: p.posicao, z: altura(p, setup) });
        }
        rede.adj = vec![Vec::new(); rede.nos.len()];
        for e in doc.model.iter() {
            if !setup.e_camada_eletroduto(&e.common.layer) {
                continue;
            }
            let Some(tr) = tracado(&e.kind, 0.005 / metros) else { continue };
            let (meio, angulo) = meio_do_tracado(&tr);
            let xd = xdata::ler(&e.common.xdata);
            let gravado = |k: &str| {
                xd.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).and_then(|(_, v)| Handle::parse_hex(v)).and_then(|h| rede.indice.get(&h).copied())
            };
            let gravados = [gravado(CHAVE_DE), gravado(CHAVE_PARA)];
            let ponta = |q: P| -> Option<usize> {
                let encosta = |n: &usize| rede.nos.get(*n).is_some_and(|no| dist(no.pos, q) <= raio);
                gravados
                    .iter()
                    .flatten()
                    .filter(|n| encosta(n))
                    .min_by(|a, b| dist(rede.nos[**a].pos, q).total_cmp(&dist(rede.nos[**b].pos, q)))
                    .copied()
                    .or_else(|| {
                        rede.nos
                            .iter()
                            .enumerate()
                            .map(|(i, no)| (i, dist(no.pos, q)))
                            .filter(|(_, d)| *d <= raio)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(i, _)| i)
                    })
            };
            let (Some(a), Some(b)) = (tr.first().copied(), tr.last().copied()) else { continue };
            let solto = |motivo: &str| Solto { eletroduto: e.handle, motivo: motivo.to_string(), meio, angulo };
            match (ponta(a), ponta(b)) {
                (Some(de), Some(para)) if de != para => {
                    let instalacao = instalacao_de(&xd, &e.common.layer, setup);
                    let tipo = xd
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_TIPO))
                        .and_then(|(_, v)| TipoEletroduto::de_codigo(v))
                        .unwrap_or_default();
                    let perfil = xd.iter().find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_PERFIL)).map(|(_, v)| v.clone());
                    let (za, zb) = (rede.nos[de].z, rede.nos[para].z);
                    let cota = match instalacao {
                        Instalacao::Teto => setup.pe_direito,
                        Instalacao::Piso => 0.0,
                        Instalacao::Parede => za,
                    };
                    let comprimento_2d = comprimento_tracado(&tr) * metros;
                    let comprimento = comprimento_2d + (cota - za).abs() + (cota - zb).abs();
                    let i = rede.arestas.len();
                    rede.arestas.push(Aresta { eletroduto: e.handle, de, para, instalacao, tipo, perfil, comprimento_2d, comprimento, meio, angulo });
                    rede.adj[de].push((para, i));
                    rede.adj[para].push((de, i));
                }
                (Some(_), Some(_)) => rede.soltos.push(solto("as duas pontas caem no mesmo bloco")),
                (None, None) => rede.soltos.push(solto("as pontas não encostam em nenhum bloco")),
                _ => rede.soltos.push(solto("uma das pontas não encosta em nenhum bloco")),
            }
        }
        rede
    }

    /// O nó de um ponto.
    pub fn no_de(&self, h: Handle) -> Option<usize> {
        self.indice.get(&h).copied()
    }

    /// O bloco ligou-se a algum eletroduto.
    pub fn conectado(&self, h: Handle) -> bool {
        self.no_de(h).is_some_and(|n| self.adj.get(n).is_some_and(|a| !a.is_empty()))
    }

    /// Caminhos mínimos a partir de um nó (Dijkstra): distância (m) e a aresta de chegada.
    pub fn arvore(&self, origem: usize) -> Arvore {
        #[derive(PartialEq)]
        struct Item(f64, usize);
        impl Eq for Item {}
        impl Ord for Item {
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.total_cmp(&self.0)
            }
        }
        impl PartialOrd for Item {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        let mut distancia = vec![f64::INFINITY; self.nos.len()];
        let mut chegada: Vec<Option<(usize, usize)>> = vec![None; self.nos.len()];
        let mut fila = BinaryHeap::new();
        if origem < self.nos.len() {
            distancia[origem] = 0.0;
            fila.push(Item(0.0, origem));
        }
        while let Some(Item(d, n)) = fila.pop() {
            if d > distancia[n] {
                continue;
            }
            for &(viz, e) in &self.adj[n] {
                let nd = d + self.arestas[e].comprimento;
                if nd < distancia[viz] {
                    distancia[viz] = nd;
                    chegada[viz] = Some((n, e));
                    fila.push(Item(nd, viz));
                }
            }
        }
        Arvore { distancia, chegada }
    }
}

pub struct Arvore {
    pub distancia: Vec<f64>,
    chegada: Vec<Option<(usize, usize)>>,
}

impl Arvore {
    /// As arestas do caminho da origem até `no`. `None`: não há caminho.
    pub fn caminho(&self, mut no: usize) -> Option<Vec<usize>> {
        if !self.distancia.get(no)?.is_finite() {
            return None;
        }
        let mut arestas = Vec::new();
        while let Some((anterior, e)) = self.chegada.get(no).copied().flatten() {
            arestas.push(e);
            no = anterior;
        }
        Some(arestas)
    }
}

/// O que o roteamento diz de um circuito.
#[derive(Clone, Debug, Default)]
pub struct RotaCircuito {
    /// Maior comprimento do quadro até um ponto (m).
    pub comprimento: Option<f64>,
    /// Maior momento elétrico até um ponto de carga (A·m).
    pub momento: Option<f64>,
    /// Comprimento do eletroduto ocupado pelo circuito (m), para o levantamento de cabo.
    pub percurso: f64,
    /// Arestas que o circuito ocupa.
    pub arestas: HashSet<usize>,
    /// Pontos sem caminho até o quadro: o comprimento deles é estimado (distância de Manhattan 3D).
    pub fora_da_rede: Vec<Handle>,
}

/// Distância de Manhattan 3D entre o quadro e um ponto (m): ΔX + ΔY + ΔZ.
pub fn manhattan_3d(a: P, za: f64, b: P, zb: f64, metros: f64) -> f64 {
    ((a[0] - b[0]).abs() + (a[1] - b[1]).abs()) * metros + (za - zb).abs()
}

/// Roteia os pontos de um circuito a partir do quadro. `corrente` dá a corrente de cada ponto.
pub fn rotear_circuito(
    rede: &Rede,
    arvore: Option<&Arvore>,
    quadro: &PontoEletrico,
    membros: &[&PontoEletrico],
    corrente: impl Fn(&PontoEletrico) -> f64,
    setup: &Setup,
) -> RotaCircuito {
    let z_quadro = altura(quadro, setup);
    let total: f64 = membros.iter().map(|p| corrente(p)).sum();
    let mut rota = RotaCircuito::default();
    let mut descida: HashMap<usize, f64> = HashMap::new();
    let mut roteados: Vec<(&PontoEletrico, Vec<usize>)> = Vec::new();
    let mut estimados: Vec<(&PontoEletrico, f64)> = Vec::new();
    for p in membros {
        let caminho = match (arvore, rede.no_de(p.handle)) {
            (Some(a), Some(n)) if a.distancia.get(n).is_some_and(|d| d.is_finite()) => a.caminho(n).map(|c| (c, a.distancia[n])),
            _ => None,
        };
        match caminho {
            Some((c, d)) => {
                for e in &c {
                    *descida.entry(*e).or_default() += corrente(p);
                }
                rota.comprimento = Some(rota.comprimento.unwrap_or(0.0).max(d));
                rota.arestas.extend(c.iter().copied());
                roteados.push((p, c));
            }
            None => {
                let l = manhattan_3d(p.posicao, altura(p, setup), quadro.posicao, z_quadro, rede.metros);
                rota.comprimento = Some(rota.comprimento.unwrap_or(0.0).max(l));
                rota.percurso += l;
                rota.fora_da_rede.push(p.handle);
                estimados.push((p, l));
            }
        }
    }
    rota.percurso += rota.arestas.iter().filter_map(|e| rede.arestas.get(*e)).map(|a| a.comprimento).sum::<f64>();
    // Momento até cada carga: cada trecho leva a corrente de tudo o que está depois dele.
    for (p, c) in &roteados {
        if corrente(p) <= 0.0 {
            continue;
        }
        let m = c.iter().filter_map(|e| Some(descida.get(e)? * rede.arestas.get(*e)?.comprimento)).sum::<f64>();
        rota.momento = Some(rota.momento.unwrap_or(0.0).max(m));
    }
    // Sem caminho: toda a corrente do circuito no comprimento estimado (lado seguro).
    for (p, l) in &estimados {
        if corrente(p) > 0.0 {
            rota.momento = Some(rota.momento.unwrap_or(0.0).max(total * l));
        }
    }
    rota
}
