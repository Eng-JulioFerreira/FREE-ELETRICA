// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
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

pub const NOMES_FASES: [&str; 3] = ["R", "S", "T"];

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
/// Tipo de cabo de um circuito: `CABO.<circuito>`; vale sobre `ISOL.`, dos desenhos anteriores.
pub const CHAVE_CABO: &str = "CABO.";
/// Esquema de condutores de um circuito: `ESQUEMA.<circuito>` (`F+N`, `F+N+T`, `2F+T`, `3F+N+T`).
pub const CHAVE_ESQUEMA: &str = "ESQUEMA.";
/// Tensão de um circuito: `TENSAO.<circuito>`; vale sobre a dos pontos.
pub const CHAVE_TENSAO: &str = "TENSAO.";
/// Fases travadas à mão de um circuito: `FASE.<circuito>` (`R`, `S`, `T`, `RS`, `RST`...).
pub const CHAVE_FASE: &str = "FASE.";
/// O que o projetista definiu para um circuito no quadro de cargas (o tipo de carga e o FP
/// vêm dos blocos: atributos TIPO_CARGA e FP).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Propriedades {
    pub isolacao: Option<nbr5410::Isolacao>,
    pub cabo: Option<nbr5410::TipoCabo>,
    pub esquema: Option<nbr5410::Esquema>,
    pub tensao: Option<f64>,
    /// Fases travadas à mão: índices em [`NOMES_FASES`].
    pub fases: Option<Vec<usize>>,
}

/// As fases de um texto como "R", "RS" ou "R+S+T". `None`: vazio ou letra que não é fase.
pub fn fases_de_texto(t: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for c in t.chars().filter(|c| !matches!(c, '+' | ' ' | ',')) {
        let i = NOMES_FASES.iter().position(|n| n.eq_ignore_ascii_case(&c.to_string()))?;
        if !v.contains(&i) {
            v.push(i);
        }
    }
    v.sort_unstable();
    (!v.is_empty()).then_some(v)
}

/// As propriedades gravadas no quadro, por circuito (em maiúsculas).
pub fn propriedades(quadro: &PontoEletrico) -> HashMap<String, Propriedades> {
    let mut m: HashMap<String, Propriedades> = HashMap::new();
    for (k, v) in &quadro.extras {
        let k = k.to_uppercase();
        if let Some(c) = k.strip_prefix(CHAVE_ISOLACAO_CIRCUITO) {
            m.entry(c.to_string()).or_default().isolacao = nbr5410::Isolacao::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_CABO) {
            m.entry(c.to_string()).or_default().cabo = nbr5410::TipoCabo::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_ESQUEMA) {
            m.entry(c.to_string()).or_default().esquema = nbr5410::Esquema::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_TENSAO) {
            m.entry(c.to_string()).or_default().tensao = crate::ponto::numero(v).filter(|t| *t > 0.0);
        } else if let Some(c) = k.strip_prefix(CHAVE_FASE) {
            m.entry(c.to_string()).or_default().fases = fases_de_texto(v);
        }
    }
    // O tipo de cabo manda; uma isolação de desenho anterior vira o cabo equivalente.
    for p in m.values_mut() {
        match (p.cabo, p.isolacao) {
            (Some(c), _) => p.isolacao = Some(c.isolacao()),
            (None, Some(i)) => p.cabo = Some(nbr5410::TipoCabo::de_isolacao(i)),
            (None, None) => {}
        }
    }
    m
}

/// A isolação gravada no quadro, de desenhos anteriores ao tipo de cabo por circuito.
fn isolacao_gravada(quadro: &PontoEletrico) -> Option<nbr5410::Isolacao> {
    quadro.extras.iter().find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_ISOLACAO)).and_then(|(_, v)| nbr5410::Isolacao::de_codigo(v))
}

/// A isolação do quadro: a gravada nele ou a do cabo padrão das configurações.
pub fn isolacao_do_quadro(quadro: &PontoEletrico, setup: &Setup) -> nbr5410::Isolacao {
    isolacao_gravada(quadro).unwrap_or_else(|| setup.cabo_padrao.isolacao())
}

/// O cabo dos circuitos do quadro que não escolheram o seu: o equivalente ao gravado no quadro ou o
/// padrão das configurações.
pub fn cabo_do_quadro(quadro: &PontoEletrico, setup: &Setup) -> nbr5410::TipoCabo {
    isolacao_gravada(quadro).map_or(setup.cabo_padrao, nbr5410::TipoCabo::de_isolacao)
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
    /// O tipo de cabo do circuito.
    pub tipo_cabo: nbr5410::TipoCabo,
    /// A isolação foi escolhida para este circuito.
    pub isolacao_propria: bool,
    /// Tipos de carga dos pontos do circuito (atributo TIPO_CARGA), sem repetição.
    pub tipos_carga: Vec<TipoCarga>,
    /// Potência ativa (W): soma da potência de cada ponto vezes o FP dele.
    pub potencia_w: f64,
    /// Fator de potência do circuito: W / VA (o dos pontos, quando o circuito não tem potência).
    pub fp: f64,
    /// Tensão do circuito (V).
    pub tensao: f64,
    /// Potência por fase do quadro (VA).
    pub carga_fase: [f64; 3],
    /// Esquema de condutores do circuito.
    pub esquema: nbr5410::Esquema,
    /// As fases foram travadas à mão.
    pub fases_travadas: bool,
}

impl Linha {
    pub fn fases_texto(&self) -> String {
        self.fases.iter().map(|f| NOMES_FASES[*f]).collect::<Vec<_>>().join("+")
    }

    /// Os tipos de carga para a tabela: um nome, ou os nomes juntos quando o circuito mistura.
    pub fn tipos_texto(&self) -> String {
        self.tipos_carga.iter().map(|t| t.nome()).collect::<Vec<_>>().join(" + ")
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
    /// O alimentador do quadro, pela demanda.
    pub alimentador: Alimentador,
    pub avisos: Vec<String>,
}

/// O alimentador do quadro, pela corrente demandada. DR e DPS são sugestões a conferir no projeto.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alimentador {
    /// Corrente de projeto (A), por fase.
    pub ib: f64,
    /// Seção das fases (mm²). `None`: a corrente passa da tabela.
    pub secao: Option<f64>,
    /// Capacidade corrigida pela temperatura (A).
    pub iz: Option<f64>,
    /// Disjuntor geral (A).
    pub disjuntor: Option<f64>,
    /// DR sugerido (NBR 5410, 5.1.3.2.2): corrente nominal (A); residual de 30 mA.
    pub dr: Option<f64>,
    /// DPS sugerido (NBR 5410, 6.3.5): classe II, In ≥ 5 kA, Uc ≥ 1,1 × Uo (V).
    pub dps_uc: f64,
}

/// Correntes nominais de DR (A).
const CORRENTES_DR: [f64; 6] = [25.0, 40.0, 63.0, 80.0, 100.0, 125.0];

/// Dimensiona o alimentador do quadro pela corrente demandada.
pub fn alimentador(ib: f64, fases: u8, tensao_fn: f64, cfg: &nbr5410::Config) -> Alimentador {
    let escolha = nbr5410::secao_para_corrente(ib, nbr5410::condutores_carregados(fases), cfg);
    let disjuntor = escolha.map(|e| e.2);
    Alimentador {
        ib,
        secao: escolha.map(|e| e.0),
        iz: escolha.map(|e| e.1),
        disjuntor,
        dr: disjuntor.and_then(|d| CORRENTES_DR.iter().copied().find(|c| *c >= d)),
        dps_uc: 1.1 * tensao_fn,
    }
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
fn indicacao(esquema: nbr5410::Esquema, secao: Option<f64>, setup: &Setup) -> String {
    let condutores = esquema.codigo();
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
    let cabo_padrao = cabo_do_quadro(q, setup);
    let mut cfg_base = setup.config_com(isolacao);
    (cfg_base.tensao_fn, cfg_base.tensao_ff) = tensoes_do_quadro(q, (cfg_base.tensao_fn, cfg_base.tensao_ff));
    let descr = descricoes(q);
    let fases_quadro = q.fases.unwrap_or(cfg_base.fases_padrao).clamp(1, 3);
    let props = propriedades(q);
    // Os circuitos do quadro com o que foi escolhido para eles (esquema e tensão).
    let esquema_de = |c: &Circuito| props.get(&c.id.to_uppercase()).and_then(|p| p.esquema).unwrap_or_else(|| nbr5410::Esquema::padrao(c.fases));
    let circuitos: Vec<Circuito> = analise
        .circuitos
        .iter()
        .filter(|c| c.quadro == Some(quadro))
        .map(|c| {
            let mut c = c.clone();
            let p = props.get(&c.id.to_uppercase());
            if p.and_then(|p| p.esquema).is_some() {
                c.fases = esquema_de(&c).fases();
            }
            if let Some(t) = p.and_then(|p| p.tensao) {
                c.tensao = Some(t);
            }
            c
        })
        .collect();

    let rede = Rede::montar(doc, setup, &analise.pontos, metros);
    let trechos_eletroduto = rede.arestas.len() + rede.soltos.len();
    if trechos_eletroduto == 0 {
        avisos.push(format!("nenhum eletroduto nas camadas \"{}\": comprimentos estimados pela distância de Manhattan 3D", setup.camadas_eletroduto));
    } else if !rede.conectado(quadro) {
        avisos.push("nenhum eletroduto liga o quadro: comprimentos estimados pela distância de Manhattan 3D".into());
    }
    if !rede.soltos.is_empty() {
        avisos.push(format!("{} eletroduto(s) sem bloco nas duas pontas: ficam fora do roteamento", rede.soltos.len()));
    }
    let arvore = rede.no_de(quadro).filter(|_| rede.conectado(quadro)).map(|n| rede.arvore(n));

    // Roteia todos primeiro: o FCA depende de quantos circuitos dividem cada trecho.
    let rotas: Vec<rota::RotaCircuito> = circuitos
        .iter()
        .map(|c| {
            let membros: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| c.pontos.contains(&p.handle)).collect();
            let (tensao, _) = nbr5410::tensao_circuito(c, &cfg_base);
            // A tensão escolhida para o circuito vale sobre a de cada ponto.
            let forcada = props.get(&c.id.to_uppercase()).and_then(|p| p.tensao);
            let corrente = |p: &PontoEletrico| {
                if p.tipo.e_carga() {
                    nbr5410::corrente_projeto(p.potencia.unwrap_or(0.0), forcada.or(p.tensao).unwrap_or(tensao), c.fases)
                } else {
                    0.0
                }
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
            let agrupados = r.arestas.iter().filter_map(|e| por_aresta.get(e).copied()).max().unwrap_or(1).max(1);
            let p = props.get(&c.id.to_uppercase()).cloned().unwrap_or_default();
            let isolacao_circuito = p.isolacao.unwrap_or(isolacao);
            // Tipo de carga e FP vêm de cada ponto (atributos TIPO_CARGA e FP).
            let cargas: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| c.pontos.contains(&p.handle) && p.tipo.e_carga()).collect();
            let mut tipos_carga: Vec<TipoCarga> = Vec::new();
            for p in &cargas {
                if !tipos_carga.contains(&p.tipo_carga_efetivo()) {
                    tipos_carga.push(p.tipo_carga_efetivo());
                }
            }
            let potencia_w: f64 = cargas.iter().map(|p| p.potencia.unwrap_or(0.0) * p.fp_efetivo()).sum();
            let fp = if c.potencia > 0.0 {
                potencia_w / c.potencia
            } else {
                cargas.first().map_or(TipoCarga::IluminacaoTug.fp_padrao(), |p| p.fp_efetivo())
            };
            let cfg = nbr5410::Config {
                fca: nbr5410::fca_agrupamento(agrupados),
                fct: isolacao_circuito.fct(setup.temperatura_ambiente),
                isolacao: isolacao_circuito,
                queda_admissivel: if c.natureza == crate::circuito::Natureza::Iluminacao { setup.queda_iluminacao } else { setup.queda_admissivel },
                ..cfg_base.clone()
            };
            let automatico = nbr5410::dimensionar_com(c, &cfg, r.momento);
            let sobrescrita = manuais.get(&c.id.to_uppercase()).copied().unwrap_or_default();
            let escolha = match (sobrescrita.secao.or(automatico.secao), sobrescrita.disjuntor.or(automatico.disjuntor)) {
                (Some(s), Some(d)) if sobrescrita != Sobrescrita::default() => nbr5410::verificar(c, &cfg, r.momento, s, d),
                _ => automatico.clone(),
            };
            let esquema = esquema_de(c);
            let condutores = esquema.condutores();
            Linha {
                circuito: c.clone(),
                descricao: descr.iter().find(|(id, _)| id.eq_ignore_ascii_case(&c.id)).map(|(_, d)| d.clone()).unwrap_or_default(),
                fases: Vec::new(),
                fct: cfg.fct,
                fca: cfg.fca,
                agrupados,
                comprimento: r.comprimento,
                fora_da_rede: r.fora_da_rede.len(),
                queda_admissivel: cfg.queda_admissivel,
                indicacao: indicacao(esquema, escolha.secao, setup),
                automatico,
                escolha,
                sobrescrita,
                condutores,
                cabo: r.percurso * f64::from(condutores) * setup.fator_materiais,
                isolacao: isolacao_circuito,
                tipo_cabo: p.cabo.unwrap_or(cabo_padrao),
                isolacao_propria: p.isolacao.is_some(),
                tipos_carga,
                potencia_w,
                fp,
                tensao: nbr5410::tensao_circuito(c, &cfg).0,
                carga_fase: [0.0; 3],
                esquema,
                fases_travadas: p.fases.is_some(),
            }
        })
        .collect();

    // Balanceamento: primeiro os circuitos travados à mão; depois as maiores cargas, cada uma nas
    // fases menos carregadas.
    let mut carga = [0.0f64; 3];
    let travadas = |l: &Linha| props.get(&l.circuito.id.to_uppercase()).and_then(|p| p.fases.clone());
    let mut ordem: Vec<usize> = (0..linhas.len()).collect();
    ordem.sort_by(|a, b| {
        travadas(&linhas[*b])
            .is_some()
            .cmp(&travadas(&linhas[*a]).is_some())
            .then(linhas[*b].circuito.potencia.total_cmp(&linhas[*a].circuito.potencia))
    });
    for i in ordem {
        let pedidas = linhas[i].circuito.fases;
        if pedidas > fases_quadro {
            avisos.push(format!("circuito {} pede {pedidas} fases e o quadro tem {fases_quadro}", linhas[i].circuito.id));
        }
        let mut escolhidas: Vec<usize> =
            match travadas(&linhas[i]).map(|v| v.into_iter().filter(|f| *f < usize::from(fases_quadro)).collect::<Vec<_>>()) {
                Some(v) if !v.is_empty() => {
                    if v.len() != usize::from(pedidas.min(fases_quadro)) {
                        avisos.push(format!("circuito {}: as fases travadas não batem com o esquema do circuito", linhas[i].circuito.id));
                    }
                    v
                }
                _ => {
                    let mut livres: Vec<usize> = (0..usize::from(fases_quadro)).collect();
                    livres.sort_by(|a, b| carga[*a].total_cmp(&carga[*b]));
                    livres.into_iter().take(usize::from(pedidas.min(fases_quadro))).collect()
                }
            };
        let n = escolhidas.len().max(1);
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

    // Demanda: cada ponto de carga dos circuitos do quadro é um aparelho do seu tipo de carga.
    let cargas: Vec<demanda::Cargas> = linhas
        .iter()
        .flat_map(|l| analise.pontos.iter().filter(|p| l.circuito.pontos.contains(&p.handle) && p.tipo.e_carga()))
        .filter_map(|p| Some(demanda::Cargas { tipo: p.tipo_carga_efetivo(), aparelhos: vec![p.potencia?] }))
        .collect();
    let demanda = demanda::calcular(setup.tabela_demanda(), &setup.edificacao, &cargas);
    let (tensao_fn, tensao_ff) = (cfg_base.tensao_fn, cfg_base.tensao_ff);
    let vazios = circuitos_do_quadro(analise, quadro).into_iter().filter(|c| c.circuito.is_none()).collect();
    let corrente_demandada = corrente_quadro(demanda.demandada, fases_quadro, tensao_fn, tensao_ff);

    Ok(ResumoQuadro {
        quadro,
        tag: q.rotulo(),
        isolacao,
        fases_quadro,
        vazios,
        corrente_instalada: corrente_quadro(demanda.instalada, fases_quadro, tensao_fn, tensao_ff),
        corrente_demandada,
        alimentador: alimentador(corrente_demandada, fases_quadro, tensao_fn, &cfg_base),
        demanda,
        tensao_fn,
        tensao_ff,
        potencia_total: linhas.iter().map(|l| l.circuito.potencia).sum(),
        linhas,
        carga_por_fase: carga,
        desequilibrio,
        materiais,
        metros_por_unidade: metros,
        trechos_eletroduto,
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
