// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. CRAFT ELE: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Ajustar fiação: quais condutores de cada circuito passam em cada trecho de eletroduto, e as
//! marcas de fiação desenhadas sobre os trechos.
//!
//! Regras desta versão, por circuito, sobre os caminhos do roteamento:
//! - fase: do quadro às tomadas, aos interruptores e às luminárias sem interruptor;
//! - neutro: do quadro às cargas (não existe em circuito de duas fases sem neutro, 2F+T);
//! - proteção: do quadro às cargas;
//! - retorno: de cada interruptor às luminárias com a letra dele (`COMANDO`), uma por letra.
//!
//! As marcas são blocos (`FIO-FASE`, `FIO-NEUTRO`, `FIO-RETORNO`, `FIO-PROTECAO`) inseridos
//! atravessando o trecho, com o número do circuito (e a seção, quando não omitida) embaixo e as
//! letras dos retornos em cima. Os blocos são criados com um desenho padrão quando o desenho
//! ainda não os tem; o projetista pode redesenhá-los, e Ajustar fiação usa os dele. Tudo o que é
//! gerado fica na camada `ELE-FIACAO` com o XDATA `FIACAO=<handle do quadro>`, e Ajustar fiação
//! apaga e refaz as marcas daquele quadro.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Circle, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Line, RawTag, Space, Text, VAlign};
use serde::Serialize;

use crate::Analise;
use crate::ponto::{PontoEletrico, TipoPonto, br};
use crate::quadro;
use crate::rota::Rede;
use crate::setup::Setup;
use crate::xdata;

/// Camada das marcas de fiação.
pub const CAMADA: &str = "ELE-FIACAO";
/// Chave do XDATA que marca o que Ajustar fiação gerou: `FIACAO=<handle do quadro>`.
pub const CHAVE: &str = "FIACAO";

/// Distância entre marcas no trecho e altura dos textos, em metros.
const PASSO: f64 = 0.04;
const ENTRE_CIRCUITOS: f64 = 0.05;
const ALTURA_TEXTO: f64 = 0.05;
const AFASTAMENTO_TEXTO: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoCondutor {
    Fase,
    Neutro,
    Retorno,
    Protecao,
}

impl TipoCondutor {
    pub const TODOS: [TipoCondutor; 4] = [Self::Fase, Self::Neutro, Self::Retorno, Self::Protecao];

    /// O bloco da marca.
    pub fn bloco(self) -> &'static str {
        match self {
            Self::Fase => "FIO-FASE",
            Self::Neutro => "FIO-NEUTRO",
            Self::Retorno => "FIO-RETORNO",
            Self::Protecao => "FIO-PROTECAO",
        }
    }
}

/// Os condutores de um circuito num trecho.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Condutores {
    pub circuito: String,
    /// Fases (0 a 3).
    pub fases: u8,
    pub neutro: bool,
    pub protecao: bool,
    /// Letras dos retornos.
    pub retornos: BTreeSet<String>,
    /// Seção do circuito (mm²), a escolhida em Rotear fiação.
    pub secao: Option<f64>,
}

impl Condutores {
    /// As marcas, na ordem em que são desenhadas: fases, neutro, retornos, proteção.
    pub fn marcas(&self) -> Vec<(TipoCondutor, Option<&str>)> {
        let mut v: Vec<(TipoCondutor, Option<&str>)> = (0..self.fases).map(|_| (TipoCondutor::Fase, None)).collect();
        if self.neutro {
            v.push((TipoCondutor::Neutro, None));
        }
        v.extend(self.retornos.iter().map(|l| (TipoCondutor::Retorno, Some(l.as_str()))));
        if self.protecao {
            v.push((TipoCondutor::Protecao, None));
        }
        v
    }

    fn vazio(&self) -> bool {
        self.fases == 0 && !self.neutro && !self.protecao && self.retornos.is_empty()
    }
}

/// Um trecho de eletroduto com os condutores que passam nele.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trecho {
    /// Pontas, nas coordenadas do desenho.
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub piso: bool,
    pub circuitos: Vec<Condutores>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fiacao {
    #[serde(serialize_with = "hex")]
    pub quadro: Handle,
    pub tag: String,
    pub trechos: Vec<Trecho>,
    /// Metros por unidade do desenho.
    pub metros_por_unidade: f64,
    pub avisos: Vec<String>,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

/// As letras de um COMANDO: "a", "ab", "a,b" ou "a b".
fn letras(comando: &str) -> Vec<String> {
    comando.chars().filter(|c| c.is_alphabetic()).map(|c| c.to_lowercase().to_string()).collect()
}

/// Calcula os condutores em cada trecho dos circuitos do quadro.
pub fn calcular(doc: &Drawing, analise: &Analise, setup: &Setup, quadro: Handle) -> Result<Fiacao, String> {
    let resumo = quadro::rotear(doc, analise, setup, quadro)?;
    let q = analise.ponto(quadro).ok_or("quadro não encontrado")?;
    let rede = Rede::montar(doc, setup, &analise.pontos, resumo.metros_por_unidade);
    let lig_q = rede.ligacoes.get(&quadro).ok_or("o quadro não encosta em nenhum eletroduto (raio de captura): não há onde marcar a fiação")?;
    let arvore_q = rede.arvore(lig_q.no);
    let mut avisos = Vec::new();

    // aresta -> circuito -> condutores
    let mut por_aresta: BTreeMap<usize, BTreeMap<usize, Condutores>> = BTreeMap::new();
    for (ic, linha) in resumo.linhas.iter().enumerate() {
        let c = &linha.circuito;
        let membros: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| c.pontos.contains(&p.handle)).collect();
        let novo = || Condutores { circuito: c.id.clone(), secao: linha.escolha.secao, ..Condutores::default() };
        let mut marcar = |arestas: &[usize], f: &dyn Fn(&mut Condutores)| {
            for e in arestas {
                f(por_aresta.entry(*e).or_default().entry(ic).or_insert_with(novo));
            }
        };
        let caminho_do_quadro = |p: &PontoEletrico| rede.ligacoes.get(&p.handle).and_then(|l| arvore_q.caminho(l.no));
        let interruptores: Vec<&&PontoEletrico> = membros.iter().filter(|p| p.tipo == TipoPonto::Interruptor).collect();
        let letras_com_interruptor: BTreeSet<String> = interruptores.iter().filter_map(|s| s.comando.as_deref()).flat_map(letras).collect();
        let fases = c.fases.max(1);
        for p in &membros {
            if matches!(p.tipo, TipoPonto::Caixa | TipoPonto::Quadro) {
                continue;
            }
            let Some(caminho) = caminho_do_quadro(p) else {
                avisos.push(format!("{}: fora da rede de eletrodutos, fiação não marcada", p.rotulo()));
                continue;
            };
            let comandada = p.tipo == TipoPonto::Iluminacao
                && fases < 2
                && p.comando.as_deref().is_some_and(|cmd| letras(cmd).iter().any(|l| letras_com_interruptor.contains(l)));
            match p.tipo {
                TipoPonto::Interruptor => marcar(&caminho, &|k: &mut Condutores| k.fases = k.fases.max(1)),
                _ if comandada => marcar(&caminho, &|k: &mut Condutores| {
                    k.neutro = true;
                    k.protecao = true;
                }),
                _ => marcar(&caminho, &|k: &mut Condutores| {
                    k.fases = k.fases.max(fases);
                    k.neutro |= fases != 2;
                    k.protecao = true;
                }),
            }
        }
        // Retornos: do interruptor a cada luminária com a letra dele.
        for s in &interruptores {
            let Some(lig_s) = rede.ligacoes.get(&s.handle) else { continue };
            let arvore_s = rede.arvore(lig_s.no);
            for letra in s.comando.as_deref().map(letras).unwrap_or_default() {
                for lum in
                    membros.iter().filter(|p| p.tipo == TipoPonto::Iluminacao && p.comando.as_deref().is_some_and(|cmd| letras(cmd).contains(&letra)))
                {
                    let caminho = rede.ligacoes.get(&lum.handle).and_then(|l| arvore_s.caminho(l.no));
                    match caminho {
                        Some(cam) => marcar(&cam, &|k: &mut Condutores| {
                            k.retornos.insert(letra.clone());
                        }),
                        None => avisos.push(format!("{}: sem caminho de eletroduto até o interruptor do comando {letra}", lum.rotulo())),
                    }
                }
            }
        }
    }

    let trechos = por_aresta
        .into_iter()
        .filter_map(|(e, circuitos)| {
            let (na, nb, _) = *rede.arestas.get(e)?;
            let (a, b) = (*rede.nos.get(na)?, *rede.nos.get(nb)?);
            // Subidas entre piso e teto não têm desenho para marcar.
            if (a[0] - b[0]).hypot(a[1] - b[1]) <= 0.0 {
                return None;
            }
            let piso = rede.elevacao.get(na).is_some_and(|z| *z < setup.pe_direito);
            let circuitos: Vec<Condutores> = circuitos.into_values().filter(|k| !k.vazio()).collect();
            (!circuitos.is_empty()).then_some(Trecho { a, b, piso, circuitos })
        })
        .collect();
    Ok(Fiacao { quadro, tag: q.rotulo(), trechos, metros_por_unidade: resumo.metros_por_unidade, avisos })
}

// ---------------------------------------------------------------- desenho

fn xdata_fiacao(quadro: Handle) -> Vec<RawTag> {
    let mut x = Vec::new();
    xdata::gravar(&mut x, CHAVE, Some(&quadro.hex()));
    x
}

fn comum(quadro: Handle) -> Common {
    Common { layer: CAMADA.into(), xdata: xdata_fiacao(quadro), ..Common::default() }
}

/// O que Ajustar fiação gerou para o quadro.
pub fn gerados(doc: &Drawing, quadro: Handle) -> Vec<Handle> {
    let alvo = quadro.hex();
    doc.model
        .iter()
        .filter(|e| xdata::ler(&e.common.xdata).iter().any(|(k, v)| k.eq_ignore_ascii_case(CHAVE) && v.eq_ignore_ascii_case(&alvo)))
        .map(|e| e.handle)
        .collect()
}

/// Apaga as marcas de fiação do quadro. Devolve quantas apagou.
pub fn limpar(doc: &mut Drawing, quadro: Handle) -> usize {
    let hs = gerados(doc, quadro);
    hs.iter().filter(|h| doc.model.remove(**h).is_some()).count()
}

/// Cria os blocos das marcas que o desenho ainda não tem (traço inclinado atravessando o
/// eletroduto; o neutro com um círculo na ponta, o retorno com uma bandeira, a proteção com um
/// traço cruzado). `m` = metros por unidade do desenho.
pub fn garantir_blocos(doc: &mut Drawing, m: f64) {
    let u = |v: f64| v / m;
    let linha = |a: (f64, f64), b: (f64, f64)| EntityKind::Line(Line { a: Vec3::new(u(a.0), u(a.1), 0.0), b: Vec3::new(u(b.0), u(b.1), 0.0) });
    let traco = linha((-0.012, -0.05), (0.012, 0.05));
    for tipo in TipoCondutor::TODOS {
        if doc.blocks.contains_key(tipo.bloco()) {
            continue;
        }
        let mut formas = vec![traco.clone()];
        match tipo {
            TipoCondutor::Fase => {}
            TipoCondutor::Neutro => formas.push(EntityKind::Circle(Circle { center: Vec3::new(u(0.012), u(0.06), 0.0), radius: u(0.01) })),
            TipoCondutor::Retorno => formas.push(linha((0.012, 0.05), (0.032, 0.035))),
            TipoCondutor::Protecao => formas.push(linha((-0.012, 0.03), (0.024, 0.03))),
        }
        let mut b = Block::new(tipo.bloco());
        for k in formas {
            let h = doc.new_handle();
            b.entities.push(Entity::new(h, k));
        }
        doc.blocks.insert(tipo.bloco().to_string(), Arc::new(b));
    }
    if doc.ensure_layer(CAMADA)
        && let Some(l) = doc.layer_mut(CAMADA)
    {
        l.color = Color::Index(1);
    }
}

fn texto(valor: String, em: [f64; 2], altura: f64, rotacao: f64) -> EntityKind {
    let p = Vec3::new(em[0], em[1], 0.0);
    EntityKind::Text(Text {
        insert: p,
        align_pt: Some(p),
        height: altura,
        value: valor,
        rotation: rotacao,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign: HAlign::Center,
        valign: VAlign::Middle,
    })
}

/// Desenha as marcas da fiação (apagando as anteriores do mesmo quadro). Devolve quantas
/// entidades criou.
pub fn aplicar(doc: &mut Drawing, f: &Fiacao, setup: &Setup) -> Result<usize, String> {
    let m = if f.metros_por_unidade > 0.0 { f.metros_por_unidade } else { 1.0 };
    limpar(doc, f.quadro);
    garantir_blocos(doc, m);
    let mut criadas = 0usize;
    for t in &f.trechos {
        // Sempre da esquerda para a direita (ou de baixo para cima), para o texto ler direito.
        let (a, b) = if t.b[0] < t.a[0] || (t.b[0] == t.a[0] && t.b[1] < t.a[1]) { (t.b, t.a) } else { (t.a, t.b) };
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let comp = dx.hypot(dy);
        if comp <= 0.0 {
            continue;
        }
        let (d, n) = ([dx / comp, dy / comp], [-dy / comp, dx / comp]);
        let rot = dy.atan2(dx);
        let meio = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let (passo, entre) = (PASSO / m, ENTRE_CIRCUITOS / m);
        let grupos: Vec<Vec<(TipoCondutor, Option<&str>)>> = t.circuitos.iter().map(Condutores::marcas).collect();
        let largura: f64 =
            grupos.iter().map(|g| (g.len().saturating_sub(1)) as f64 * passo).sum::<f64>() + grupos.len().saturating_sub(1) as f64 * entre;
        let mut s = -largura / 2.0;
        for (k, grupo) in t.circuitos.iter().zip(&grupos) {
            let inicio = s;
            for (tipo, letra) in grupo {
                let p = [meio[0] + d[0] * s, meio[1] + d[1] * s];
                let ins = Insert {
                    block: tipo.bloco().into(),
                    insert: Vec3::new(p[0], p[1], 0.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                    rotation: rot,
                    attribs: Vec::new(),
                    cols: 1,
                    rows: 1,
                    col_spacing: 0.0,
                    row_spacing: 0.0,
                };
                doc.add(&Space::Model, comum(f.quadro), EntityKind::Insert(ins)).map_err(|e| e.to_string())?;
                criadas += 1;
                if let Some(l) = letra {
                    let em = [p[0] + n[0] * AFASTAMENTO_TEXTO / m, p[1] + n[1] * AFASTAMENTO_TEXTO / m];
                    doc.add(&Space::Model, comum(f.quadro), texto(l.to_string(), em, ALTURA_TEXTO / m, rot)).map_err(|e| e.to_string())?;
                    criadas += 1;
                }
                s += passo;
            }
            // Rótulo do circuito embaixo do grupo: número e seção (quando não omitida).
            let centro = (inicio + s - passo) / 2.0;
            let secao = k.secao.filter(|x| !setup.omitida(*x)).map(|x| format!(" #{}", br(x))).unwrap_or_default();
            let em = [meio[0] + d[0] * centro - n[0] * AFASTAMENTO_TEXTO / m, meio[1] + d[1] * centro - n[1] * AFASTAMENTO_TEXTO / m];
            doc.add(&Space::Model, comum(f.quadro), texto(format!("{}{secao}", k.circuito), em, ALTURA_TEXTO / m, rot)).map_err(|e| e.to_string())?;
            criadas += 1;
            s += entre - passo;
        }
    }
    Ok(criadas)
}

/// Resumo por trecho para o JSON dos comandos: quantas marcas de cada tipo.
pub fn contagem(f: &Fiacao) -> HashMap<&'static str, usize> {
    let mut c: HashMap<&'static str, usize> = HashMap::new();
    for t in &f.trechos {
        for k in &t.circuitos {
            for (tipo, _) in k.marcas() {
                *c.entry(tipo.bloco()).or_default() += 1;
            }
        }
    }
    c
}
