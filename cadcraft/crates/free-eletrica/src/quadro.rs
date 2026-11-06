// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Rotear a fiação de um quadro: os circuitos dele roteados pelos eletrodutos, dimensionados
//! (pré-seleção automática de seção e disjuntor, que o projetista pode sobrescrever) e
//! distribuídos nas fases. É a tabela de resumo da janela Rotear fiação.

use std::collections::{BTreeMap, HashMap};

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
/// O nome do cabo no catálogo: `CABONOME.<circuito>`. A classe que vale no cálculo fica em `CABO.`.
pub const CHAVE_CABO_NOME: &str = "CABONOME.";
/// A curva do disjuntor de um circuito: `CURVA.<circuito>` (`B`, `C`, `D`).
pub const CHAVE_CURVA: &str = "CURVA.";
/// Grupo de DR de um circuito: `DR.<circuito>`.
pub const CHAVE_DR: &str = "DR.";
/// O último dimensionamento de um circuito: `DIM.<circuito>` = `seção;disjuntor;FCA;queda`.
pub const CHAVE_DIM: &str = "DIM.";
/// Esquema de condutores de um circuito: `ESQUEMA.<circuito>` (`F+N`, `F+N+T`, `2F+T`, `3F+N+T`).
pub const CHAVE_ESQUEMA: &str = "ESQUEMA.";
/// Tensão de um circuito: `TENSAO.<circuito>`; vale sobre a dos pontos.
pub const CHAVE_TENSAO: &str = "TENSAO.";
/// Fases travadas à mão de um circuito: `FASE.<circuito>` (`R`, `S`, `T`, `RS`, `RST`...).
pub const CHAVE_FASE: &str = "FASE.";
/// Sistema do quadro, na inserção dele: `F+N+T`, `2F+N+T`, `2F+T`, `3F+N+T`.
pub const CHAVE_SISTEMA_QUADRO: &str = "SISTEMA_QUADRO";
/// Tensão nominal do quadro: `127/220` ou `220/380`.
pub const CHAVE_TENSAO_NOMINAL: &str = "TENSAO_NOMINAL";
/// Tipo de cabo padrão do quadro: o dos circuitos que não escolheram o seu.
pub const CHAVE_CABO_QUADRO: &str = "CABO_PADRAO_QUADRO";
/// Quem alimenta o quadro: [`CONCESSIONARIA`] ou o handle de outro quadro.
pub const CHAVE_ALIMENTADO_POR: &str = "ALIMENTADO_POR";
pub const CONCESSIONARIA: &str = "CONCESSIONARIA";
/// Como o quadro é montado: `EMBUTIR` ou `SOBREPOR`.
pub const CHAVE_MONTAGEM: &str = "MONTAGEM_QUADRO";
/// As observações do quadro, para o cabeçalho da tabela.
pub const CHAVE_OBSERVACOES: &str = "OBS_QUADRO";
/// O número de módulos, quando o projetista o define.
pub const CHAVE_MODULOS: &str = "MODULOS_QUADRO";
/// Quantos circuitos reserva, quando o projetista os define.
pub const CHAVE_RESERVAS: &str = "RESERVAS_QUADRO";
/// A proteção geral travada à mão: o disjuntor (A) e a seção do alimentador (mm²).
pub const CHAVE_DISJ_GERAL: &str = "DISJ_GERAL";
pub const CHAVE_SECAO_GERAL: &str = "SECAO_GERAL";
/// O DPS do quadro: se tem (`1`/`0`), a classe e o disjuntor exclusivo antes dele (A).
pub const CHAVE_USAR_DPS: &str = "USAR_DPS";
pub const CHAVE_CLASSE_DPS: &str = "CLASSE_DPS";
pub const CHAVE_DISJ_DPS: &str = "DISJ_DPS";
/// O método de instalação do quadro; por circuito, `METODO.<circuito>`.
pub const CHAVE_METODO_QUADRO: &str = "METODO_QUADRO";
pub const CHAVE_METODO: &str = "METODO.";
/// A montagem nos trilhos DIN: módulos por trilho, o barramento das fases e onde ficam neutro e terra.
pub const CHAVE_MODULOS_TRILHO: &str = "MODULOS_TRILHO";
pub const CHAVE_BARRAMENTO_FASE: &str = "BARRAMENTO_FASE";
pub const CHAVE_BARRAMENTO_NT: &str = "BARRAMENTO_NT";
/// A proteção geral: os polos e a curva do disjuntor geral e o tipo de cabo do alimentador.
pub const CHAVE_POLOS_GERAL: &str = "POLOS_GERAL";
pub const CHAVE_CURVA_GERAL: &str = "CURVA_GERAL";
pub const CHAVE_CABO_GERAL: &str = "CABO_GERAL";
/// O DPS: a tensão máxima de operação contínua (Uc, V), a corrente máxima de descarga (Imax, kA) e os polos
/// do disjuntor exclusivo dele.
pub const CHAVE_TENSAO_DPS: &str = "TENSAO_DPS";
pub const CHAVE_IMAX_DPS: &str = "IMAX_DPS";
pub const CHAVE_POLOS_DPS: &str = "POLOS_DPS";
/// A corrente nominal (A) e a sensibilidade (mA) de cada grupo de DR, + o nome do grupo.
pub const CHAVE_DR_CORRENTE: &str = "DRIN.";
pub const CHAVE_DR_SENSIBILIDADE: &str = "DRSENS.";

/// Os tamanhos de trilho DIN oferecidos (módulos).
pub const MODULOS_POR_TRILHO: [u32; 4] = [12, 16, 18, 24];
/// As tensões Uc de DPS de catálogo (V). O padrão é 275 V, desde que passe de 1,1 × Uo (NBR 5410, 6.3.5.2.2).
pub const TENSOES_DPS: [f64; 5] = [175.0, 275.0, 320.0, 385.0, 440.0];
/// As correntes máximas de descarga de DPS oferecidas (kA).
pub const CORRENTES_DPS_KA: [f64; 2] = [20.0, 45.0];
/// As sensibilidades de DR (mA). 30 mA é o exigido na proteção complementar (NBR 5410, 5.1.3.2.2).
pub const SENSIBILIDADES_DR: [f64; 4] = [10.0, 30.0, 100.0, 300.0];
/// Correntes nominais de DR (A).
pub const CORRENTES_DR: [f64; 6] = [25.0, 40.0, 63.0, 80.0, 100.0, 125.0];

/// A classe do DPS (NBR 5410, 6.3.5; ABNT NBR IEC 61643-11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum ClasseDps {
    I,
    #[default]
    II,
    #[serde(rename = "I+II")]
    IeII,
}

impl ClasseDps {
    pub const TODAS: [ClasseDps; 3] = [Self::I, Self::II, Self::IeII];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::I => "I",
            Self::II => "II",
            Self::IeII => "I+II",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::I => "Classe I",
            Self::II => "Classe II",
            Self::IeII => "Classe I+II",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim().trim_start_matches("Classe").trim();
        Self::TODAS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t))
    }
}

/// O barramento que liga as fases aos disjuntores de cada trilho.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoBarramentoFase {
    /// Barramento pente: uma régua por fase sobre os disjuntores do trilho.
    #[default]
    Pente,
    /// Barramento espinha de peixe: uma espinha com derivações para os polos.
    EscamaDePeixe,
}

impl TipoBarramentoFase {
    pub const TODOS: [TipoBarramentoFase; 2] = [Self::Pente, Self::EscamaDePeixe];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Pente => "PENTE",
            Self::EscamaDePeixe => "ESCAMA",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Pente => "Barramento pente DIN",
            Self::EscamaDePeixe => "Escama de peixe (espinha de peixe)",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// Onde ficam os barramentos de neutro e de terra no chassi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PosicaoBarramentoNt {
    /// Neutro em cima e terra embaixo dos trilhos.
    #[default]
    TopoBase,
    /// Neutro à esquerda e terra à direita dos trilhos.
    Laterais,
}

impl PosicaoBarramentoNt {
    pub const TODAS: [PosicaoBarramentoNt; 2] = [Self::TopoBase, Self::Laterais];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::TopoBase => "TOPO_BASE",
            Self::Laterais => "LATERAIS",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::TopoBase => "Em cima e embaixo",
            Self::Laterais => "Nas laterais",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// O máximo aceito em módulos e reservas: entrada hostil não vira uma tabela sem fim.
pub const MAXIMO_MODULOS: u32 = 500;
pub const MAXIMO_RESERVAS: u32 = 100;

/// Como um quadro é montado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Montagem {
    Embutir,
    Sobrepor,
}

impl Montagem {
    pub const TODAS: [Montagem; 2] = [Self::Embutir, Self::Sobrepor];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Embutir => "EMBUTIR",
            Self::Sobrepor => "SOBREPOR",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Embutir => "Embutir",
            Self::Sobrepor => "Sobrepor",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|m| m.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// Os espaços de reserva de um quadro de distribuição (NBR 5410, 6.5.4.7, tabela 59): 2 até 6 circuitos,
/// 3 de 7 a 12, 4 de 13 a 30 e 15 % acima disso.
pub fn reservas_da_norma(circuitos: usize) -> u32 {
    match circuitos {
        0..=6 => 2,
        7..=12 => 3,
        13..=30 => 4,
        n => (n as f64 * 0.15).ceil() as u32,
    }
}

/// O sistema de um quadro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SistemaQuadro {
    Monofasico,
    BifasicoComNeutro,
    BifasicoSemNeutro,
    Trifasico,
}

impl SistemaQuadro {
    pub const TODOS: [SistemaQuadro; 4] = [Self::Monofasico, Self::BifasicoComNeutro, Self::BifasicoSemNeutro, Self::Trifasico];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Monofasico => "F+N+T",
            Self::BifasicoComNeutro => "2F+N+T",
            Self::BifasicoSemNeutro => "2F+T",
            Self::Trifasico => "3F+N+T",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Monofasico => "Monofásico (F+N+T)",
            Self::BifasicoComNeutro => "Bifásico (2F+N+T)",
            Self::BifasicoSemNeutro => "Bifásico (2F+T)",
            Self::Trifasico => "Trifásico (3F+N+T)",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|s| s.codigo().eq_ignore_ascii_case(t.trim()))
    }

    pub fn fases(self) -> u8 {
        match self {
            Self::Monofasico => 1,
            Self::BifasicoComNeutro | Self::BifasicoSemNeutro => 2,
            Self::Trifasico => 3,
        }
    }
}

/// A tensão nominal de um quadro: fase-neutro / fase-fase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TensaoNominal {
    V127x220,
    V220x380,
}

impl TensaoNominal {
    pub const TODAS: [TensaoNominal; 2] = [Self::V127x220, Self::V220x380];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::V127x220 => "127/220",
            Self::V220x380 => "220/380",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::V127x220 => "127/220 V",
            Self::V220x380 => "220/380 V",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim().trim_end_matches(['V', 'v']).trim();
        Self::TODAS.into_iter().find(|s| s.codigo() == t)
    }

    /// (fase-neutro, fase-fase), em V.
    pub fn tensoes(self) -> (f64, f64) {
        match self {
            Self::V127x220 => (127.0, 220.0),
            Self::V220x380 => (220.0, 380.0),
        }
    }
}

/// O que está gravado na inserção de um quadro. `None`: não escolhido, valem os atributos do bloco e
/// os padrões do programa.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigQuadro {
    pub sistema: Option<SistemaQuadro>,
    pub tensao: Option<TensaoNominal>,
    pub cabo: Option<nbr5410::TipoCabo>,
    /// [`CONCESSIONARIA`] ou o handle de outro quadro.
    pub alimentado_por: Option<String>,
    pub montagem: Option<Montagem>,
    pub observacoes: Option<String>,
    /// `None`: a estimativa de [`ResumoQuadro::modulos`].
    pub modulos: Option<u32>,
    /// `None`: o mínimo da NBR 5410.
    pub reservas: Option<u32>,
    /// O disjuntor geral (A) e a seção do alimentador (mm²) travados à mão. `None`: os calculados.
    pub disjuntor_geral: Option<f64>,
    pub secao_geral: Option<f64>,
    /// O quadro tem DPS. `None`: tem.
    pub usar_dps: Option<bool>,
    pub classe_dps: Option<ClasseDps>,
    /// O disjuntor exclusivo de proteção do DPS (A). `None`: sem disjuntor próprio.
    pub disjuntor_dps: Option<f64>,
    /// O método de instalação dos circuitos do quadro. `None`: o das configurações.
    pub metodo: Option<nbr5410::MetodoInstalacao>,
    /// Módulos por trilho DIN. `None`: 12 até 12 módulos, 18 acima.
    pub modulos_por_trilho: Option<u32>,
    pub barramento_fase: Option<TipoBarramentoFase>,
    /// Gravado por versões anteriores: os barramentos de neutro e terra ficam sempre nas laterais.
    pub posicao_barramento_nt: Option<PosicaoBarramentoNt>,
    /// Os polos e a curva do disjuntor geral. `None`: os polos do quadro e a curva C.
    pub polos_geral: Option<u8>,
    pub curva_geral: Option<nbr5410::Curva>,
    /// O tipo de cabo do alimentador. `None`: o cabo padrão do quadro.
    pub cabo_geral: Option<nbr5410::TipoCabo>,
    /// A tensão Uc (V) e a corrente máxima de descarga (kA) do DPS, e os polos do disjuntor dele.
    pub tensao_dps: Option<f64>,
    pub imax_dps: Option<f64>,
    pub polos_dps: Option<u8>,
    /// A corrente nominal (A) e a sensibilidade (mA) escolhidas por grupo de DR (nome em maiúsculas).
    pub drs: BTreeMap<String, ConfigDr>,
}

/// O que foi escolhido para um grupo de DR. `None`: a corrente sugerida e 30 mA.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDr {
    pub corrente: Option<f64>,
    pub sensibilidade: Option<f64>,
}

impl ConfigQuadro {
    /// O quadro tem DPS (o padrão).
    pub fn tem_dps(&self) -> bool {
        self.usar_dps.unwrap_or(true)
    }

    pub fn classe_do_dps(&self) -> ClasseDps {
        self.classe_dps.unwrap_or_default()
    }
}

/// Um número positivo e finito de um texto do XDATA.
fn positivo(v: &str) -> Option<f64> {
    crate::ponto::numero(v).filter(|x| x.is_finite() && *x > 0.0)
}

/// A configuração gravada num quadro.
pub fn config_do_quadro(q: &PontoEletrico) -> ConfigQuadro {
    let valor = |k: &str| q.extras.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
    ConfigQuadro {
        sistema: valor(CHAVE_SISTEMA_QUADRO).and_then(SistemaQuadro::de_codigo),
        tensao: valor(CHAVE_TENSAO_NOMINAL).and_then(TensaoNominal::de_codigo),
        cabo: valor(CHAVE_CABO_QUADRO).and_then(nbr5410::TipoCabo::de_codigo),
        alimentado_por: valor(CHAVE_ALIMENTADO_POR).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
        montagem: valor(CHAVE_MONTAGEM).and_then(Montagem::de_codigo),
        observacoes: valor(CHAVE_OBSERVACOES).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
        modulos: valor(CHAVE_MODULOS).and_then(|v| v.trim().parse().ok()).filter(|n| *n <= MAXIMO_MODULOS),
        reservas: valor(CHAVE_RESERVAS).and_then(|v| v.trim().parse().ok()).filter(|n| *n <= MAXIMO_RESERVAS),
        disjuntor_geral: valor(CHAVE_DISJ_GERAL).and_then(positivo),
        secao_geral: valor(CHAVE_SECAO_GERAL).and_then(positivo).filter(|s| nbr5410::indice_secao(*s).is_some()),
        usar_dps: valor(CHAVE_USAR_DPS).map(|v| !matches!(v.trim(), "0" | "false" | "NAO" | "nao")),
        classe_dps: valor(CHAVE_CLASSE_DPS).and_then(ClasseDps::de_codigo),
        disjuntor_dps: valor(CHAVE_DISJ_DPS).and_then(positivo),
        metodo: valor(CHAVE_METODO_QUADRO).and_then(nbr5410::MetodoInstalacao::de_codigo),
        modulos_por_trilho: valor(CHAVE_MODULOS_TRILHO).and_then(|v| v.trim().parse().ok()).filter(|n| MODULOS_POR_TRILHO.contains(n)),
        barramento_fase: valor(CHAVE_BARRAMENTO_FASE).and_then(TipoBarramentoFase::de_codigo),
        posicao_barramento_nt: valor(CHAVE_BARRAMENTO_NT).and_then(PosicaoBarramentoNt::de_codigo),
        polos_geral: valor(CHAVE_POLOS_GERAL).and_then(|v| v.trim().parse().ok()).filter(|n| (1..=3).contains(n)),
        curva_geral: valor(CHAVE_CURVA_GERAL).and_then(nbr5410::Curva::de_codigo),
        cabo_geral: valor(CHAVE_CABO_GERAL).and_then(nbr5410::TipoCabo::de_codigo),
        tensao_dps: valor(CHAVE_TENSAO_DPS).and_then(positivo).filter(|v| TENSOES_DPS.iter().any(|t| (t - v).abs() < 1e-6)),
        imax_dps: valor(CHAVE_IMAX_DPS).and_then(positivo).filter(|v| CORRENTES_DPS_KA.iter().any(|t| (t - v).abs() < 1e-6)),
        polos_dps: valor(CHAVE_POLOS_DPS).and_then(|v| v.trim().parse().ok()).filter(|n| (1..=3).contains(n)),
        drs: configs_de_dr(q),
    }
}

/// A corrente e a sensibilidade gravadas para cada grupo de DR.
fn configs_de_dr(q: &PontoEletrico) -> BTreeMap<String, ConfigDr> {
    let mut m: BTreeMap<String, ConfigDr> = BTreeMap::new();
    for (k, v) in &q.extras {
        let k = k.to_uppercase();
        if let Some(g) = k.strip_prefix(CHAVE_DR_CORRENTE) {
            m.entry(g.to_string()).or_default().corrente = positivo(v).filter(|c| CORRENTES_DR.iter().any(|x| (x - c).abs() < 1e-6));
        } else if let Some(g) = k.strip_prefix(CHAVE_DR_SENSIBILIDADE) {
            m.entry(g.to_string()).or_default().sensibilidade = positivo(v).filter(|c| SENSIBILIDADES_DR.iter().any(|x| (x - c).abs() < 1e-6));
        }
    }
    m
}

/// A tensão Uc padrão do DPS: 275 V, ou a primeira de catálogo que passe de 1,1 × Uo.
pub fn tensao_dps_padrao(tensao_fn: f64) -> f64 {
    let minima = 1.1 * tensao_fn;
    TENSOES_DPS.iter().copied().filter(|t| *t >= 275.0).find(|t| *t + 1e-9 >= minima).unwrap_or(440.0)
}

/// O quadro e o rack escolhidos na mini-barra das ferramentas de documentação (tabela do quadro, unifilar,
/// vista frontal e tabela do rack), por desenho aberto (`uid`): (desenho, é rack) → bloco.
static ALVO_DOCUMENTACAO: std::sync::Mutex<BTreeMap<(u64, bool), Handle>> = std::sync::Mutex::new(BTreeMap::new());

/// Escolhe o quadro (`rack` falso) ou o rack das próximas tabelas e diagramas do desenho. `None` esquece.
pub fn definir_alvo_documentacao(desenho: u64, rack: bool, alvo: Option<Handle>) {
    let mut m = ALVO_DOCUMENTACAO.lock().unwrap_or_else(|e| e.into_inner());
    match alvo {
        Some(h) => m.insert((desenho, rack), h),
        None => m.remove(&(desenho, rack)),
    };
}

/// O quadro (ou rack) escolhido para as próximas tabelas e diagramas do desenho.
pub fn alvo_documentacao(desenho: u64, rack: bool) -> Option<Handle> {
    ALVO_DOCUMENTACAO.lock().unwrap_or_else(|e| e.into_inner()).get(&(desenho, rack)).copied()
}

/// O quadro que alimenta este, quando é outro quadro.
pub fn quadro_que_alimenta(q: &PontoEletrico) -> Option<Handle> {
    config_do_quadro(q).alimentado_por.as_deref().and_then(Handle::parse_hex)
}

/// O que o projetista definiu para um circuito no quadro de cargas (o tipo de carga e o FP
/// vêm dos blocos: atributos TIPO_CARGA e FP).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Propriedades {
    pub isolacao: Option<nbr5410::Isolacao>,
    pub cabo: Option<nbr5410::TipoCabo>,
    /// O nome do cabo no catálogo.
    pub cabo_nome: Option<String>,
    pub curva: Option<nbr5410::Curva>,
    pub grupo_dr: Option<String>,
    pub calculado: Option<Calculado>,
    pub esquema: Option<nbr5410::Esquema>,
    pub tensao: Option<f64>,
    /// Fases travadas à mão: índices em [`NOMES_FASES`].
    pub fases: Option<Vec<usize>>,
    /// O método de instalação do circuito.
    pub metodo: Option<nbr5410::MetodoInstalacao>,
}

/// O que o último "Processar cálculo" gravou para um circuito. O quadro de cargas mostra isto: o
/// projeto pode mudar depois, e aí o circuito aparece como desatualizado.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calculado {
    pub secao: Option<f64>,
    pub disjuntor: Option<f64>,
    pub fca: f64,
    pub queda: Option<f64>,
}

impl Calculado {
    fn de_texto(t: &str) -> Option<Self> {
        let mut p = t.split(';').map(str::trim);
        let opcao = |v: Option<&str>| v.and_then(|x| x.parse::<f64>().ok()).filter(|x| x.is_finite());
        let (secao, disjuntor, fca, queda) = (opcao(p.next()), opcao(p.next()), opcao(p.next())?, opcao(p.next()));
        Some(Calculado { secao, disjuntor, fca, queda })
    }

    fn texto(&self) -> String {
        let f = |v: Option<f64>| v.map_or(String::new(), |x| x.to_string());
        format!("{};{};{};{}", f(self.secao), f(self.disjuntor), self.fca, f(self.queda))
    }
}

/// Como um circuito está em relação ao último cálculo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoCalculo {
    /// Nunca foi processado.
    Pendente,
    /// O projeto mudou depois do cálculo.
    Desatualizado,
    EmDia,
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
        if let Some(c) = k.strip_prefix(CHAVE_METODO) {
            m.entry(c.to_string()).or_default().metodo = nbr5410::MetodoInstalacao::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_ISOLACAO_CIRCUITO) {
            m.entry(c.to_string()).or_default().isolacao = nbr5410::Isolacao::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_DR) {
            m.entry(c.to_string()).or_default().grupo_dr = Some(v.trim().to_string()).filter(|g| !g.is_empty());
        } else if let Some(c) = k.strip_prefix(CHAVE_DIM) {
            m.entry(c.to_string()).or_default().calculado = Calculado::de_texto(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_CABO) {
            m.entry(c.to_string()).or_default().cabo = nbr5410::TipoCabo::de_codigo(v);
        } else if let Some(c) = k.strip_prefix(CHAVE_CABO_NOME) {
            m.entry(c.to_string()).or_default().cabo_nome = Some(v.trim().to_string()).filter(|n| !n.is_empty());
        } else if let Some(c) = k.strip_prefix(CHAVE_CURVA) {
            m.entry(c.to_string()).or_default().curva = nbr5410::Curva::de_codigo(v);
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

/// A isolação do quadro: a do cabo padrão dele, a gravada em desenhos anteriores ou a do padrão do programa.
pub fn isolacao_do_quadro(quadro: &PontoEletrico, setup: &Setup) -> nbr5410::Isolacao {
    config_do_quadro(quadro)
        .cabo
        .map(nbr5410::TipoCabo::isolacao)
        .or_else(|| isolacao_gravada(quadro))
        .unwrap_or_else(|| setup.cabo_padrao.isolacao())
}

/// O cabo dos circuitos do quadro que não escolheram o seu: o padrão do quadro, o equivalente ao gravado
/// em desenhos anteriores ou o padrão do programa.
pub fn cabo_do_quadro(quadro: &PontoEletrico, setup: &Setup) -> nbr5410::TipoCabo {
    config_do_quadro(quadro).cabo.unwrap_or_else(|| isolacao_gravada(quadro).map_or(setup.cabo_padrao, nbr5410::TipoCabo::de_isolacao))
}

/// A curva sugerida: B em cargas só resistivas, sem corrente de partida; C nas outras. É o ponto de
/// partida do projetista, que escolhe a curva de cada circuito.
pub fn curva_sugerida(tipos: &[TipoCarga]) -> nbr5410::Curva {
    if !tipos.is_empty() && tipos.iter().all(|t| *t == TipoCarga::Aquecimento) { nbr5410::Curva::B } else { nbr5410::Curva::C }
}

/// Um grupo de DR de um quadro.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrupoDr {
    pub nome: String,
    pub circuitos: Vec<String>,
    /// Soma das correntes de projeto dos circuitos (A).
    pub ib: f64,
    /// Corrente nominal do DR (A): a escolhida ou a sugerida, a conferir. `None`: a soma passa da tabela.
    pub dr: Option<f64>,
    /// A corrente foi escolhida à mão.
    pub corrente_travada: bool,
    /// A sensibilidade (corrente diferencial residual nominal, mA).
    pub sensibilidade: f64,
}

impl GrupoDr {
    /// O nome do DR na compra e nos desenhos: `DR 40 A / 30 mA`.
    pub fn texto(&self) -> String {
        format!("{} A / {} mA", self.dr.map_or("-".to_string(), br), br(self.sensibilidade))
    }
}

/// O próximo nome livre de grupo de DR.
pub fn proximo_grupo_dr(existentes: &[String]) -> String {
    (1..).map(|n| format!("IDR{n}")).find(|g| !existentes.iter().any(|e| e.eq_ignore_ascii_case(g))).unwrap_or_default()
}

/// O que "Processar cálculo" fez num quadro.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Processado {
    pub tag: String,
    pub circuitos: usize,
    pub avisos: Vec<String>,
}

/// O texto gravado do cálculo de um circuito.
pub fn texto_do_calculo(c: &Calculado) -> String {
    c.texto()
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
    /// O nome do cabo no catálogo.
    pub nome_cabo: String,
    /// A curva do disjuntor: a escolhida ou a sugerida.
    pub curva: nbr5410::Curva,
    /// A curva foi escolhida para este circuito.
    pub curva_propria: bool,
    /// O grupo de DR do circuito.
    pub grupo_dr: Option<String>,
    /// O que o último "Processar cálculo" gravou.
    pub calculado: Option<Calculado>,
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
    /// O método de instalação do circuito, e se foi escolhido para ele.
    pub metodo: nbr5410::MetodoInstalacao,
    pub metodo_proprio: bool,
    /// A seção do neutro (tabela 48) e a do condutor de proteção (tabela 58). `None`: o circuito não tem.
    pub secao_neutro: Option<f64>,
    pub secao_pe: Option<f64>,
    /// Cabo do circuito por seção (mm², m): as fases, o neutro e o PE, cada um na seção dele.
    pub cabo_por_secao: Vec<(f64, f64)>,
}

impl Linha {
    /// O resultado do cálculo de agora, no formato gravado.
    pub fn calculo_atual(&self) -> Calculado {
        Calculado { secao: self.escolha.secao, disjuntor: self.escolha.disjuntor, fca: self.fca, queda: self.escolha.queda }
    }

    /// Como o circuito está em relação ao último cálculo.
    pub fn estado_calculo(&self) -> EstadoCalculo {
        let igual = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-6,
            (None, None) => true,
            _ => false,
        };
        let atual = self.calculo_atual();
        match self.calculado {
            None => EstadoCalculo::Pendente,
            Some(c)
                if igual(c.secao, atual.secao)
                    && igual(c.disjuntor, atual.disjuntor)
                    && igual(Some(c.fca), Some(atual.fca))
                    && igual(c.queda, atual.queda) =>
            {
                EstadoCalculo::EmDia
            }
            Some(_) => EstadoCalculo::Desatualizado,
        }
    }

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
    /// O que está gravado no quadro: sistema, tensão nominal, cabo padrão e quem o alimenta.
    pub config: ConfigQuadro,
    /// O cabo dos circuitos que não escolheram o seu.
    pub cabo_padrao: nbr5410::TipoCabo,
    pub avisos: Vec<String>,
}

impl ResumoQuadro {
    /// Os grupos de DR do quadro, por nome, com os circuitos de cada um.
    pub fn grupos_dr(&self) -> Vec<GrupoDr> {
        let mut grupos: Vec<GrupoDr> = Vec::new();
        for l in &self.linhas {
            let Some(nome) = &l.grupo_dr else { continue };
            let i = match grupos.iter().position(|g| g.nome.eq_ignore_ascii_case(nome)) {
                Some(i) => i,
                None => {
                    let escolhido = self.config.drs.get(&nome.to_uppercase()).copied().unwrap_or_default();
                    grupos.push(GrupoDr {
                        nome: nome.clone(),
                        circuitos: Vec::new(),
                        ib: 0.0,
                        dr: escolhido.corrente,
                        corrente_travada: escolhido.corrente.is_some(),
                        sensibilidade: escolhido.sensibilidade.unwrap_or(30.0),
                    });
                    grupos.len() - 1
                }
            };
            if let Some(g) = grupos.get_mut(i) {
                g.circuitos.push(l.circuito.id.clone());
                g.ib += l.escolha.ib;
                if !g.corrente_travada {
                    g.dr = CORRENTES_DR.iter().copied().find(|c| *c >= g.ib);
                }
            }
        }
        grupos.sort_by(|a, b| crate::circuito::ordem_natural(&a.nome, &b.nome));
        grupos
    }

    /// Quantos circuitos reserva o quadro tem: os definidos ou o mínimo da NBR 5410.
    pub fn reservas(&self) -> u32 {
        self.config.reservas.unwrap_or_else(|| reservas_da_norma(self.linhas.len() + self.vazios.len()))
    }

    /// Os números dos circuitos reserva, depois do maior número em uso.
    pub fn ids_de_reserva(&self) -> Vec<String> {
        let ids = self.linhas.iter().map(|l| l.circuito.id.as_str()).chain(self.vazios.iter().map(|v| v.id.as_str()));
        let maior = ids.filter_map(|i| i.trim().parse::<u32>().ok()).max().unwrap_or(0);
        (1..=self.reservas()).map(|k| (maior.saturating_add(k)).to_string()).collect()
    }

    /// O número de módulos do quadro: o definido ou uma estimativa, sem o DPS. Um módulo por polo dos
    /// disjuntores (reservas como unipolares), mais o geral, e 2 ou 4 por DR, conforme os circuitos dele.
    pub fn modulos(&self) -> u32 {
        if let Some(n) = self.config.modulos {
            return n;
        }
        let polos: u32 = self.linhas.iter().map(|l| u32::from(l.esquema.fases())).sum();
        let vazios = u32::try_from(self.vazios.len()).unwrap_or(0);
        let drs: u32 = self
            .grupos_dr()
            .iter()
            .map(|g| {
                let multipolar = self.linhas.iter().any(|l| g.circuitos.contains(&l.circuito.id) && l.esquema.fases() > 1);
                if multipolar { 4 } else { 2 }
            })
            .sum();
        polos + vazios + self.reservas() + u32::from(self.fases_quadro.clamp(1, 3)) + drs
    }

    /// A alimentação do quadro em texto: "220/127 V bifásico".
    pub fn alimentacao(&self) -> String {
        match self.fases_quadro {
            1 => format!("{} V monofásico", br(self.tensao_fn)),
            2 => format!("{}/{} V bifásico", br(self.tensao_ff), br(self.tensao_fn)),
            _ => format!("{}/{} V trifásico", br(self.tensao_ff), br(self.tensao_fn)),
        }
    }
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
    /// O quadro tem DPS, a classe dele e o disjuntor exclusivo antes dele (A).
    pub dps: bool,
    pub classe_dps: ClasseDps,
    pub disjuntor_dps: Option<f64>,
    /// A tensão Uc (V) e a corrente máxima de descarga (kA) do DPS, e os polos do disjuntor dele.
    pub dps_tensao: f64,
    pub dps_imax: f64,
    pub polos_dps: u8,
    /// Os polos e a curva do disjuntor geral.
    pub polos_geral: u8,
    pub curva_geral: nbr5410::Curva,
    /// O tipo de cabo do alimentador.
    pub cabo_geral: nbr5410::TipoCabo,
    /// A seção e o disjuntor geral foram travados à mão.
    pub secao_travada: bool,
    pub disjuntor_travado: bool,
    /// O que a norma não aceita na escolha travada: avisa sem impedir.
    pub avisos: Vec<String>,
}

/// Dimensiona o alimentador do quadro pela corrente demandada. A seção e o disjuntor geral travados no
/// quadro valem sobre os calculados, conferidos contra Ib ≤ In ≤ Iz (5.3.4.1).
pub fn alimentador(ib: f64, fases: u8, tensao_fn: f64, cfg: &nbr5410::Config, config: &ConfigQuadro) -> Alimentador {
    let carregados = nbr5410::condutores_carregados(fases);
    let escolha = nbr5410::secao_para_corrente(ib, carregados, cfg);
    let mut avisos = Vec::new();
    let secao = config.secao_geral.or(escolha.map(|e| e.0));
    let iz = match config.secao_geral {
        Some(s) => nbr5410::indice_secao(s)
            .and_then(|i| nbr5410::capacidade_no_metodo(i, carregados, cfg.isolacao, cfg.metodo))
            .map(|v| v * cfg.fct * cfg.fca),
        None => escolha.map(|e| e.1),
    };
    let disjuntor = match config.disjuntor_geral {
        Some(d) => Some(d),
        None if config.secao_geral.is_some() => iz.and_then(|iz| cfg.criterio.escolher(ib, iz)),
        None => escolha.map(|e| e.2),
    };
    if config.secao_geral.is_some() || config.disjuntor_geral.is_some() {
        match (disjuntor, iz) {
            (Some(d), _) if d < ib - 1e-9 => avisos.push(format!("disjuntor geral de {} A abaixo da corrente demandada ({:.1} A)", br(d), ib)),
            (Some(d), Some(iz)) if d > iz + 1e-9 => {
                avisos.push(format!("disjuntor geral de {} A acima da capacidade do alimentador ({:.1} A)", br(d), iz))
            }
            (None, _) => avisos.push("a seção travada do alimentador não tem disjuntor entre Ib e Iz".into()),
            _ => {}
        }
    }
    if let (Some(dd), Some(dg)) = (config.disjuntor_dps.filter(|_| config.tem_dps()), disjuntor)
        && dd > dg + 1e-9
    {
        avisos.push(format!("disjuntor do DPS de {} A maior que o geral de {} A", br(dd), br(dg)));
    }
    let dps_uc = 1.1 * tensao_fn;
    let dps_tensao = config.tensao_dps.unwrap_or_else(|| tensao_dps_padrao(tensao_fn));
    if config.tem_dps() && dps_tensao + 1e-9 < dps_uc {
        avisos.push(format!("DPS de {} V abaixo de 1,1 × Uo ({} V): NBR 5410, 6.3.5.2.2", br(dps_tensao), br(dps_uc.ceil())));
    }
    let fases = fases.clamp(1, 3);
    Alimentador {
        ib,
        secao,
        iz,
        disjuntor,
        dr: disjuntor.and_then(|d| CORRENTES_DR.iter().copied().find(|c| *c >= d)),
        dps_uc,
        dps: config.tem_dps(),
        classe_dps: config.classe_do_dps(),
        disjuntor_dps: config.disjuntor_dps.filter(|_| config.tem_dps()),
        dps_tensao,
        dps_imax: config.imax_dps.unwrap_or(CORRENTES_DPS_KA[0]),
        polos_dps: config.polos_dps.unwrap_or(fases),
        polos_geral: config.polos_geral.unwrap_or(fases),
        curva_geral: config.curva_geral.unwrap_or(nbr5410::Curva::C),
        cabo_geral: config.cabo_geral.unwrap_or_else(|| nbr5410::TipoCabo::de_isolacao(cfg.isolacao)),
        secao_travada: config.secao_geral.is_some(),
        disjuntor_travado: config.disjuntor_geral.is_some(),
        avisos,
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

/// As seções do neutro e do condutor de proteção de um circuito com fase `secao`: o neutro reduzido pela
/// tabela 48 só no trifásico com neutro (3F+N+T); o PE pela tabela 58.
pub fn secoes_de_neutro_e_pe(esquema: nbr5410::Esquema, secao: Option<f64>) -> (Option<f64>, Option<f64>) {
    let neutro = secao.filter(|_| esquema.neutro()).map(|s| if esquema == nbr5410::Esquema::Fffnt { nbr5410::secao_neutro(s) } else { s });
    let pe = secao.filter(|_| esquema.protecao()).map(nbr5410::secao_pe);
    (neutro, pe)
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
    let config = config_do_quadro(q);
    cfg_base.metodo = config.metodo.unwrap_or(setup.metodo_instalacao);
    (cfg_base.tensao_fn, cfg_base.tensao_ff) = match config.tensao {
        Some(n) => n.tensoes(),
        None => tensoes_do_quadro(q, (cfg_base.tensao_fn, cfg_base.tensao_ff)),
    };
    let descr = descricoes(q);
    let fases_quadro = config.sistema.map(SistemaQuadro::fases).or(q.fases).unwrap_or(cfg_base.fases_padrao).clamp(1, 3);
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
            // A tensão escolhida para o circuito vale sobre tudo; a nominal do quadro, sobre a dos pontos.
            let da_nominal = config.tensao.map(|n| if c.fases >= 2 { n.tensoes().1 } else { n.tensoes().0 });
            if let Some(t) = p.and_then(|p| p.tensao).or(da_nominal) {
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
            let forcada = props.get(&c.id.to_uppercase()).and_then(|p| p.tensao).or(if config.tensao.is_some() { c.tensao } else { None });
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
            let metodo = p.metodo.unwrap_or(cfg_base.metodo);
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
                fca: setup.fca_manual.unwrap_or_else(|| nbr5410::fca_agrupamento(agrupados)),
                // No método D a referência é o solo a 20 °C: o FCT do ar não vale (o do solo ainda não é aplicado).
                fct: if metodo == nbr5410::MetodoInstalacao::D { 1.0 } else { isolacao_circuito.fct(setup.temperatura_ambiente) },
                isolacao: isolacao_circuito,
                metodo,
                queda_admissivel: if c.natureza == crate::circuito::Natureza::Iluminacao { setup.queda_iluminacao } else { setup.queda_admissivel },
                carregados: Some(esquema_de(c).carregados()),
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
            let (secao_neutro, secao_pe) = secoes_de_neutro_e_pe(esquema, escolha.secao);
            let metros = r.percurso * setup.fator_materiais;
            let mut cabo_por_secao: Vec<(f64, f64)> = Vec::new();
            let mut somar = |s: Option<f64>, n: u8| {
                if let Some(s) = s.filter(|_| n > 0) {
                    match cabo_por_secao.iter_mut().find(|(x, _)| (x - s).abs() < 1e-6) {
                        Some(m) => m.1 += metros * f64::from(n),
                        None => cabo_por_secao.push((s, metros * f64::from(n))),
                    }
                }
            };
            somar(escolha.secao, esquema.fases());
            somar(secao_neutro, 1);
            somar(secao_pe, 1);
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
                nome_cabo: p.cabo_nome.clone().unwrap_or_else(|| setup.nome_do_cabo(p.cabo.unwrap_or(cabo_padrao))),
                curva: p.curva.unwrap_or_else(|| curva_sugerida(&tipos_carga)),
                curva_propria: p.curva.is_some(),
                grupo_dr: p.grupo_dr.clone(),
                calculado: p.calculado,
                isolacao_propria: p.isolacao.is_some(),
                tipos_carga,
                potencia_w,
                fp,
                tensao: nbr5410::tensao_circuito(c, &cfg).0,
                carga_fase: [0.0; 3],
                esquema,
                fases_travadas: p.fases.is_some(),
                metodo,
                metodo_proprio: p.metodo.is_some(),
                secao_neutro,
                secao_pe,
                cabo_por_secao,
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
    if config.sistema == Some(SistemaQuadro::BifasicoSemNeutro) {
        for l in linhas.iter().filter(|l| l.esquema.neutro()) {
            avisos.push(format!("circuito {} usa neutro ({}) e o quadro é 2F+T, sem neutro", l.circuito.id, l.esquema.codigo()));
        }
    }
    let usadas = &carga[..usize::from(fases_quadro)];
    let maior = usadas.iter().copied().fold(0.0, f64::max);
    let menor = usadas.iter().copied().fold(f64::INFINITY, f64::min);
    let desequilibrio = if maior > 0.0 && fases_quadro > 1 { (maior - menor) / maior * 100.0 } else { 0.0 };

    let mut materiais: Vec<(f64, f64)> = Vec::new();
    for (s, metros) in linhas.iter().flat_map(|l| l.cabo_por_secao.iter().copied()) {
        match materiais.iter_mut().find(|(x, _)| (*x - s).abs() < 1e-6) {
            Some(m) => m.1 += metros,
            None => materiais.push((s, metros)),
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
        .filter_map(|p| Some(demanda::Cargas { tipo: p.tipo_carga_efetivo(), aparelhos: vec![p.potencia?], fator: p.fator_demanda }))
        .collect();
    let demanda = demanda::calcular(setup.tabela_demanda(), &setup.edificacao, &cargas);
    let (tensao_fn, tensao_ff) = (cfg_base.tensao_fn, cfg_base.tensao_ff);
    let vazios = circuitos_do_quadro(analise, quadro).into_iter().filter(|c| c.circuito.is_none()).collect();
    let corrente_demandada = corrente_quadro(demanda.demandada, fases_quadro, tensao_fn, tensao_ff);
    // O alimentador com a isolação do cabo dele, quando foi escolhido.
    let cfg_alimentador = match config.cabo_geral {
        Some(c) => nbr5410::Config { isolacao: c.isolacao(), fct: c.isolacao().fct(setup.temperatura_ambiente), ..cfg_base.clone() },
        None => cfg_base.clone(),
    };
    let mut alimentador = alimentador(corrente_demandada, fases_quadro, tensao_fn, &cfg_alimentador, &config);
    alimentador.cabo_geral = config.cabo_geral.unwrap_or(cabo_padrao);
    avisos.extend(alimentador.avisos.iter().cloned());
    if linhas.iter().any(|l| l.metodo == nbr5410::MetodoInstalacao::D) {
        avisos.push("método D (enterrado): o FCT do solo e o FCA da tabela 44 não são aplicados; confira".into());
    }

    Ok(ResumoQuadro {
        quadro,
        tag: q.rotulo(),
        isolacao,
        fases_quadro,
        vazios,
        corrente_instalada: corrente_quadro(demanda.instalada, fases_quadro, tensao_fn, tensao_ff),
        corrente_demandada,
        alimentador,
        config,
        cabo_padrao,
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
    let mut s = format!("RESUMO DA FIAÇÃO - {}\n{} · {} °C (FCT {})\n\n", r.tag, r.alimentacao(), br(setup.temperatura_ambiente), br(setup.fct()));
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
