// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! O cabeamento de telecom (ABNT NBR 14565): pontos de dados, CFTV, acesso sem fio, fibra e TV ligados ao
//! rack (quadro VDI) em estrela.
//!
//! - **Vincular** ([`vincular`]): cada ponto ganha o rack (`RACK_PAI`, no XDATA), a identificação
//!   (`ID_PONTO`: `D-01`, `CFTV-01`, `AP-01`, `FO-01`, `TV-01`), o cabo (`TIPO_CABO`, do catálogo) e a
//!   quantidade de cabos (`QTD_CABOS`). Com um rack só no projeto, todos vão para ele.
//! - **Rotear** ([`rotear`]): o menor caminho no grafo de eletrodutos de cada ponto até o rack dele. Um cabo
//!   da disciplina só passa por pontos de telecom e caixas de passagem
//!   ([`crate::disciplina::Disciplina::atravessa`]); um módulo empilhado (`host_point_handle`) sai da caixa
//!   base. Cada ponto leva os cabos dele inteiros até o rack: um trecho por onde passam D-01, D-02 e D-03
//!   leva três cabos.
//! - **Comprimento do enlace:** o caminho em planta, as subidas e descidas até os blocos e as folgas de
//!   terminação ([`FOLGA_RACK`], [`FOLGA_PONTO`]). Acima de [`LIMITE_UTP`] num cabo de pares trançados, um
//!   aviso: a NBR 14565 limita o cabo horizontal a 90 m.
//! - **Eletroduto:** o menor diâmetro do tipo (ou do perfil) do trecho com a ocupação até [`OCUPACAO_MAXIMA`].
//! - **Tabela do rack** no desenho: um bloco `TELECOM_RACK_<handle do rack>`, refeito pelo `ELEREGEN`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space};
use serde::Serialize;

use crate::disciplina::Disciplina;
use crate::ponto::{PontoEletrico, br};
use crate::rota::{self, Rede};
use crate::setup::{CategoriaTelecom, Setup, TamanhoEletroduto};
use crate::tabela::{ALTURA, ALTURA_TITULO, CAMADA, LARGURA_CARACTERE, linha, texto};
use crate::{Analise, xdata};

/// O rack do ponto (o handle, no XDATA).
pub const CHAVE_RACK: &str = "RACK_PAI";
/// A identificação, o cabo e a quantidade de cabos do ponto: no atributo, se o bloco tiver, ou no XDATA.
pub const TAGS_ID_TELECOM: &[&str] = &["ID_PONTO", "TAG"];
pub const TAGS_CABO: &[&str] = &["TIPO_CABO"];
pub const TAGS_QTD: &[&str] = &["QTD_CABOS"];
/// O comprimento do enlace do último cálculo (m), no XDATA do ponto.
pub const CHAVE_COMPRIMENTO: &str = "telecom_link_m";
/// Os cabos que passam no eletroduto, no XDATA dele.
pub const CHAVE_CABOS: &str = "telecom_cables";
/// O limite do cabo horizontal de pares trançados (m), NBR 14565.
pub const LIMITE_UTP: f64 = 90.0;
/// A ocupação máxima do eletroduto com cabos de telecom (0 a 1).
pub const OCUPACAO_MAXIMA: f64 = 0.40;
/// As folgas de terminação do cabo (m): no rack, para chegar ao patch panel, e na caixa do ponto. São prática
/// de instalação, não números da norma.
pub const FOLGA_RACK: f64 = 3.0;
pub const FOLGA_PONTO: f64 = 0.30;
/// O cabo de um ponto sem `TIPO_CABO`, por tipo de ponto.
const CABO_PADRAO_DADOS: &str = "UTP Cat 6";
const CABO_PADRAO_FIBRA: &str = "Fibra óptica N2";
const CABO_PADRAO_TV: &str = "Coaxial RG 59";
/// Mais cabos que isso num ponto é engano de digitação.
const MAXIMO_DE_CABOS: u32 = 48;

/// O que o ponto de telecom é: dá o prefixo da identificação e o cabo padrão.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoTelecom {
    Dados,
    Cftv,
    AcessoSemFio,
    Fibra,
    Tv,
}

impl TipoTelecom {
    pub fn prefixo(self) -> &'static str {
        match self {
            Self::Dados => "D",
            Self::Cftv => "CFTV",
            Self::AcessoSemFio => "AP",
            Self::Fibra => "FO",
            Self::Tv => "TV",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Dados => "Ponto de dados",
            Self::Cftv => "Câmera (CFTV)",
            Self::AcessoSemFio => "Ponto de acesso sem fio",
            Self::Fibra => "Ponto de fibra óptica",
            Self::Tv => "Ponto de TV",
        }
    }

    fn cabo_padrao(self) -> &'static str {
        match self {
            Self::Fibra => CABO_PADRAO_FIBRA,
            Self::Tv => CABO_PADRAO_TV,
            _ => CABO_PADRAO_DADOS,
        }
    }

    /// O tipo do ponto: pelo cabo dele e, sem isso, pelas palavras do bloco e da legenda.
    pub fn de_ponto(p: &PontoEletrico, setup: &Setup) -> Self {
        let cabo = p.valor(TAGS_CABO).and_then(|n| setup.cabos_telecom.iter().find(|c| c.nome.eq_ignore_ascii_case(n.trim())));
        match cabo.map(|c| c.categoria) {
            Some(CategoriaTelecom::Fibra) => return Self::Fibra,
            Some(CategoriaTelecom::Coaxial) => return Self::Tv,
            _ => {}
        }
        let texto = crate::ponto::normalizar(&format!("{} {}", p.bloco, p.legenda.as_deref().unwrap_or("")));
        let palavras: Vec<&str> = texto.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        let tem = |w: &str| palavras.contains(&w);
        if tem("CFTV") || texto.contains("CAMERA") {
            Self::Cftv
        } else if tem("AP") || tem("WIFI") || texto.contains("ACESSO SEM FIO") || texto.contains("ACCESS POINT") {
            Self::AcessoSemFio
        } else if tem("FO") || texto.contains("FIBRA") || texto.contains("OPTIC") {
            Self::Fibra
        } else if tem("TV") || texto.contains("COAX") || texto.contains("ANTENA") {
            Self::Tv
        } else {
            Self::Dados
        }
    }
}

/// `RACK_PAI` de um ponto desvinculado à mão: fica sem rack mesmo com um rack só no projeto.
pub const SEM_RACK: &str = "-";
/// A descrição da porta no quadro VDI (sem ela, a legenda do ponto).
pub const TAGS_DESCRICAO: &[&str] = &["DESC_PORTA"];

/// O ponto foi desvinculado à mão.
fn desvinculado(p: &PontoEletrico) -> bool {
    p.valor(&[CHAVE_RACK]).is_some_and(|v| v.trim() == SEM_RACK)
}

/// O rack gravado no ponto: o handle no XDATA ou, num atributo, a identificação do rack.
fn rack_gravado(p: &PontoEletrico, analise: &Analise) -> Option<Handle> {
    let v = p.valor(&[CHAVE_RACK])?.trim();
    let racks = || analise.pontos.iter().filter(|r| Disciplina::Telecom.e_central(r));
    Handle::parse_hex(v)
        .filter(|h| racks().any(|r| r.handle == *h))
        .or_else(|| racks().find(|r| r.tag.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(v))).map(|r| r.handle))
}

/// O rack do ponto: o gravado ou, com um rack só no projeto, ele. Desvinculado à mão: nenhum.
pub fn rack_do_ponto(p: &PontoEletrico, analise: &Analise) -> Option<Handle> {
    if desvinculado(p) {
        return None;
    }
    rack_gravado(p, analise).or_else(|| match racks(analise).as_slice() {
        [r] => Some(r.handle),
        _ => None,
    })
}

/// Os racks (quadros VDI) do desenho.
pub fn racks(analise: &Analise) -> Vec<&PontoEletrico> {
    analise.pontos.iter().filter(|p| Disciplina::Telecom.e_central(p)).collect()
}

/// Os pontos de telecom do desenho.
pub fn pontos(analise: &Analise) -> Vec<&PontoEletrico> {
    analise.pontos.iter().filter(|p| Disciplina::Telecom.e_ponto(p)).collect()
}

/// O cabo do ponto: o gravado ou o padrão do tipo.
pub fn cabo_do_ponto(p: &PontoEletrico, setup: &Setup) -> String {
    p.valor(TAGS_CABO)
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map_or_else(|| TipoTelecom::de_ponto(p, setup).cabo_padrao().to_string(), str::to_string)
}

/// Quantos cabos o ponto leva: o gravado (1 a 48) ou 1.
pub fn quantidade_do_ponto(p: &PontoEletrico) -> u32 {
    p.valor(TAGS_QTD).and_then(|v| v.trim().parse::<u32>().ok()).map_or(1, |q| q.clamp(1, MAXIMO_DE_CABOS))
}

/// A identificação gravada no ponto.
pub fn id_do_ponto(p: &PontoEletrico) -> Option<String> {
    p.valor(TAGS_ID_TELECOM).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

fn metros_por_unidade(doc: &Drawing, setup: &Setup) -> f64 {
    let (m, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    if m > 0.0 { m } else { 1.0 }
}

// ---------------------------------------------------------------- vincular e numerar

/// O que vincular faz: os pontos, o rack e a identificação de cada um.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vinculo {
    pub pontos: Vec<PontoVinculado>,
    pub avisos: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PontoVinculado {
    #[serde(serialize_with = "hex")]
    pub ponto: Handle,
    pub id: String,
    #[serde(serialize_with = "hex_opcional")]
    pub rack: Option<Handle>,
    pub cabo: String,
    pub quantidade: u32,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

fn hex_opcional<S: serde::Serializer>(h: &Option<Handle>, s: S) -> Result<S::Ok, S::Error> {
    match h {
        Some(h) => s.serialize_str(&h.hex()),
        None => s.serialize_none(),
    }
}

/// A ordem das identificações: os pontos de dados (`D-`) primeiro, depois os outros prefixos, cada um na
/// ordem natural dos números; as vazias no fim.
pub fn ordem_dos_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let chave = |s: &str| {
        let prefixo = s.rsplit_once('-').map_or(s, |(p, _)| p).to_uppercase();
        (s.is_empty(), prefixo != "D", prefixo)
    };
    chave(a).cmp(&chave(b)).then(crate::circuito::ordem_natural(a, b))
}

/// A porta de uma identificação numérica: `7` → 7. Identificações de versões anteriores (`D-07`) não são porta.
pub fn porta_do_id(id: &str) -> Option<u32> {
    id.trim().parse().ok().filter(|n| *n > 0)
}

/// Vincula os pontos de telecom ao rack e numera os que não têm identificação (com `renumerar`, todos).
/// `rack`: o rack de todos; sem ele, o de cada ponto, o único do projeto ou o mais perto pela rede. `alvos`:
/// só esses pontos. O número do ponto é a porta dele no rack (`1`, `2`, `3`...): um ponto com dois cabos ocupa
/// duas portas seguidas e leva o número da primeira. A numeração segue a distância até o rack pela rede, e
/// depois a posição na planta.
pub fn vincular(doc: &mut Drawing, setup: &Setup, rack: Option<Handle>, alvos: Option<&[Handle]>, renumerar: bool) -> Result<Vinculo, String> {
    let analise = crate::analisar(doc);
    let lista_racks = racks(&analise);
    if lista_racks.is_empty() {
        return Err("nenhum rack / quadro VDI no desenho: insira um com Inserir rack".into());
    }
    if let Some(r) = rack
        && !lista_racks.iter().any(|x| x.handle == r)
    {
        return Err(format!("o objeto {} não é um rack / quadro VDI", r.hex()));
    }
    let todos = pontos(&analise);
    if let Some(a) = alvos
        && let Some(h) = a.iter().find(|h| !todos.iter().any(|p| p.handle == **h))
    {
        return Err(format!("o objeto {} não é um ponto de telecom", h.hex()));
    }
    let escolhidos: Vec<&PontoEletrico> = todos.iter().copied().filter(|p| alvos.is_none_or(|a| a.contains(&p.handle))).collect();
    let rede = Rede::montar(doc, setup, &analise.pontos, metros_por_unidade(doc, setup));
    let arvores: Vec<(Handle, Option<rota::Arvore>)> =
        lista_racks.iter().map(|r| (r.handle, rede.no_de(r.handle).map(|n| rede.arvore_da_disciplina(n, Disciplina::Telecom)))).collect();
    let distancia = |rack: Handle, p: &PontoEletrico| {
        let a = arvores.iter().find(|(h, _)| *h == rack).and_then(|(_, a)| a.as_ref());
        rede.no_de(p.handle).and_then(|n| a.and_then(|a| a.distancia.get(n).copied())).filter(|d| d.is_finite())
    };
    let mut vinculo = Vinculo::default();
    // O rack de cada ponto.
    let mut com_rack: Vec<(&PontoEletrico, Option<Handle>)> = Vec::new();
    for p in &escolhidos {
        let r = rack.or_else(|| rack_do_ponto(p, &analise)).or_else(|| {
            lista_racks.iter().filter_map(|r| distancia(r.handle, p).map(|d| (r.handle, d))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
        });
        if r.is_none() {
            vinculo.avisos.push(format!("{}: mais de um rack e nenhum caminho de eletroduto até eles; escolha o rack", p.rotulo()));
        }
        com_rack.push((p, r));
    }
    // A numeração: as portas dos números que ficam são reservadas, em cada rack; os novos pegam as primeiras
    // portas livres seguidas.
    let renumerados: Vec<Handle> = com_rack.iter().filter(|(p, _)| renumerar || id_do_ponto(p).is_none()).map(|(p, _)| p.handle).collect();
    let mut usados: HashMap<Option<u64>, Vec<u32>> = HashMap::new();
    for p in todos.iter().filter(|p| !renumerados.contains(&p.handle)) {
        let r = com_rack.iter().find(|(x, _)| x.handle == p.handle).map_or_else(|| rack_do_ponto(p, &analise), |(_, r)| *r);
        if let Some(n) = id_do_ponto(p).and_then(|id| porta_do_id(&id)) {
            usados.entry(r.map(|h| h.0)).or_default().extend(n..n + quantidade_do_ponto(p));
        }
    }
    let mut ordem: Vec<&(&PontoEletrico, Option<Handle>)> = com_rack.iter().filter(|(p, _)| renumerados.contains(&p.handle)).collect();
    ordem.sort_by(|(a, ra), (b, rb)| {
        let d = |p: &PontoEletrico, r: &Option<Handle>| r.and_then(|r| distancia(r, p)).unwrap_or(f64::INFINITY);
        ra.map(|h| h.0)
            .cmp(&rb.map(|h| h.0))
            .then(d(a, ra).total_cmp(&d(b, rb)))
            .then(b.posicao[1].total_cmp(&a.posicao[1]))
            .then(a.posicao[0].total_cmp(&b.posicao[0]))
    });
    let mut novos: HashMap<Handle, String> = HashMap::new();
    for (p, r) in ordem {
        let q = quantidade_do_ponto(p);
        let lista = usados.entry(r.map(|h| h.0)).or_default();
        let n = (1..).find(|k| (*k..*k + q).all(|x| !lista.contains(&x))).unwrap_or(1);
        lista.extend(n..n + q);
        novos.insert(p.handle, n.to_string());
    }
    for (p, r) in &com_rack {
        let id = novos.get(&p.handle).cloned().or_else(|| id_do_ponto(p)).unwrap_or_default();
        crate::editar::gravar_xdata(doc, p.handle, CHAVE_RACK, r.map(|h| h.hex()).as_deref())?;
        if novos.contains_key(&p.handle) {
            crate::editar::definir(doc, p.handle, TAGS_ID_TELECOM, Some(&id))?;
        }
        let cabo = cabo_do_ponto(p, setup);
        if p.valor(TAGS_CABO).is_none_or(|c| c.trim().is_empty()) {
            crate::editar::definir(doc, p.handle, TAGS_CABO, Some(&cabo))?;
        }
        let quantidade = quantidade_do_ponto(p);
        if p.valor(TAGS_QTD).is_none_or(|c| c.trim().is_empty()) {
            crate::editar::definir(doc, p.handle, TAGS_QTD, Some(&quantidade.to_string()))?;
        }
        vinculo.pontos.push(PontoVinculado { ponto: p.handle, id, rack: *r, cabo, quantidade });
    }
    vinculo.pontos.sort_by(|a, b| ordem_dos_ids(&a.id, &b.id));
    Ok(vinculo)
}

/// O que trocar num ponto de telecom, pelo quadro VDI. `None` não mexe.
#[derive(Clone, Debug, Default)]
pub struct EdicaoPonto {
    /// Um nome do catálogo de telecom.
    pub cabo: Option<String>,
    /// De 1 a 48.
    pub quantidade: Option<u32>,
    /// O número do ponto (a porta). Vazio tira.
    pub id: Option<String>,
    /// A descrição da porta. Vazio volta à legenda do ponto.
    pub descricao: Option<String>,
    /// `Some(Some(rack))` vincula a esse rack; `Some(None)` desvincula (fica sem rack).
    pub rack: Option<Option<Handle>>,
}

/// Troca o cabo, a quantidade, o número, a descrição ou o rack dos pontos.
pub fn editar_pontos(doc: &mut Drawing, setup: &Setup, pontos: &[Handle], e: &EdicaoPonto) -> Result<(), String> {
    definir_cabo(doc, setup, pontos, e.cabo.as_deref(), e.quantidade)?;
    let analise = crate::analisar(doc);
    if let Some(Some(r)) = e.rack
        && !racks(&analise).iter().any(|x| x.handle == r)
    {
        return Err(format!("o objeto {} não é um rack / quadro VDI", r.hex()));
    }
    for h in pontos {
        if let Some(id) = &e.id {
            crate::editar::definir(doc, *h, TAGS_ID_TELECOM, Some(id.trim()).filter(|t| !t.is_empty()))?;
        }
        if let Some(d) = &e.descricao {
            crate::editar::gravar_xdata(doc, *h, TAGS_DESCRICAO[0], Some(d.trim()).filter(|t| !t.is_empty()))?;
        }
        match e.rack {
            Some(Some(r)) => crate::editar::gravar_xdata(doc, *h, CHAVE_RACK, Some(&r.hex()))?,
            Some(None) => crate::editar::gravar_xdata(doc, *h, CHAVE_RACK, Some(SEM_RACK))?,
            None => {}
        }
    }
    Ok(())
}

/// Troca o cabo (um nome do catálogo de telecom) e a quantidade de cabos (1 a 48) dos pontos. `None` não mexe.
pub fn definir_cabo(doc: &mut Drawing, setup: &Setup, pontos: &[Handle], cabo: Option<&str>, quantidade: Option<u32>) -> Result<(), String> {
    let analise = crate::analisar(doc);
    for h in pontos {
        if !analise.ponto(*h).is_some_and(|p| Disciplina::Telecom.e_ponto(p)) {
            return Err(format!("o objeto {} não é um ponto de telecom", h.hex()));
        }
    }
    let cabo = match cabo.map(str::trim) {
        Some(c) => Some(
            setup
                .cabos_telecom
                .iter()
                .find(|x| x.nome.eq_ignore_ascii_case(c))
                .map(|x| x.nome.clone())
                .ok_or_else(|| format!("o cabo \"{c}\" não está no catálogo de telecom"))?,
        ),
        None => None,
    };
    if quantidade.is_some_and(|q| !(1..=MAXIMO_DE_CABOS).contains(&q)) {
        return Err(format!("quantidade de cabos de 1 a {MAXIMO_DE_CABOS}"));
    }
    for h in pontos {
        if let Some(c) = &cabo {
            crate::editar::definir(doc, *h, TAGS_CABO, Some(c))?;
        }
        if let Some(q) = quantidade {
            crate::editar::definir(doc, *h, TAGS_QTD, Some(&q.to_string()))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- rotear e dimensionar

/// A situação de um enlace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusEnlace {
    Ok,
    /// Cabo de pares trançados acima de [`LIMITE_UTP`].
    AcimaDoLimite,
    /// Sem caminho de eletrodutos até o rack: o comprimento é estimado.
    SemCaminho,
    /// Sem rack: mais de um no projeto e nenhum vinculado.
    SemRack,
}

impl StatusEnlace {
    pub fn texto(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::AcimaDoLimite => "ACIMA DE 90 m",
            Self::SemCaminho => "SEM CAMINHO",
            Self::SemRack => "SEM RACK",
        }
    }
}

/// Um enlace: os cabos de um ponto até o rack.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Enlace {
    #[serde(serialize_with = "hex")]
    pub ponto: Handle,
    /// A identificação (vazia: ainda sem número).
    pub id: String,
    pub descricao: String,
    pub tipo: TipoTelecom,
    pub cabo: String,
    pub quantidade: u32,
    /// As portas do rack, em ordem: `"3"` ou `"3-4"`.
    pub porta: String,
    /// Comprimento de cada cabo (m), com as folgas.
    pub comprimento: f64,
    /// Sem caminho pela rede: distância de Manhattan 3D.
    pub estimado: bool,
    pub status: StatusEnlace,
    #[serde(serialize_with = "hex_opcional")]
    pub rack: Option<Handle>,
}

/// Um cabo num eletroduto.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaboNoTrecho {
    pub id: String,
    pub cabo: String,
    pub quantidade: u32,
}

/// Um eletroduto com os cabos de telecom que passam nele.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrechoTelecom {
    #[serde(serialize_with = "hex")]
    pub eletroduto: Handle,
    pub cabos: Vec<CaboNoTrecho>,
    pub eletroduto_escolhido: Option<TamanhoEletroduto>,
    pub ocupacao: Option<f64>,
    pub diametro_travado: bool,
    /// Os pontos que passam (`1..3`), os cabos (`3x UTP Cat 6`, o texto de cima da tag) e o diâmetro (`Ø25`, o
    /// de baixo).
    pub texto_ids: String,
    pub texto_cabos: String,
    pub texto_diametro: String,
}

/// Um rack com os enlaces dele, em ordem de porta.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RackTelecom {
    #[serde(serialize_with = "hex")]
    pub rack: Handle,
    pub tag: String,
    pub enlaces: Vec<Enlace>,
    /// Portas ocupadas (um cabo, uma porta).
    pub portas: u32,
}

/// O cabeamento do projeto.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Telecom {
    pub racks: Vec<RackTelecom>,
    /// Pontos sem rack.
    pub sem_rack: Vec<Enlace>,
    pub trechos: Vec<TrechoTelecom>,
    pub avisos: Vec<String>,
}

impl Telecom {
    /// O trecho de um eletroduto.
    pub fn trecho(&self, eletroduto: Handle) -> Option<&TrechoTelecom> {
        self.trechos.iter().find(|t| t.eletroduto == eletroduto)
    }

    /// Todos os enlaces.
    pub fn enlaces(&self) -> impl Iterator<Item = &Enlace> {
        self.racks.iter().flat_map(|r| r.enlaces.iter()).chain(self.sem_rack.iter())
    }
}

/// O nome do cabo na tag: o do catálogo, com a fibra encurtada (`Fibra óptica N2` → `Fibra N2`).
pub fn abreviar_cabo(nome: &str) -> String {
    nome.trim().replace("Fibra óptica", "Fibra").replace("Fibra optica", "Fibra")
}

/// As identificações em faixas: `D-01, D-02, D-03, D-05, CFTV-01` → `D-01..D-03, D-05, CFTV-01`. Três ou mais
/// números seguidos viram uma faixa.
pub fn texto_ids(cabos: &[CaboNoTrecho]) -> String {
    let mut por_prefixo: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    let mut soltos: Vec<String> = Vec::new();
    for c in cabos {
        let id = c.id.trim();
        let numerado = id.parse::<u32>().ok().map(|n| (String::new(), n));
        match numerado.or_else(|| id.rsplit_once('-').and_then(|(p, n)| n.parse::<u32>().ok().map(|n| (p.to_string(), n)))) {
            Some((p, n)) => por_prefixo.entry(p).or_default().push((n, id.to_string())),
            None if !id.is_empty() => soltos.push(id.to_string()),
            None => soltos.push("?".into()),
        }
    }
    let mut partes: Vec<String> = Vec::new();
    // D primeiro, depois os outros prefixos em ordem alfabética.
    let mut prefixos: Vec<&String> = por_prefixo.keys().collect();
    prefixos.sort_by_key(|p| (!p.is_empty() && p.as_str() != "D", (*p).clone()));
    for p in prefixos {
        let mut v = por_prefixo[p].clone();
        v.sort();
        v.dedup_by_key(|x| x.0);
        let mut i = 0;
        while i < v.len() {
            let mut j = i;
            while j + 1 < v.len() && v[j + 1].0 == v[j].0 + 1 {
                j += 1;
            }
            if j - i >= 2 {
                partes.push(format!("{}..{}", v[i].1, v[j].1));
            } else {
                partes.extend(v[i..=j].iter().map(|x| x.1.clone()));
            }
            i = j + 1;
        }
    }
    soltos.sort();
    soltos.dedup();
    partes.extend(soltos);
    partes.join(", ")
}

/// A quantidade e o tipo de cabo, agrupados: `3x UTP Cat 6 + 1x Fibra N2`. É o texto de cima da tag.
pub fn texto_cabos(cabos: &[CaboNoTrecho]) -> String {
    let mut por_cabo: BTreeMap<String, u32> = BTreeMap::new();
    for c in cabos {
        *por_cabo.entry(abreviar_cabo(&c.cabo)).or_default() += c.quantidade;
    }
    por_cabo.into_iter().map(|(c, n)| format!("{n}x {c}")).collect::<Vec<_>>().join(" + ")
}

/// O diâmetro do eletroduto, o texto de baixo da tag: `%%c25` (`%%c` é o Ø do CAD).
pub fn texto_diametro(diametro: Option<f64>) -> String {
    diametro.map_or_else(|| "%%c?".to_string(), |n| format!("%%c{}", br(n)))
}

/// O menor eletroduto do trecho com os cabos até [`OCUPACAO_MAXIMA`]; um diâmetro travado só tem a ocupação
/// conferida.
fn dimensionar(
    cabos: &[CaboNoTrecho],
    perfil: Option<&str>,
    tipo: rota::TipoEletroduto,
    travado: Option<f64>,
    setup: &Setup,
    avisos: &mut Vec<String>,
) -> (Option<TamanhoEletroduto>, Option<f64>, bool) {
    let area = |d: f64| std::f64::consts::PI * d * d / 4.0;
    let mut ocupada = 0.0;
    for c in cabos {
        match setup
            .cabos_telecom
            .iter()
            .find(|x| x.nome.eq_ignore_ascii_case(c.cabo.trim()))
            .map(|x| x.diametro)
            .filter(|d| d.is_finite() && *d > 0.0)
        {
            Some(d) => ocupada += area(d) * f64::from(c.quantidade),
            None => {
                avisos.push(format!("cabo \"{}\" sem diâmetro no catálogo de telecom: eletroduto não dimensionado", c.cabo));
                return (None, None, false);
            }
        }
    }
    let tamanhos = setup.tamanhos_do_trecho(perfil, tipo);
    if let Some(n) = travado {
        let Some(e) = tamanhos.into_iter().find(|e| (e.nominal - n).abs() < 1e-6) else {
            avisos.push(format!("diâmetro travado Ø{} não existe no tipo {}", br(n), tipo.abreviado()));
            return (None, None, false);
        };
        let oc = (e.interno > 0.0).then(|| ocupada / area(e.interno));
        if oc.is_some_and(|o| o > OCUPACAO_MAXIMA + 1e-9) {
            avisos.push(format!("eletroduto Ø{} travado à mão: cabos de telecom acima de 40 % de ocupação", br(e.nominal)));
        }
        return (Some(e), oc, true);
    }
    let escolhido = tamanhos
        .into_iter()
        .filter(|e| e.nominal + 1e-9 >= setup.eletroduto_minimo && e.interno > 0.0)
        .map(|e| {
            let oc = ocupada / area(e.interno);
            (e, oc)
        })
        .find(|(_, oc)| *oc <= OCUPACAO_MAXIMA + 1e-9);
    match escolhido {
        Some((e, oc)) => (Some(e), Some(oc), false),
        None => {
            let n: u32 = cabos.iter().map(|c| c.quantidade).sum();
            avisos.push(format!("{n} cabos de telecom num trecho: nenhum eletroduto {} comporta com 40 % de ocupação", tipo.abreviado()));
            (None, None, false)
        }
    }
}

/// Roteia o cabeamento do projeto em estrela e dimensiona os eletrodutos de telecom.
pub fn rotear(doc: &Drawing, analise: &Analise, setup: &Setup) -> Telecom {
    let metros = metros_por_unidade(doc, setup);
    let rede = Rede::montar(doc, setup, &analise.pontos, metros);
    let mut t = Telecom::default();
    let mut arvores: HashMap<Handle, Option<(usize, rota::Arvore)>> = HashMap::new();
    let mut por_aresta: BTreeMap<usize, Vec<CaboNoTrecho>> = BTreeMap::new();
    let mut por_rack: BTreeMap<u64, Vec<Enlace>> = BTreeMap::new();
    for p in pontos(analise) {
        let tipo = TipoTelecom::de_ponto(p, setup);
        let cabo = cabo_do_ponto(p, setup);
        let quantidade = quantidade_do_ponto(p);
        let id = id_do_ponto(p).unwrap_or_default();
        let categoria = setup.cabos_telecom.iter().find(|c| c.nome.eq_ignore_ascii_case(&cabo)).map(|c| c.categoria);
        let mut enlace = Enlace {
            ponto: p.handle,
            id: id.clone(),
            descricao: p
                .valor(TAGS_DESCRICAO)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string)
                .or_else(|| p.legenda.clone())
                .unwrap_or_else(|| tipo.nome().to_string()),
            tipo,
            cabo: cabo.clone(),
            quantidade,
            porta: String::new(),
            comprimento: 0.0,
            estimado: true,
            status: StatusEnlace::SemRack,
            rack: rack_do_ponto(p, analise),
        };
        let rotulo = if id.is_empty() { p.rotulo() } else { id.clone() };
        let Some(r) = enlace.rack.and_then(|h| analise.ponto(h)) else {
            t.avisos.push(format!("{rotulo}: sem rack (há mais de um no projeto): use Numerar e vincular ao rack"));
            t.sem_rack.push(enlace);
            continue;
        };
        let arvore = arvores.entry(r.handle).or_insert_with(|| {
            rede.no_de(r.handle).filter(|_| rede.conectado(r.handle)).map(|n| (n, rede.arvore_da_disciplina(n, Disciplina::Telecom)))
        });
        let z_ponto = rota::altura(p, setup);
        let caminho = rede.no_de(p.handle).and_then(|n| {
            let (_, a) = arvore.as_ref()?;
            let d = a.distancia.get(n).copied().filter(|d| d.is_finite())?;
            // Um módulo empilhado sai da caixa base: a diferença de altura entre os dois entra no cabo.
            let desnivel = rede.nos.get(n).map_or(0.0, |no| (no.z - z_ponto).abs());
            Some((a.caminho(n)?, d + desnivel))
        });
        match caminho {
            Some((arestas, d)) => {
                enlace.comprimento = d + FOLGA_RACK + FOLGA_PONTO;
                enlace.estimado = false;
                enlace.status = StatusEnlace::Ok;
                for e in arestas {
                    por_aresta.entry(e).or_default().push(CaboNoTrecho { id: rotulo.clone(), cabo: cabo.clone(), quantidade });
                }
            }
            None => {
                enlace.comprimento = rota::manhattan_3d(r.posicao, rota::altura(r, setup), p.posicao, z_ponto, metros) + FOLGA_RACK + FOLGA_PONTO;
                enlace.status = StatusEnlace::SemCaminho;
                t.avisos.push(format!("{rotulo}: sem caminho de eletrodutos de telecom até o rack {} (comprimento estimado)", r.rotulo()));
            }
        }
        if categoria == Some(CategoriaTelecom::Utp) && enlace.comprimento > LIMITE_UTP + 1e-9 {
            enlace.status = StatusEnlace::AcimaDoLimite;
            t.avisos.push(format!(
                "{rotulo}: enlace de {} m, acima dos {} m do cabo horizontal (NBR 14565)",
                crate::ponto::br((enlace.comprimento * 10.0).round() / 10.0),
                br(LIMITE_UTP)
            ));
        }
        por_rack.entry(r.handle.0).or_default().push(enlace);
    }
    for r in racks(analise) {
        let mut enlaces = por_rack.remove(&r.handle.0).unwrap_or_default();
        // O número do ponto é a porta; os sem número (ou de versões anteriores, como D-01) vêm depois da maior.
        enlaces.sort_by(|a, b| match (porta_do_id(&a.id), porta_do_id(&b.id)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => ordem_dos_ids(&a.id, &b.id),
        });
        let mut maior = 0u32;
        for e in &mut enlaces {
            let primeira = porta_do_id(&e.id).filter(|n| *n > maior).unwrap_or(maior + 1);
            let ultima = primeira + e.quantidade.max(1) - 1;
            maior = ultima;
            e.porta = if ultima > primeira { format!("{primeira}-{ultima}") } else { primeira.to_string() };
        }
        let portas = enlaces.iter().map(|e| e.quantidade).sum();
        t.racks.push(RackTelecom { rack: r.handle, tag: r.rotulo(), enlaces, portas });
    }
    for (e, mut cabos) in por_aresta {
        let Some(a) = rede.arestas.get(e) else { continue };
        cabos.sort_by(|x, y| ordem_dos_ids(&x.id, &y.id));
        let travado = crate::fiacao::diametro_travado(doc, a.eletroduto);
        let (escolhido, ocupacao, diametro_travado) = dimensionar(&cabos, a.perfil.as_deref(), a.tipo, travado, setup, &mut t.avisos);
        t.trechos.push(TrechoTelecom {
            eletroduto: a.eletroduto,
            texto_ids: texto_ids(&cabos),
            texto_cabos: texto_cabos(&cabos),
            texto_diametro: texto_diametro(escolhido.as_ref().map(|e| e.nominal)),
            cabos,
            eletroduto_escolhido: escolhido,
            ocupacao,
            diametro_travado,
        });
    }
    t.avisos.sort();
    t.avisos.dedup();
    t
}

/// Grava o resultado no desenho: o comprimento de cada enlace no ponto, e os cabos e o diâmetro em cada
/// eletroduto de telecom (um diâmetro travado fica). Devolve quantos eletrodutos mudaram.
pub fn gravar(doc: &mut Drawing, t: &Telecom) -> usize {
    for e in t.enlaces() {
        let valor = (!e.estimado).then(|| format!("{:.2}", e.comprimento));
        let _ = crate::editar::gravar_xdata(doc, e.ponto, CHAVE_COMPRIMENTO, valor.as_deref());
    }
    let mut n = 0;
    for tr in &t.trechos {
        let Some(mut e) = doc.model.get(tr.eletroduto).map(|e| (**e).clone()) else { continue };
        let x = &mut e.common.xdata;
        if !tr.diametro_travado {
            let d = tr.eletroduto_escolhido.as_ref().map(|d| d.nominal.to_string());
            xdata::gravar(x, rota::CHAVE_DIAMETRO, d.as_deref());
        }
        let cabos = tr.cabos.iter().map(|c| format!("{}:{}:{}", c.id, c.cabo, c.quantidade)).collect::<Vec<_>>().join(";");
        let antigos: Vec<String> = xdata::ler(x).into_iter().map(|(k, _)| k).filter(|k| k.starts_with(CHAVE_CABOS)).collect();
        for k in antigos {
            xdata::gravar(x, &k, None);
        }
        let chars: Vec<char> = cabos.chars().collect();
        for (i, parte) in chars.chunks(200).enumerate() {
            xdata::gravar(x, &format!("{CHAVE_CABOS}.{i}"), Some(&parte.iter().collect::<String>()));
        }
        doc.model.replace(e);
        n += 1;
    }
    n
}

// ---------------------------------------------------------------- tabela do rack no desenho

/// Prefixo do bloco da tabela do rack.
pub const PREFIXO_TABELA: &str = "TELECOM_RACK_";
const LINHA: f64 = 0.26;

fn nome_da_tabela(rack: Handle) -> String {
    format!("{PREFIXO_TABELA}{}", rack.hex())
}

/// As tabelas de rack do desenho: (inserção, rack).
pub fn tabelas(doc: &Drawing) -> Vec<(Handle, Handle)> {
    doc.model
        .iter()
        .filter_map(|e| {
            let EntityKind::Insert(i) = &e.kind else { return None };
            Some((e.handle, Handle::parse_hex(i.block.strip_prefix(PREFIXO_TABELA)?)?))
        })
        .collect()
}

/// O título da tabela de um rack.
pub fn titulo_da_tabela(tag: &str) -> String {
    format!("RACK / QUADRO VDI: {tag}")
}

/// As linhas da tabela: o cabeçalho e um enlace por linha.
fn linhas_da_tabela(r: &RackTelecom) -> Vec<[String; 5]> {
    let mut v = vec![["PORTA / PONTO", "DESCRIÇÃO", "CABO", "COMPRIMENTO 3D (m)", "STATUS"].map(String::from)];
    for e in &r.enlaces {
        let comprimento = format!("{}{}", if e.estimado { "~" } else { "" }, format!("{:.1}", e.comprimento).replace('.', ","));
        let cabo = if e.quantidade > 1 { format!("{}x {}", e.quantidade, e.cabo) } else { e.cabo.clone() };
        v.push([e.porta.clone(), e.descricao.clone(), cabo, comprimento, e.status.texto().into()]);
    }
    v
}

fn pecas_da_tabela(r: &RackTelecom, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let celulas = linhas_da_tabela(r);
    let larguras: Vec<f64> = (0..5)
        .map(|c| celulas.iter().map(|l| l[c].chars().count() as f64 * ALTURA * LARGURA_CARACTERE).fold(4.0 * ALTURA, f64::max) + ALTURA)
        .collect();
    let titulo = titulo_da_tabela(&r.tag);
    let total = larguras.iter().sum::<f64>().max(titulo.chars().count() as f64 * ALTURA_TITULO * LARGURA_CARACTERE + ALTURA);
    let sobra = total - larguras.iter().sum::<f64>();
    let mut larguras = larguras;
    if let Some(d) = larguras.get_mut(1) {
        *d += sobra;
    }
    let alto_titulo = LINHA * 1.5;
    let fundo = -alto_titulo - LINHA * celulas.len() as f64;
    let mut p = vec![texto(&titulo, [u(total / 2.0), u(-alto_titulo / 2.0)], u(ALTURA_TITULO), HAlign::Center)];
    for (a, b) in [([0.0, 0.0], [total, 0.0]), ([total, 0.0], [total, fundo]), ([total, fundo], [0.0, fundo]), ([0.0, fundo], [0.0, 0.0])] {
        p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
    }
    let mut y = -alto_titulo;
    for l in &celulas {
        p.push(linha([0.0, u(y)], [u(total), u(y)]));
        let mut x = 0.0;
        for (c, t) in l.iter().enumerate() {
            let w = larguras.get(c).copied().unwrap_or(0.0);
            p.push(if c == 1 {
                texto(t, [u(x + ALTURA / 2.0), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Left)
            } else {
                texto(t, [u(x + w / 2.0), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Center)
            });
            x += w;
        }
        y -= LINHA;
    }
    let mut x = 0.0;
    for w in larguras.iter().take(4) {
        x += w;
        p.push(linha([u(x), u(-alto_titulo)], [u(x), u(fundo)]));
    }
    p
}

fn gravar_tabela(doc: &mut Drawing, r: &RackTelecom, m: f64) {
    let nome = nome_da_tabela(r.rack);
    let mut b = Block::new(&nome);
    for k in pecas_da_tabela(r, m) {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(nome, Arc::new(b));
}

/// Insere a tabela do rack em `pos`. Se ele já tem tabela, ela é refeita e movida para lá.
pub fn inserir_tabela(doc: &mut Drawing, analise: &Analise, setup: &Setup, rack: Handle, pos: [f64; 2]) -> Result<Handle, String> {
    if !pos.iter().all(|v| v.is_finite()) {
        return Err("posição inválida".into());
    }
    let t = rotear(doc, analise, setup);
    let r = t.racks.iter().find(|r| r.rack == rack).ok_or_else(|| format!("o objeto {} não é um rack / quadro VDI", rack.hex()))?;
    gravar_tabela(doc, r, metros_por_unidade(doc, setup));
    if let Some((h, _)) = tabelas(doc).into_iter().find(|(_, q)| *q == rack)
        && let Some(mut e) = doc.model.get(h).map(|e| (**e).clone())
    {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert = Vec3::new(pos[0], pos[1], 0.0);
        }
        doc.model.replace(e);
        return Ok(h);
    }
    let ins = Insert {
        block: nome_da_tabela(rack),
        insert: Vec3::new(pos[0], pos[1], 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
        rotation: 0.0,
        attribs: Vec::new(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    };
    doc.ensure_layer(CAMADA);
    doc.add(&Space::Model, Common { layer: CAMADA.into(), ..Common::default() }, EntityKind::Insert(ins)).map_err(|e| e.to_string())
}

/// Refaz as tabelas de rack do desenho sem mexer na inserção; a de um rack que não existe mais sai. Devolve
/// quantas refez.
pub fn regenerar_tabelas(doc: &mut Drawing, t: &Telecom, setup: &Setup) -> usize {
    let m = metros_por_unidade(doc, setup);
    let mut n = 0;
    for (h, rack) in tabelas(doc) {
        match t.racks.iter().find(|r| r.rack == rack) {
            Some(r) => {
                gravar_tabela(doc, r, m);
                n += 1;
            }
            None => {
                doc.model.remove(h);
                doc.blocks.remove(&nome_da_tabela(rack));
            }
        }
    }
    n
}

// ---------------------------------------------------------------- materiais

/// Metros de um cabo de telecom.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetrosDeCabo {
    pub nome: String,
    pub metros: f64,
}

/// Os cabos de telecom (metros por cabo) e as terminações: uma no ponto e uma porta no rack por cabo.
pub fn materiais(t: &Telecom) -> (Vec<MetrosDeCabo>, Vec<(String, usize)>) {
    let mut cabos: BTreeMap<String, f64> = BTreeMap::new();
    let mut itens: BTreeMap<String, usize> = BTreeMap::new();
    for e in t.enlaces() {
        *cabos.entry(e.cabo.clone()).or_default() += e.comprimento * f64::from(e.quantidade);
        let q = usize::try_from(e.quantidade).unwrap_or(1);
        *itens.entry(format!("Terminação no ponto - {}", e.cabo)).or_default() += q;
        if e.rack.is_some() {
            *itens.entry(format!("Porta no rack (patch panel / DIO) - {}", e.cabo)).or_default() += q;
        }
    }
    (cabos.into_iter().map(|(nome, metros)| MetrosDeCabo { nome, metros }).collect(), itens.into_iter().collect())
}

/// Para os comandos: o quadro VDI de um rack em texto.
pub fn resumo(r: &RackTelecom) -> String {
    let mut s = format!("{}\n", titulo_da_tabela(&r.tag));
    for l in linhas_da_tabela(r) {
        s.push_str(&l.join("\t"));
        s.push('\n');
    }
    s
}
