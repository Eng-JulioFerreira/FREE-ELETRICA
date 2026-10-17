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
use crate::demanda::{self, Demanda, TipoCarga};
use crate::nbr5410::{self, Dimensionamento};
use crate::ponto::{PontoEletrico, TipoPonto, br};
use crate::rota::{self, Rede};
use crate::setup::Setup;

pub const NOMES_FASES: [&str; 3] = ["A", "B", "C"];

/// Chaves do XDATA do quadro com as escolhas feitas à mão: `SECAO.<circuito>` e
/// `DISJ.<circuito>`.
pub const CHAVE_SECAO: &str = "SECAO.";
pub const CHAVE_DISJUNTOR: &str = "DISJ.";
/// Descrição de um circuito no quadro de cargas: `DESC.<circuito>`. Um circuito descrito
/// existe no quadro mesmo sem pontos (criado no quadro de cargas).
pub const CHAVE_DESCRICAO: &str = "DESC.";
/// Isolação dos condutores do quadro (`PVC`, `EPR`, `XLPE`); sem ela vale a das configurações.
pub const CHAVE_ISOLACAO: &str = "ISOLACAO";
/// Isolação de um circuito: `ISOL.<circuito>`; sem ela vale a do quadro.
pub const CHAVE_ISOLACAO_CIRCUITO: &str = "ISOL.";
/// Tipo de carga de um circuito no cálculo de demanda: `CARGA.<circuito>`.
pub const CHAVE_CARGA: &str = "CARGA.";
/// Fator de potência de um circuito: `FP.<circuito>`.
pub const CHAVE_FP: &str = "FP.";

/// O que o projetista definiu para um circuito no quadro de cargas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Propriedades {
    pub isolacao: Option<nbr5410::Isolacao>,
    pub carga: Option<TipoCarga>,
    pub fp: Option<f64>,
}

/// As propriedades gravadas no quadro, por circuito (em maiúsculas).
pub fn propriedades(quadro: &PontoEletrico) -> HashMap<String, Propriedades> {
    let mut m: HashMap<String, Propriedades> = HashMap::new();
    for (k, v) in &quadro.extras {
        let k = k.to_uppercase();
        if let Some(c) = k.strip_prefix(CHAVE_ISOLACAO_CIRCUITO) {
            m.entry(c.to_string()).or_default().isolacao = nbr5410::Isolacao::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_CARGA) {
            m.entry(c.to_string()).or_default().carga = TipoCarga::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_FP) {
            m.entry(c.to_string()).or_default().fp = crate::ponto::numero(v).filter(|f| *f > 0.0 && *f <= 1.0);
        }
    }
    m
}

/// A isolação do quadro: a gravada nele ou a padrão das configurações.
pub fn isolacao_do_quadro(quadro: &PontoEletrico, setup: &Setup) -> nbr5410::Isolacao {
    quadro
        .extras
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_ISOLACAO))
        .and_then(|(_, v)| nbr5410::Isolacao::de_codigo(v))
        .unwrap_or(setup.isolacao)
}

/// As descrições de circuitos gravadas no quadro: (circuito, descrição), na ordem natural.
pub fn descricoes(quadro: &PontoEletrico) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> =
        quadro.extras.iter().filter_map(|(k, d)| Some((k.to_uppercase().strip_prefix(CHAVE_DESCRICAO)?.to_string(), d.clone()))).collect();
    v.sort_by(|a, b| crate::circuito::ordem_natural(&a.0, &b.0));
    v
}

/// Um circuito no quadro de cargas: os que têm pontos e os criados (descritos) ainda sem pontos.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitoDoQuadro {
    pub id: String,
    pub descricao: String,
    pub circuito: Option<Circuito>,
}

/// Os circuitos do quadro, na ordem natural.
pub fn circuitos_do_quadro(analise: &Analise, quadro: Handle) -> Vec<CircuitoDoQuadro> {
    let descr = analise.ponto(quadro).map(descricoes).unwrap_or_default();
    let mut v: Vec<CircuitoDoQuadro> = analise
        .circuitos
        .iter()
        .filter(|c| c.quadro == Some(quadro))
        .map(|c| CircuitoDoQuadro {
            id: c.id.clone(),
            descricao: descr.iter().find(|(id, _)| id.eq_ignore_ascii_case(&c.id)).map(|(_, d)| d.clone()).unwrap_or_default(),
            circuito: Some(c.clone()),
        })
        .collect();
    for (id, d) in descr {
        if !v.iter().any(|c| c.id.eq_ignore_ascii_case(&id)) {
            v.push(CircuitoDoQuadro { id, descricao: d, circuito: None });
        }
    }
    v.sort_by(|a, b| crate::circuito::ordem_natural(&a.id, &b.id));
    v
}

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
    /// Descrição do circuito no quadro de cargas.
    pub descricao: String,
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
    /// Isolação dos condutores do circuito (a dele, a do quadro ou a das configurações).
    pub isolacao: nbr5410::Isolacao,
    /// A isolação foi escolhida para este circuito.
    pub isolacao_propria: bool,
    /// Tipo de carga no cálculo de demanda.
    pub tipo_carga: TipoCarga,
    pub tipo_carga_proprio: bool,
    /// Fator de potência.
    pub fp: f64,
    pub fp_proprio: bool,
    /// Tensão do circuito (V).
    pub tensao: f64,
    /// Potência por fase do quadro (VA).
    pub carga_fase: [f64; 3],
}

impl Linha {
    pub fn fases_texto(&self) -> String {
        self.fases.iter().map(|f| NOMES_FASES[*f]).collect::<Vec<_>>().join("+")
    }

    /// Potência ativa (W): a aparente vezes o fator de potência.
    pub fn potencia_w(&self) -> f64 {
        self.circuito.potencia * self.fp
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoQuadro {
    #[serde(serialize_with = "hex")]
    pub quadro: Handle,
    pub tag: String,
    /// Isolação dos condutores do quadro.
    pub isolacao: nbr5410::Isolacao,
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
    /// Circuitos criados no quadro de cargas ainda sem pontos.
    pub vazios: Vec<CircuitoDoQuadro>,
    /// Demanda pela tabela da concessionária.
    pub demanda: Demanda,
    /// Tensões de alimentação do quadro (V): fase-neutro e fase-fase.
    pub tensao_fn: f64,
    pub tensao_ff: f64,
    /// Corrente da potência instalada e da demandada (A), por fase.
    pub corrente_instalada: f64,
    pub corrente_demandada: f64,
    pub avisos: Vec<String>,
}

/// Tensões do quadro (fase-neutro, fase-fase) pelo atributo TENSAO no formato "220/127V"; sem
/// ele, as do sistema das configurações.
pub fn tensoes_do_quadro(q: &PontoEletrico, padrao: (f64, f64)) -> (f64, f64) {
    let Some(texto) = q.valor(crate::ponto::TAGS_TENSAO) else { return padrao };
    let partes: Vec<f64> = texto.split('/').filter_map(crate::ponto::numero).filter(|v| v.is_finite() && *v > 0.0).collect();
    match partes.as_slice() {
        [a, b] => (a.min(*b), a.max(*b)),
        _ => padrao,
    }
}

/// Corrente por fase (A) de uma potência (VA) num quadro de `fases` fases.
pub fn corrente_quadro(potencia_va: f64, fases: u8, tensao_fn: f64, tensao_ff: f64) -> f64 {
    match fases {
        0 | 1 if tensao_fn > 0.0 => potencia_va / tensao_fn,
        2 if tensao_fn > 0.0 => potencia_va / (2.0 * tensao_fn),
        _ if tensao_ff > 0.0 => potencia_va / (3f64.sqrt() * tensao_ff),
        _ => 0.0,
    }
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
    let isolacao = isolacao_do_quadro(q, setup);
    let mut cfg_base = setup.config_com(isolacao);
    (cfg_base.tensao_fn, cfg_base.tensao_ff) = tensoes_do_quadro(q, (cfg_base.tensao_fn, cfg_base.tensao_ff));
    let descr = descricoes(q);
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
    let props = propriedades(q);

    let mut linhas: Vec<Linha> = circuitos
        .iter()
        .zip(&rotas)
        .map(|(c, r)| {
            let agrupados = r.arestas.iter().filter_map(|e| por_aresta.get(e).copied()).max().unwrap_or(1).max(1);
            let p = props.get(&c.id.to_uppercase()).copied().unwrap_or_default();
            let isolacao_circuito = p.isolacao.unwrap_or(isolacao);
            let tipo_carga = p.carga.unwrap_or_else(|| TipoCarga::padrao(c.natureza));
            let cfg = nbr5410::Config {
                fca: nbr5410::fca_agrupamento(agrupados),
                fct: isolacao_circuito.fct(setup.temperatura_ambiente),
                isolacao: isolacao_circuito,
                ..cfg_base.clone()
            };
            let automatico = nbr5410::dimensionar_com(c, &cfg, r.momento);
            let sobrescrita = manuais.get(&c.id.to_uppercase()).copied().unwrap_or_default();
            let escolha = match (sobrescrita.secao.or(automatico.secao), sobrescrita.disjuntor.or(automatico.disjuntor)) {
                (Some(s), Some(d)) if sobrescrita != Sobrescrita::default() => nbr5410::verificar(c, &cfg, r.momento, s, d),
                _ => automatico.clone(),
            };
            let condutores = nbr5410::condutores_total(c.fases);
            Linha {
                circuito: (*c).clone(),
                descricao: descr.iter().find(|(id, _)| id.eq_ignore_ascii_case(&c.id)).map(|(_, d)| d.clone()).unwrap_or_default(),
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
                isolacao: isolacao_circuito,
                isolacao_propria: p.isolacao.is_some(),
                tipo_carga,
                tipo_carga_proprio: p.carga.is_some(),
                fp: p.fp.unwrap_or_else(|| tipo_carga.fp_padrao()),
                fp_proprio: p.fp.is_some(),
                tensao: nbr5410::tensao_circuito(c, &cfg).0,
                carga_fase: [0.0; 3],
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
            let parte = linhas[i].circuito.potencia / n as f64;
            carga[*f] += parte;
            if let Some(cf) = linhas[i].carga_fase.get_mut(*f) {
                *cf += parte;
            }
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

    // Demanda: cada ponto de carga é um aparelho do tipo de carga do seu circuito.
    let cargas: Vec<demanda::Cargas> = linhas
        .iter()
        .map(|l| demanda::Cargas {
            tipo: l.tipo_carga,
            aparelhos: analise
                .pontos
                .iter()
                .filter(|p| l.circuito.pontos.contains(&p.handle) && p.tipo.e_carga())
                .filter_map(|p| p.potencia)
                .collect(),
        })
        .collect();
    let demanda = demanda::calcular(setup.tabela_demanda(), &setup.edificacao, &cargas);
    let (tensao_fn, tensao_ff) = (cfg_base.tensao_fn, cfg_base.tensao_ff);
    let vazios = circuitos_do_quadro(analise, quadro).into_iter().filter(|c| c.circuito.is_none()).collect();

    Ok(ResumoQuadro {
        quadro,
        tag: q.rotulo(),
        isolacao,
        fases_quadro,
        vazios,
        corrente_instalada: corrente_quadro(demanda.instalada, fases_quadro, tensao_fn, tensao_ff),
        corrente_demandada: corrente_quadro(demanda.demandada, fases_quadro, tensao_fn, tensao_ff),
        demanda,
        tensao_fn,
        tensao_ff,
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
