// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. CRAFT ELE: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Rotear a fiação de um quadro: os circuitos dele roteados pelos eletrodutos, dimensionados
//! (pré-seleção automática de seção e disjuntor, que o projetista pode sobrescrever) e
//! distribuídos nas fases. É a tabela de resumo da janela Rotear fiação.

use std::collections::HashMap;

use cadcraft_doc::{Drawing, Handle};
use serde::Serialize;

use crate::Analise;
use crate::circuito::Circuito;
use crate::nbr5410::{self, Dimensionamento};
use crate::ponto::{PontoEletrico, TipoPonto, br};
use crate::rota::{self, Rede};
use crate::setup::Setup;

pub const NOMES_FASES: [&str; 3] = ["A", "B", "C"];

/// Chaves do XDATA do quadro com as escolhas feitas à mão: `SECAO.<circuito>` e
/// `DISJ.<circuito>`.
pub const CHAVE_SECAO: &str = "SECAO.";
pub const CHAVE_DISJUNTOR: &str = "DISJ.";

/// O que o projetista escolheu à mão para um circuito.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sobrescrita {
    pub secao: Option<f64>,
    pub disjuntor: Option<f64>,
}

/// As sobrescritas gravadas no quadro, por circuito (em maiúsculas).
pub fn sobrescritas(quadro: &PontoEletrico) -> HashMap<String, Sobrescrita> {
    let mut m: HashMap<String, Sobrescrita> = HashMap::new();
    for (k, v) in &quadro.extras {
        let k = k.to_uppercase();
        let valor = crate::ponto::numero(v);
        if let Some(c) = k.strip_prefix(CHAVE_SECAO) {
            m.entry(c.to_string()).or_default().secao = valor;
        } else if let Some(c) = k.strip_prefix(CHAVE_DISJUNTOR) {
            m.entry(c.to_string()).or_default().disjuntor = valor;
        }
    }
    m
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Linha {
    pub circuito: Circuito,
    /// Fases do quadro que o circuito usa: índices em [`NOMES_FASES`].
    pub fases: Vec<usize>,
    pub fct: f64,
    pub fca: f64,
    /// Circuitos no trecho de eletroduto mais cheio que este circuito usa.
    pub agrupados: usize,
    /// Maior comprimento do quadro a um ponto, com subidas e descidas (m).
    pub comprimento: Option<f64>,
    /// Pontos fora da rede de eletrodutos (comprimento estimado).
    pub fora_da_rede: usize,
    pub queda_admissivel: f64,
    /// A pré-seleção automática.
    pub automatico: Dimensionamento,
    /// O que vale: a pré-seleção com as sobrescritas por cima.
    pub escolha: Dimensionamento,
    pub sobrescrita: Sobrescrita,
    /// Condutores do circuito (fases, neutro, proteção).
    pub condutores: u8,
    /// Cabo do circuito (m): percurso × condutores × fator de materiais.
    pub cabo: f64,
    /// Indicação de fiação, sem a seção quando ela é omitida nas configurações.
    pub indicacao: String,
}

impl Linha {
    pub fn fases_texto(&self) -> String {
        self.fases.iter().map(|f| NOMES_FASES[*f]).collect::<Vec<_>>().join("+")
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoQuadro {
    #[serde(serialize_with = "hex")]
    pub quadro: Handle,
    pub tag: String,
    /// Fases disponíveis no quadro.
    pub fases_quadro: u8,
    pub linhas: Vec<Linha>,
    /// Potência instalada (VA).
    pub potencia_total: f64,
    /// Carga por fase (VA).
    pub carga_por_fase: [f64; 3],
    /// (maior − menor) / maior entre as fases usadas, em %.
    pub desequilibrio: f64,
    /// Cabo por seção (mm², m), com o fator de materiais.
    pub materiais: Vec<(f64, f64)>,
    /// Metros por unidade do desenho.
    pub metros_por_unidade: f64,
    /// Trechos de eletroduto lidos.
    pub trechos_eletroduto: usize,
    pub avisos: Vec<String>,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

/// Os quadros do desenho, na ordem em que aparecem.
pub fn quadros(analise: &Analise) -> Vec<&PontoEletrico> {
    analise.pontos.iter().filter(|p| p.tipo == TipoPonto::Quadro).collect()
}

/// Indicação de fiação: "F+N+T #2,5", "2F+T #4", "3F+N+T #6". A seção sai quando é omitida.
fn indicacao(fases: u8, secao: Option<f64>, setup: &Setup) -> String {
    let condutores = match fases {
        0 | 1 => "F+N+T",
        2 => "2F+T",
        _ => "3F+N+T",
    };
    match secao {
        Some(s) if !setup.omitida(s) => format!("{condutores} #{}", br(s)),
        _ => condutores.to_string(),
    }
}

/// Roteia e dimensiona os circuitos do quadro `quadro`.
pub fn rotear(doc: &Drawing, analise: &Analise, setup: &Setup, quadro: Handle) -> Result<ResumoQuadro, String> {
    let q = analise.pontos.iter().find(|p| p.handle == quadro && p.tipo == TipoPonto::Quadro).ok_or("o objeto não é um quadro do desenho")?;
    let mut avisos = Vec::new();
    let (metros, aviso_unidade) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    avisos.extend(aviso_unidade);
    let cfg_base = setup.config();
    let fases_quadro = q.fases.unwrap_or(cfg_base.fases_padrao).clamp(1, 3);
    let circuitos: Vec<&Circuito> = analise.circuitos.iter().filter(|c| c.quadro == Some(quadro)).collect();

    let rede = Rede::montar(doc, setup, &analise.pontos, metros);
    if rede.trechos == 0 {
        avisos.push(format!("nenhum eletroduto nas camadas \"{}\": comprimentos estimados em L a partir do quadro", setup.camadas_eletroduto));
    } else if !rede.ligacoes.contains_key(&quadro) {
        avisos.push("o quadro não encosta em nenhum eletroduto (raio de captura): comprimentos estimados".into());
    }
    let arvore = rede.ligacoes.get(&quadro).map(|l| rede.arvore(l.no));

    // Roteia todos primeiro: o FCA depende de quantos circuitos dividem cada trecho.
    let rotas: Vec<rota::RotaCircuito> = circuitos
        .iter()
        .map(|c| {
            let membros: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| c.pontos.contains(&p.handle)).collect();
            let (tensao, _) = nbr5410::tensao_circuito(c, &cfg_base);
            let corrente = |p: &PontoEletrico| {
                if p.tipo.e_carga() { nbr5410::corrente_projeto(p.potencia.unwrap_or(0.0), p.tensao.unwrap_or(tensao), c.fases) } else { 0.0 }
            };
            rota::rotear_circuito(&rede, arvore.as_ref(), q, &membros, corrente, setup)
        })
        .collect();
    let mut por_aresta: HashMap<usize, usize> = HashMap::new();
    for r in &rotas {
        for e in &r.arestas {
            *por_aresta.entry(*e).or_default() += 1;
        }
    }
    let manuais = sobrescritas(q);

    let mut linhas: Vec<Linha> = circuitos
        .iter()
        .zip(&rotas)
        .map(|(c, r)| {
            let agrupados = r.arestas.iter().map(|e| por_aresta[e]).max().unwrap_or(1).max(1);
            let cfg = nbr5410::Config { fca: nbr5410::fca_agrupamento(agrupados), ..cfg_base.clone() };
            let automatico = nbr5410::dimensionar_com(c, &cfg, r.momento);
            let sobrescrita = manuais.get(&c.id.to_uppercase()).copied().unwrap_or_default();
            let escolha = match (sobrescrita.secao.or(automatico.secao), sobrescrita.disjuntor.or(automatico.disjuntor)) {
                (Some(s), Some(d)) if sobrescrita != Sobrescrita::default() => nbr5410::verificar(c, &cfg, r.momento, s, d),
                _ => automatico.clone(),
            };
            let condutores = nbr5410::condutores_total(c.fases);
            Linha {
                circuito: (*c).clone(),
                fases: Vec::new(),
                fct: cfg.fct,
                fca: cfg.fca,
                agrupados,
                comprimento: r.comprimento,
                fora_da_rede: r.fora_da_rede.len(),
                queda_admissivel: cfg.queda_admissivel,
                indicacao: indicacao(c.fases, escolha.secao, setup),
                automatico,
                escolha,
                sobrescrita,
                condutores,
                cabo: r.percurso * f64::from(condutores) * setup.fator_materiais,
            }
        })
        .collect();

    // Balanceamento: maiores cargas primeiro, cada uma nas fases menos carregadas.
    let mut carga = [0.0f64; 3];
    let mut ordem: Vec<usize> = (0..linhas.len()).collect();
    ordem.sort_by(|a, b| linhas[*b].circuito.potencia.total_cmp(&linhas[*a].circuito.potencia));
    for i in ordem {
        let pedidas = linhas[i].circuito.fases;
        if pedidas > fases_quadro {
            avisos.push(format!("circuito {} pede {pedidas} fases e o quadro tem {fases_quadro}", linhas[i].circuito.id));
        }
        let n = usize::from(pedidas.min(fases_quadro));
        let mut livres: Vec<usize> = (0..usize::from(fases_quadro)).collect();
        livres.sort_by(|a, b| carga[*a].total_cmp(&carga[*b]));
        let mut escolhidas: Vec<usize> = livres.into_iter().take(n).collect();
        for f in &escolhidas {
            carga[*f] += linhas[i].circuito.potencia / n as f64;
        }
        escolhidas.sort_unstable();
        linhas[i].fases = escolhidas;
    }
    let usadas = &carga[..usize::from(fases_quadro)];
    let maior = usadas.iter().copied().fold(0.0, f64::max);
    let menor = usadas.iter().copied().fold(f64::INFINITY, f64::min);
    let desequilibrio = if maior > 0.0 && fases_quadro > 1 { (maior - menor) / maior * 100.0 } else { 0.0 };

    let mut materiais: Vec<(f64, f64)> = Vec::new();
    for l in &linhas {
        if let Some(s) = l.escolha.secao {
            match materiais.iter_mut().find(|(x, _)| (*x - s).abs() < 1e-6) {
                Some(m) => m.1 += l.cabo,
                None => materiais.push((s, l.cabo)),
            }
        }
    }
    materiais.sort_by(|a, b| a.0.total_cmp(&b.0));
    if circuitos.is_empty() {
        avisos.push("nenhum circuito ligado a este quadro: use Criar circuito".into());
    }

    Ok(ResumoQuadro {
        quadro,
        tag: q.rotulo(),
        fases_quadro,
        potencia_total: linhas.iter().map(|l| l.circuito.potencia).sum(),
        linhas,
        carga_por_fase: carga,
        desequilibrio,
        materiais,
        metros_por_unidade: metros,
        trechos_eletroduto: rede.trechos,
        avisos,
    })
}

/// O resumo técnico em texto, uma linha por circuito.
pub fn texto(r: &ResumoQuadro, setup: &Setup) -> String {
    let mut s =
        format!("RESUMO DA FIAÇÃO - {}\n{} · {} °C (FCT {})\n\n", r.tag, setup.sistema.nome(), br(setup.temperatura_ambiente), br(setup.fct()));
    s.push_str("Circ.  Fases  VA      Ib(A)  FCA   L(m)   dV(%)  Fiação              Disj.\n");
    for l in &r.linhas {
        let e = &l.escolha;
        s.push_str(&format!(
            "{:<6} {:<6} {:<7} {:<6} {:<5} {:<6} {:<6} {:<19} {}\n",
            l.circuito.id,
            l.fases_texto(),
            br(l.circuito.potencia),
            br((e.ib * 10.0).round() / 10.0),
            br(l.fca),
            l.comprimento.map_or("-".into(), |v| br(v.round())),
            e.queda.map_or("-".into(), |v| br((v * 100.0).round() / 100.0)),
            l.indicacao,
            e.disjuntor.map_or("-".into(), |d| format!("{} A", br(d))),
        ));
    }
    s.push_str(&format!("\nPotência instalada: {} VA\n", br(r.potencia_total)));
    if !r.materiais.is_empty() {
        s.push_str(&format!("Cabos (com fator {}):\n", br(setup.fator_materiais)));
        for (secao, m) in &r.materiais {
            s.push_str(&format!("  {} mm²: {} m\n", br(*secao), br(m.ceil())));
        }
    }
    s
}
