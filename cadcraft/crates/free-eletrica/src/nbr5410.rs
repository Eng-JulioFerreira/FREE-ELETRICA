// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Dimensionamento de circuitos terminais pela NBR 5410.
//!
//! O que está aqui: corrente de projeto, seção mínima (tabela 47), capacidade de condução do cobre
//! pelos métodos de referência A1, A2, B1, B2, C e D (tabelas 36 e 37) e E e F (tabelas 38 e 39),
//! fatores de correção de temperatura (tabela 40, por isolação) e de agrupamento (tabela 42,
//! pelo roteamento), queda de tensão pelo momento elétrico (6.2.7), a escolha do disjuntor com
//! Ib ≤ In ≤ Iz (5.3.4.1) e as seções do neutro (tabela 48) e do condutor de proteção (tabela 58).
//!
//! Ainda não: seções acima de 120 mm², o método G, o FCT do solo e o FCA dos cabos enterrados
//! (tabelas 40 e 44) no método D, e a coordenação com o curto-circuito. Os valores das tabelas são os
//! da NBR 5410:2004 (iguais aos da IEC 60364-5-52) e devem ser conferidos contra a norma vigente.

use serde::{Deserialize, Serialize};

use crate::circuito::{Circuito, Natureza};

/// Seções nominais de condutor, em mm².
pub const SECOES: [f64; 12] = [1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0];

/// O método de instalação de referência (NBR 5410, tabela 33): escolhe a coluna das tabelas 36 a 39.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetodoInstalacao {
    /// Condutores isolados em eletroduto embutido em parede termicamente isolante.
    A1,
    /// Cabo multipolar em eletroduto embutido em parede termicamente isolante.
    A2,
    /// Condutores isolados em eletroduto aparente ou embutido em alvenaria.
    #[default]
    B1,
    /// Cabo multipolar em eletroduto aparente ou embutido em alvenaria.
    B2,
    /// Cabo unipolar ou multipolar sobre parede.
    C,
    /// Cabo em eletroduto enterrado.
    D,
    /// Cabo multipolar ao ar livre (bandeja perfurada, leito).
    E,
    /// Cabos unipolares justapostos ao ar livre.
    F,
}

impl MetodoInstalacao {
    pub const TODOS: [MetodoInstalacao; 8] = [Self::A1, Self::A2, Self::B1, Self::B2, Self::C, Self::D, Self::E, Self::F];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::A1 => "A1",
            Self::A2 => "A2",
            Self::B1 => "B1",
            Self::B2 => "B2",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::A1 => "A1: condutores isolados em eletroduto em parede isolante",
            Self::A2 => "A2: cabo multipolar em eletroduto em parede isolante",
            Self::B1 => "B1: condutores isolados em eletroduto (alvenaria ou aparente)",
            Self::B2 => "B2: cabo multipolar em eletroduto (alvenaria ou aparente)",
            Self::C => "C: cabo sobre parede",
            Self::D => "D: cabo em eletroduto enterrado",
            Self::E => "E: cabo multipolar ao ar livre",
            Self::F => "F: cabos unipolares justapostos ao ar livre",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|m| m.codigo().eq_ignore_ascii_case(t.trim()))
    }

    /// A coluna nas tabelas de capacidade.
    fn coluna(self) -> usize {
        match self {
            Self::A1 => 0,
            Self::A2 => 1,
            Self::B1 => 2,
            Self::B2 => 3,
            Self::C => 4,
            Self::D => 5,
            Self::E => 6,
            Self::F => 7,
        }
    }
}

/// O critério da escolha do disjuntor entre Ib e Iz (NBR 5410, 5.3.4.1: Ib ≤ In ≤ Iz).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CriterioDisjuntor {
    /// O menor disjuntor com In ≥ Ib: protege o circuito pela carga.
    #[default]
    CorrenteProjetoIb,
    /// O maior disjuntor com In ≤ Iz do cabo escolhido: aproveita o cabo.
    CapacidadeCaboIz,
}

impl CriterioDisjuntor {
    pub const TODOS: [CriterioDisjuntor; 2] = [Self::CorrenteProjetoIb, Self::CapacidadeCaboIz];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::CorrenteProjetoIb => "IB",
            Self::CapacidadeCaboIz => "IZ",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::CorrenteProjetoIb => "Menor disjuntor acima de Ib (corrente de projeto)",
            Self::CapacidadeCaboIz => "Maior disjuntor até Iz (capacidade do cabo)",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }

    /// O disjuntor de [`DISJUNTORES`] entre `ib` e `iz` pelo critério.
    pub fn escolher(self, ib: f64, iz: f64) -> Option<f64> {
        let cabe = |d: &f64| *d >= ib && *d <= iz;
        match self {
            Self::CorrenteProjetoIb => DISJUNTORES.iter().copied().find(cabe),
            Self::CapacidadeCaboIz => DISJUNTORES.iter().copied().rfind(cabe),
        }
    }
}

/// Capacidade de condução do cobre com 30 °C no ar e 20 °C no solo (método D), em A, por seção de [`SECOES`] e pelas colunas A1, A2, B1, B2,
/// C, D, E e F. Tabela 36 (PVC) e 38 (PVC, E e F); tabela 37 (EPR/XLPE) e 39 (EPR/XLPE, E e F). No método F
/// (unipolares justapostos; três carregados em trifólio) os valores aqui começam em 25 mm²: as seções menores
/// ficam sem capacidade e o cálculo passa à seguinte.
const N: f64 = f64::NAN;
const IZ_PVC_2: [[f64; 8]; 12] = [
    [14.5, 14.0, 17.5, 16.5, 19.5, 22.0, 22.0, N],
    [19.5, 18.5, 24.0, 23.0, 27.0, 29.0, 30.0, N],
    [26.0, 25.0, 32.0, 30.0, 36.0, 38.0, 40.0, N],
    [34.0, 32.0, 41.0, 38.0, 46.0, 47.0, 51.0, N],
    [46.0, 43.0, 57.0, 52.0, 63.0, 63.0, 70.0, N],
    [61.0, 57.0, 76.0, 69.0, 85.0, 81.0, 94.0, N],
    [80.0, 75.0, 101.0, 90.0, 112.0, 104.0, 119.0, 131.0],
    [99.0, 92.0, 125.0, 111.0, 138.0, 125.0, 148.0, 162.0],
    [119.0, 110.0, 151.0, 133.0, 168.0, 148.0, 180.0, 196.0],
    [151.0, 139.0, 192.0, 168.0, 213.0, 183.0, 232.0, 251.0],
    [182.0, 167.0, 232.0, 201.0, 258.0, 216.0, 282.0, 304.0],
    [210.0, 192.0, 269.0, 232.0, 299.0, 246.0, 328.0, 352.0],
];
const IZ_PVC_3: [[f64; 8]; 12] = [
    [13.5, 13.0, 15.5, 15.0, 17.5, 18.0, 18.5, N],
    [18.0, 17.5, 21.0, 20.0, 24.0, 24.0, 25.0, N],
    [24.0, 23.0, 28.0, 27.0, 32.0, 31.0, 34.0, N],
    [31.0, 29.0, 36.0, 34.0, 41.0, 39.0, 43.0, N],
    [42.0, 39.0, 50.0, 46.0, 57.0, 52.0, 60.0, N],
    [56.0, 52.0, 68.0, 62.0, 76.0, 67.0, 80.0, N],
    [73.0, 68.0, 89.0, 80.0, 96.0, 86.0, 101.0, 110.0],
    [89.0, 83.0, 110.0, 99.0, 119.0, 103.0, 126.0, 137.0],
    [108.0, 99.0, 134.0, 118.0, 144.0, 122.0, 153.0, 167.0],
    [136.0, 125.0, 171.0, 149.0, 184.0, 151.0, 196.0, 216.0],
    [164.0, 150.0, 207.0, 179.0, 223.0, 179.0, 238.0, 264.0],
    [188.0, 172.0, 239.0, 206.0, 259.0, 203.0, 276.0, 308.0],
];
const IZ_EPR_2: [[f64; 8]; 12] = [
    [19.0, 18.5, 23.0, 22.0, 24.0, 26.0, 26.0, N],
    [26.0, 25.0, 31.0, 30.0, 33.0, 34.0, 36.0, N],
    [35.0, 33.0, 42.0, 40.0, 45.0, 44.0, 49.0, N],
    [45.0, 42.0, 54.0, 51.0, 58.0, 56.0, 63.0, N],
    [61.0, 57.0, 75.0, 69.0, 80.0, 73.0, 86.0, N],
    [81.0, 76.0, 100.0, 91.0, 107.0, 95.0, 115.0, N],
    [106.0, 99.0, 133.0, 119.0, 138.0, 121.0, 149.0, 161.0],
    [131.0, 121.0, 164.0, 146.0, 171.0, 146.0, 185.0, 200.0],
    [158.0, 145.0, 198.0, 175.0, 209.0, 173.0, 225.0, 242.0],
    [200.0, 183.0, 253.0, 221.0, 269.0, 213.0, 289.0, 310.0],
    [241.0, 220.0, 306.0, 265.0, 328.0, 252.0, 352.0, 377.0],
    [278.0, 253.0, 354.0, 305.0, 382.0, 287.0, 410.0, 437.0],
];
const IZ_EPR_3: [[f64; 8]; 12] = [
    [17.0, 16.5, 20.0, 19.5, 22.0, 22.0, 23.0, N],
    [23.0, 22.0, 28.0, 26.0, 30.0, 29.0, 32.0, N],
    [31.0, 30.0, 37.0, 35.0, 40.0, 37.0, 42.0, N],
    [40.0, 38.0, 48.0, 44.0, 52.0, 46.0, 54.0, N],
    [54.0, 51.0, 66.0, 60.0, 71.0, 61.0, 75.0, N],
    [73.0, 68.0, 88.0, 80.0, 96.0, 79.0, 100.0, N],
    [95.0, 89.0, 117.0, 105.0, 119.0, 101.0, 127.0, 135.0],
    [117.0, 109.0, 144.0, 128.0, 147.0, 122.0, 158.0, 169.0],
    [141.0, 130.0, 175.0, 154.0, 179.0, 144.0, 192.0, 207.0],
    [179.0, 164.0, 222.0, 194.0, 229.0, 178.0, 246.0, 268.0],
    [216.0, 197.0, 269.0, 233.0, 278.0, 211.0, 298.0, 328.0],
    [249.0, 227.0, 312.0, 268.0, 322.0, 240.0, 346.0, 383.0],
];

/// Tabela 58: a seção mínima do condutor de proteção (PE) de mesmo material que a fase: igual à fase até
/// 16 mm², 16 mm² de 25 a 35 mm², e metade da fase acima disso (a seção comercial logo acima).
pub fn secao_pe(secao_fase: f64) -> f64 {
    if secao_fase <= 16.0 + 1e-9 {
        secao_fase
    } else if secao_fase <= 35.0 + 1e-9 {
        16.0
    } else {
        SECOES.iter().copied().find(|s| *s + 1e-9 >= secao_fase / 2.0).unwrap_or(secao_fase)
    }
}

/// Tabela 48: a seção reduzida do neutro num circuito trifásico equilibrado com terceira harmônica até
/// 15 % (6.2.6.2.6): igual à fase até 25 mm²; 25 mm² com fase de 35 e 50; 35 com 70; 50 com 95; 70 com 120.
/// Nos outros circuitos o neutro é igual à fase.
pub fn secao_neutro(secao_fase: f64) -> f64 {
    const TABELA_48: [(f64, f64); 5] = [(35.0, 25.0), (50.0, 25.0), (70.0, 35.0), (95.0, 50.0), (120.0, 70.0)];
    TABELA_48.iter().find(|(f, _)| (f - secao_fase).abs() < 1e-6).map_or(secao_fase, |(_, n)| *n)
}

/// Tabela 40, temperatura ambiente (não no solo): (°C, fator PVC, fator EPR/XLPE).
const FCT_AMBIENTE: [(f64, f64, f64); 11] = [
    (10.0, 1.22, 1.15),
    (15.0, 1.17, 1.12),
    (20.0, 1.12, 1.08),
    (25.0, 1.06, 1.04),
    (30.0, 1.00, 1.00),
    (35.0, 0.94, 0.96),
    (40.0, 0.87, 0.91),
    (45.0, 0.79, 0.87),
    (50.0, 0.71, 0.82),
    (55.0, 0.61, 0.76),
    (60.0, 0.50, 0.71),
];

/// Correntes nominais de disjuntores termomagnéticos (A).
pub const DISJUNTORES: [f64; 12] = [6.0, 10.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0];

/// Resistividade do cobre a 70 °C (temperatura máxima do PVC em serviço), Ω·mm²/m.
pub const RESISTIVIDADE_COBRE_70C: f64 = 0.0206;
/// Resistividade do cobre a 90 °C (temperatura máxima do EPR e do XLPE em serviço), Ω·mm²/m.
pub const RESISTIVIDADE_COBRE_90C: f64 = 0.0219;

/// Isolação dos condutores.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Isolacao {
    /// PVC 70 °C (cabos de 450/750 V).
    #[default]
    Pvc,
    /// EPR 90 °C (cabos de 0,6/1 kV).
    Epr,
    /// XLPE 90 °C (cabos de 0,6/1 kV).
    Xlpe,
}

impl Isolacao {
    pub const TODAS: [Isolacao; 3] = [Self::Pvc, Self::Epr, Self::Xlpe];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Pvc => "PVC 70 °C",
            Self::Epr => "EPR 90 °C",
            Self::Xlpe => "XLPE 90 °C",
        }
    }

    /// O código curto ("PVC", "EPR", "XLPE"), como fica gravado no quadro.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Pvc => "PVC",
            Self::Epr => "EPR",
            Self::Xlpe => "XLPE",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|i| i.codigo().eq_ignore_ascii_case(t.trim()))
    }

    /// Resistividade do cobre na temperatura máxima de serviço da isolação (Ω·mm²/m).
    pub fn resistividade(self) -> f64 {
        if self == Self::Pvc { RESISTIVIDADE_COBRE_70C } else { RESISTIVIDADE_COBRE_90C }
    }

    /// Fator de correção de temperatura (tabela 40). Entre duas linhas da tabela vale a
    /// temperatura de cima, que é o lado seguro.
    pub fn fct(self, temperatura: f64) -> f64 {
        let fator = |l: &(f64, f64, f64)| if self == Self::Pvc { l.1 } else { l.2 };
        FCT_AMBIENTE.iter().find(|l| temperatura <= l.0 + 1e-9).or(FCT_AMBIENTE.last()).map_or(1.0, fator)
    }
}

/// O tipo de cabo de um circuito. A temperatura da isolação escolhe a tabela de capacidade de
/// condução e o FCT (NBR 5410, tabelas 36, 37 e 40): 70 °C ou 90 °C.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoCabo {
    #[default]
    FlexivelPvc70,
    UnipolarPvc70,
    HeprXlpe90,
    Lszh70,
    LszhHepr90,
}

impl TipoCabo {
    pub const TODOS: [TipoCabo; 5] = [Self::FlexivelPvc70, Self::UnipolarPvc70, Self::HeprXlpe90, Self::Lszh70, Self::LszhHepr90];

    /// O código gravado no XDATA.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::FlexivelPvc70 => "FLEX_PVC_70",
            Self::UnipolarPvc70 => "UNI_PVC_70",
            Self::HeprXlpe90 => "HEPR_XLPE_90",
            Self::Lszh70 => "LSZH_70",
            Self::LszhHepr90 => "LSZH_HEPR_90",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::FlexivelPvc70 => "Cabo flexível PVC 70 °C (450/750 V)",
            Self::UnipolarPvc70 => "Cabo unipolar PVC 70 °C (0,6/1 kV)",
            Self::HeprXlpe90 => "Cabo HEPR / XLPE 90 °C (0,6/1 kV)",
            Self::Lszh70 => "Cabo livre de halogênio LSZH 70 °C",
            Self::LszhHepr90 => "Cabo livre de halogênio LSZH/HEPR 90 °C",
        }
    }

    /// O nome curto, para tabelas.
    pub fn abreviado(self) -> &'static str {
        match self {
            Self::FlexivelPvc70 => "PVC flex. 70°",
            Self::UnipolarPvc70 => "PVC unip. 70°",
            Self::HeprXlpe90 => "HEPR/XLPE 90°",
            Self::Lszh70 => "LSZH 70°",
            Self::LszhHepr90 => "LSZH/HEPR 90°",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }

    /// A isolação que escolhe a tabela de cálculo: a de 70 °C ou a de 90 °C.
    pub fn isolacao(self) -> Isolacao {
        match self {
            Self::FlexivelPvc70 | Self::UnipolarPvc70 | Self::Lszh70 => Isolacao::Pvc,
            Self::HeprXlpe90 | Self::LszhHepr90 => Isolacao::Epr,
        }
    }

    /// O cabo equivalente a uma isolação gravada antes de existir o tipo de cabo.
    pub fn de_isolacao(i: Isolacao) -> Self {
        if i == Isolacao::Pvc { Self::FlexivelPvc70 } else { Self::HeprXlpe90 }
    }

    /// É um cabo de 0,6/1 kV: o diâmetro externo vem dessa coluna da tabela de condutores.
    pub fn de_06_1kv(self) -> bool {
        matches!(self, Self::UnipolarPvc70 | Self::HeprXlpe90 | Self::LszhHepr90)
    }
}

/// O esquema de condutores de um circuito.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Esquema {
    /// Fase e neutro.
    Fn,
    /// Fase, neutro e proteção.
    Fnt,
    /// Duas fases e proteção.
    Fft,
    /// Duas fases, neutro e proteção.
    Ffnt,
    /// Três fases e proteção.
    Ffft,
    /// Três fases, neutro e proteção.
    Fffnt,
}

/// A curva de disparo de um disjuntor (ABNT NBR NM 60898).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Curva {
    B,
    #[default]
    C,
    D,
}

impl Curva {
    pub const TODAS: [Curva; 3] = [Self::B, Self::C, Self::D];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

impl Esquema {
    pub const TODOS: [Esquema; 6] = [Self::Fn, Self::Fnt, Self::Fft, Self::Ffnt, Self::Ffft, Self::Fffnt];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Fn => "F+N",
            Self::Fnt => "F+N+T",
            Self::Fft => "2F+T",
            Self::Ffnt => "2F+N+T",
            Self::Ffft => "3F+T",
            Self::Fffnt => "3F+N+T",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim();
        Self::TODOS.into_iter().find(|e| e.codigo().eq_ignore_ascii_case(t))
    }

    /// O esquema de quem não escolheu, pelas fases dos pontos.
    pub fn padrao(fases: u8) -> Self {
        match fases {
            0 | 1 => Self::Fnt,
            2 => Self::Fft,
            _ => Self::Fffnt,
        }
    }

    pub fn fases(self) -> u8 {
        match self {
            Self::Fn | Self::Fnt => 1,
            Self::Fft | Self::Ffnt => 2,
            Self::Ffft | Self::Fffnt => 3,
        }
    }

    pub fn neutro(self) -> bool {
        !matches!(self, Self::Fft | Self::Ffft)
    }

    /// Condutores carregados (NBR 5410, tabela 46): duas fases com neutro são três.
    pub fn carregados(self) -> u8 {
        match self {
            Self::Fn | Self::Fnt | Self::Fft => 2,
            Self::Ffnt | Self::Ffft | Self::Fffnt => 3,
        }
    }

    pub fn protecao(self) -> bool {
        self != Self::Fn
    }

    /// Condutores no eletroduto, para o levantamento de material.
    pub fn condutores(self) -> u8 {
        self.fases() + u8::from(self.neutro()) + u8::from(self.protecao())
    }
}

/// Tabela 42, condutores agrupados em feixe ou num mesmo eletroduto: (circuitos, fator).
const FCA_AGRUPAMENTO: [(usize, f64); 12] =
    [(1, 1.00), (2, 0.80), (3, 0.70), (4, 0.65), (5, 0.60), (6, 0.57), (7, 0.54), (8, 0.52), (11, 0.50), (15, 0.45), (19, 0.41), (usize::MAX, 0.38)];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Tensão fase-neutro, usada quando o circuito monofásico não informa tensão (V).
    pub tensao_fn: f64,
    /// Tensão fase-fase, usada quando o circuito de 2 ou 3 fases não informa tensão (V).
    pub tensao_ff: f64,
    /// Fator de correção de temperatura (tabela 40).
    pub fct: f64,
    /// Fator de correção de agrupamento (tabela 42).
    pub fca: f64,
    /// Fases do quadro quando o desenho não diz (sistema de distribuição do projeto).
    pub fases_padrao: u8,
    /// Queda de tensão admissível no circuito terminal (%).
    pub queda_admissivel: f64,
    /// Isolação dos condutores: escolhe a tabela de capacidade e a resistividade.
    pub isolacao: Isolacao,
    /// Condutores carregados pelo esquema escolhido. `None`: pelo número de fases.
    pub carregados: Option<u8>,
    /// O método de instalação: a coluna das tabelas 36 a 39.
    pub metodo: MetodoInstalacao,
    /// Como escolher o disjuntor entre Ib e Iz.
    pub criterio: CriterioDisjuntor,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            tensao_fn: 127.0,
            tensao_ff: 220.0,
            fct: 1.0,
            fca: 1.0,
            fases_padrao: 2,
            queda_admissivel: 4.0,
            isolacao: Isolacao::Pvc,
            carregados: None,
            metodo: MetodoInstalacao::B1,
            criterio: CriterioDisjuntor::CorrenteProjetoIb,
        }
    }
}

/// Seção mínima pela tabela 47: 1,5 mm² em iluminação, 2,5 mm² em força.
pub fn secao_minima(natureza: Natureza) -> f64 {
    if natureza == Natureza::Iluminacao { 1.5 } else { 2.5 }
}

/// Corrente de projeto Ib (A). Trifásico usa a tensão de linha.
pub fn corrente_projeto(potencia_va: f64, tensao: f64, fases: u8) -> f64 {
    if tensao <= 0.0 {
        return 0.0;
    }
    if fases >= 3 { potencia_va / (3f64.sqrt() * tensao) } else { potencia_va / tensao }
}

/// Condutores carregados: F+N ou F+F são dois, trifásico são três.
pub fn condutores_carregados(fases: u8) -> u8 {
    if fases >= 3 { 3 } else { 2 }
}

/// Condutores no eletroduto para o levantamento de material: fases, neutro (monofásico e
/// trifásico) e proteção.
pub fn condutores_total(fases: u8) -> u8 {
    if fases >= 3 { 5 } else { 3 }
}

/// Capacidade de condução pelo método B1 (tabela 36 ou 37) da seção de índice `i` em [`SECOES`].
pub fn capacidade(i: usize, carregados: u8, isolacao: Isolacao) -> Option<f64> {
    capacidade_no_metodo(i, carregados, isolacao, MetodoInstalacao::B1)
}

/// Capacidade de condução (tabelas 36 a 39) da seção de índice `i` em [`SECOES`] no método de instalação.
/// `None`: fora da tabela.
pub fn capacidade_no_metodo(i: usize, carregados: u8, isolacao: Isolacao, metodo: MetodoInstalacao) -> Option<f64> {
    let tabela = match (isolacao, carregados >= 3) {
        (Isolacao::Pvc, false) => &IZ_PVC_2,
        (Isolacao::Pvc, true) => &IZ_PVC_3,
        (_, false) => &IZ_EPR_2,
        (_, true) => &IZ_EPR_3,
    };
    tabela.get(i).and_then(|l| l.get(metodo.coluna())).copied().filter(|v| v.is_finite())
}

pub fn indice_secao(secao: f64) -> Option<usize> {
    SECOES.iter().position(|s| (s - secao).abs() < 1e-6)
}

/// Fator de agrupamento da tabela 42 para `n` circuitos no mesmo eletroduto.
pub fn fca_agrupamento(n: usize) -> f64 {
    FCA_AGRUPAMENTO.iter().find(|(ate, _)| n <= *ate).map_or(0.38, |(_, f)| *f)
}

/// A tensão do circuito: a dos pontos ou a do sistema (fase-neutro ou fase-fase). O `bool` diz
/// se veio dos pontos.
pub fn tensao_circuito(c: &Circuito, cfg: &Config) -> (f64, bool) {
    match c.tensao {
        Some(v) => (v, true),
        None if c.fases >= 2 => (cfg.tensao_ff, false),
        None => (cfg.tensao_fn, false),
    }
}

/// Queda de tensão (%) para o momento elétrico `momento` (soma de corrente × comprimento, A·m)
/// na seção `secao`: 2ρ·M/S em circuitos de uma ou duas fases, √3ρ·M/S em trifásicos, com ρ do
/// cobre na temperatura de serviço da isolação.
pub fn queda_percentual(momento: f64, secao: f64, tensao: f64, fases: u8, isolacao: Isolacao) -> f64 {
    if secao <= 0.0 || tensao <= 0.0 {
        return 0.0;
    }
    let k = if fases >= 3 { 3f64.sqrt() } else { 2.0 };
    k * isolacao.resistividade() * momento / secao / tensao * 100.0
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimensionamento {
    pub tensao: f64,
    /// Corrente de projeto (A).
    pub ib: f64,
    /// Corrente de projeto corrigida (A): I'b = Ib / (FCT × FCA).
    pub ib_corrigida: f64,
    /// Seção do condutor (mm²). `None`: a corrente passa da tabela.
    pub secao: Option<f64>,
    /// Capacidade de condução já corrigida por FCT e FCA (A).
    pub iz: Option<f64>,
    /// Corrente nominal do disjuntor (A).
    pub disjuntor: Option<f64>,
    /// Queda de tensão na seção escolhida (%), quando o comprimento é conhecido.
    pub queda: Option<f64>,
    pub avisos: Vec<String>,
}

/// I'b = Ib / (FCT × FCA).
pub fn corrente_corrigida(ib: f64, cfg: &Config) -> f64 {
    let fator = cfg.fct * cfg.fca;
    if fator > 0.0 { ib / fator } else { ib }
}

/// A menor seção e o disjuntor para uma corrente, sem queda de tensão: o alimentador do quadro.
/// Devolve (seção, Iz corrigida, disjuntor). `None`: a corrente passa da tabela.
pub fn secao_para_corrente(ib: f64, carregados: u8, cfg: &Config) -> Option<(f64, f64, f64)> {
    let fator = cfg.fct * cfg.fca;
    SECOES.iter().copied().enumerate().filter(|(_, s)| *s >= 2.5).find_map(|(i, secao)| {
        let iz = capacidade_no_metodo(i, carregados, cfg.isolacao, cfg.metodo)? * fator;
        let disjuntor = cfg.criterio.escolher(ib, iz)?;
        Some((secao, iz, disjuntor))
    })
}

/// Escolhe seção e disjuntor só pela corrente (sem comprimento).
pub fn dimensionar(c: &Circuito, cfg: &Config) -> Dimensionamento {
    dimensionar_com(c, cfg, None)
}

/// Escolhe seção e disjuntor: a menor seção (acima da mínima) que conduz Ib corrigida, que tem
/// um disjuntor com Ib ≤ In ≤ Iz e, com o momento elétrico conhecido, cuja queda de tensão fica
/// dentro do admissível.
pub fn dimensionar_com(c: &Circuito, cfg: &Config, momento: Option<f64>) -> Dimensionamento {
    let mut avisos = Vec::new();
    let (tensao, informada) = tensao_circuito(c, cfg);
    if !informada {
        avisos.push(format!("circuito sem tensão nos pontos: usando {tensao} V do sistema"));
    }
    let ib = corrente_projeto(c.potencia, tensao, c.fases);
    let carregados = cfg.carregados.unwrap_or_else(|| condutores_carregados(c.fases));
    let fator = cfg.fct * cfg.fca;
    let minima = secao_minima(c.natureza);
    for (i, secao) in SECOES.iter().copied().enumerate().filter(|(_, s)| *s >= minima) {
        let Some(iz) = capacidade_no_metodo(i, carregados, cfg.isolacao, cfg.metodo).map(|v| v * fator) else { continue };
        if iz < ib {
            continue;
        }
        // Ib ≤ In ≤ Iz; se nenhum disjuntor cabe nesta seção, a próxima seção resolve. Entre os que cabem, o
        // critério escolhe o menor (pela carga) ou o maior (pelo cabo).
        let Some(disjuntor) = cfg.criterio.escolher(ib, iz) else { continue };
        let queda = momento.map(|m| queda_percentual(m, secao, tensao, c.fases, cfg.isolacao));
        if queda.is_some_and(|q| q > cfg.queda_admissivel) {
            continue;
        }
        return Dimensionamento {
            tensao,
            ib,
            ib_corrigida: corrente_corrigida(ib, cfg),
            secao: Some(secao),
            iz: Some(iz),
            disjuntor: Some(disjuntor),
            queda,
            avisos,
        };
    }
    avisos.push("nenhuma seção até 120 mm² atende corrente e queda de tensão".into());
    Dimensionamento { tensao, ib, ib_corrigida: corrente_corrigida(ib, cfg), secao: None, iz: None, disjuntor: None, queda: None, avisos }
}

/// Confere uma escolha feita à mão (seção e disjuntor sobrescritos): calcula Iz e queda e avisa
/// o que a norma não aceita, sem impedir.
pub fn verificar(c: &Circuito, cfg: &Config, momento: Option<f64>, secao: f64, disjuntor: f64) -> Dimensionamento {
    let mut avisos = Vec::new();
    let (tensao, _) = tensao_circuito(c, cfg);
    let ib = corrente_projeto(c.potencia, tensao, c.fases);
    let iz = indice_secao(secao)
        .and_then(|i| capacidade_no_metodo(i, cfg.carregados.unwrap_or_else(|| condutores_carregados(c.fases)), cfg.isolacao, cfg.metodo))
        .map(|v| v * cfg.fct * cfg.fca);
    let queda = momento.map(|m| queda_percentual(m, secao, tensao, c.fases, cfg.isolacao));
    if secao < secao_minima(c.natureza) {
        avisos.push(format!("seção abaixo da mínima da tabela 47 ({} mm²)", secao_minima(c.natureza)));
    }
    if disjuntor < ib {
        avisos.push("disjuntor menor que a corrente de projeto (In < Ib)".into());
    }
    match iz {
        Some(iz) if disjuntor > iz => avisos.push("disjuntor maior que a capacidade do condutor (In > Iz)".into()),
        Some(_) => {}
        None => avisos.push("seção fora da tabela implementada".into()),
    }
    if queda.is_some_and(|q| q > cfg.queda_admissivel) {
        avisos.push(format!("queda de tensão acima de {} %", cfg.queda_admissivel));
    }
    Dimensionamento { tensao, ib, ib_corrigida: corrente_corrigida(ib, cfg), secao: Some(secao), iz, disjuntor: Some(disjuntor), queda, avisos }
}
