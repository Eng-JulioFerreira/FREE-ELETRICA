//! Configurações gerais do projeto (a janela Configurações do painel Elétrica). Delas saem os
//! parâmetros do dimensionamento: nada disso é digitado como fator solto.

use serde::{Deserialize, Serialize};

use crate::nbr5410::Config;

/// Sistema de distribuição da instalação.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sistema {
    /// F+N, 127 V.
    Monofasico127,
    /// 2F+N, 220/127 V: o mais comum em residências.
    #[default]
    Bifasico220127,
    /// 3F+N, 220/127 V.
    Trifasico220127,
    /// 3F+N, 380/220 V.
    Trifasico380220,
}

impl Sistema {
    pub const TODOS: [Sistema; 4] = [Self::Monofasico127, Self::Bifasico220127, Self::Trifasico220127, Self::Trifasico380220];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Monofasico127 => "Monofásico 127 V (F+N)",
            Self::Bifasico220127 => "Bifásico 220/127 V (2F+N)",
            Self::Trifasico220127 => "Trifásico 220/127 V (3F+N)",
            Self::Trifasico380220 => "Trifásico 380/220 V (3F+N)",
        }
    }

    /// Tensão fase-neutro (V).
    pub fn tensao_fn(self) -> f64 {
        match self {
            Self::Trifasico380220 => 220.0,
            _ => 127.0,
        }
    }

    /// Tensão fase-fase (V). Monofásico não tem.
    pub fn tensao_ff(self) -> Option<f64> {
        match self {
            Self::Monofasico127 => None,
            Self::Bifasico220127 | Self::Trifasico220127 => Some(220.0),
            Self::Trifasico380220 => Some(380.0),
        }
    }

    pub fn fases(self) -> u8 {
        match self {
            Self::Monofasico127 => 1,
            Self::Bifasico220127 => 2,
            Self::Trifasico220127 | Self::Trifasico380220 => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Setup {
    pub sistema: Sistema,
    /// Temperatura ambiente dos condutores (°C), de 10 a 60.
    pub temperatura_ambiente: f64,
}

impl Default for Setup {
    fn default() -> Self {
        Setup { sistema: Sistema::default(), temperatura_ambiente: 30.0 }
    }
}

/// Tabela 40, isolação PVC, temperatura ambiente (não no solo): (°C, fator).
const FCT_PVC: [(f64, f64); 11] = [
    (10.0, 1.22),
    (15.0, 1.17),
    (20.0, 1.12),
    (25.0, 1.06),
    (30.0, 1.00),
    (35.0, 0.94),
    (40.0, 0.87),
    (45.0, 0.79),
    (50.0, 0.71),
    (55.0, 0.61),
    (60.0, 0.50),
];

impl Setup {
    /// Fator de correção de temperatura (tabela 40, PVC). Entre duas linhas da tabela vale a
    /// temperatura de cima, que é o lado seguro.
    pub fn fct(&self) -> f64 {
        let t = self.temperatura_ambiente;
        FCT_PVC.iter().find(|(limite, _)| t <= *limite + 1e-9).map_or(FCT_PVC[FCT_PVC.len() - 1].1, |(_, f)| *f)
    }

    /// Os parâmetros do dimensionamento. O FCA fica 1 até o agrupamento ser lido dos
    /// eletrodutos.
    pub fn config(&self) -> Config {
        Config { tensao_fn: self.sistema.tensao_fn(), fct: self.fct(), fca: 1.0, fases_padrao: self.sistema.fases() }
    }
}
