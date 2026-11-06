// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! As disciplinas do projeto no mesmo grafo de eletrodutos. Cada uma diz qual é a central de onde os cabos
//! saem, quais pontos são dela, por quais blocos um cabo dela pode passar e a topologia:
//!
//! - **Elétrica** (NBR 5410): radial, do quadro aos pontos, com os circuitos se ramificando pelas caixas.
//! - **Telecom** (NBR 14565): estrela, um cabo inteiro do rack a cada ponto, sem emenda no caminho.
//! - **Alarme de incêndio** (NBR 17240): laço classe A (ida e volta por caminhos separados) ou classe B (o
//!   laço termina no último dispositivo), saindo da central. Fica preparado aqui: os blocos e a interface
//!   virão depois, e nenhum ponto é dela enquanto não houver família com o domínio de incêndio.

use serde::Serialize;

use crate::familia::{FamilyCategory, SystemDomain};
use crate::ponto::{PontoEletrico, TipoPonto};

/// Uma disciplina do projeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Disciplina {
    Eletrica,
    Telecom,
    AlarmeIncendio,
}

/// Como os cabos de uma disciplina saem da central.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Topologia {
    /// Circuitos que se ramificam do quadro até os pontos.
    Radial,
    /// Um cabo individual, inteiro, da central a cada ponto.
    Estrela,
    /// Um laço que sai da central e passa pelos dispositivos.
    Laco(ClasseLaco),
}

/// A classe de um laço de alarme (NBR 17240).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClasseLaco {
    /// O laço volta à central por outro caminho.
    A,
    /// O laço termina no último dispositivo.
    B,
}

impl Disciplina {
    pub const TODAS: [Disciplina; 3] = [Self::Eletrica, Self::Telecom, Self::AlarmeIncendio];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Eletrica => "Elétrica",
            Self::Telecom => "Telecom",
            Self::AlarmeIncendio => "Alarme de incêndio",
        }
    }

    /// A norma de cada disciplina.
    pub fn norma(self) -> &'static str {
        match self {
            Self::Eletrica => "ABNT NBR 5410",
            Self::Telecom => "ABNT NBR 14565",
            Self::AlarmeIncendio => "ABNT NBR 17240",
        }
    }

    /// A topologia padrão. No alarme, a classe do laço será escolha do projeto; a padrão é a A.
    pub fn topologia(self) -> Topologia {
        match self {
            Self::Eletrica => Topologia::Radial,
            Self::Telecom => Topologia::Estrela,
            Self::AlarmeIncendio => Topologia::Laco(ClasseLaco::A),
        }
    }

    /// A disciplina do domínio de uma família.
    pub fn do_dominio(d: SystemDomain) -> Option<Self> {
        match d {
            SystemDomain::Power => Some(Self::Eletrica),
            SystemDomain::DataTelecom => Some(Self::Telecom),
            SystemDomain::FireSecurity => Some(Self::AlarmeIncendio),
            SystemDomain::Infra => None,
        }
    }

    /// A disciplina de um ponto: a do domínio da família ou, sem família, a do tipo. As caixas de passagem são
    /// da infraestrutura: de nenhuma disciplina.
    pub fn do_ponto(p: &PontoEletrico) -> Option<Self> {
        if let Some(c) = p.categoria {
            return Self::do_dominio(c.dominio_padrao()).filter(|_| c != FamilyCategory::Caixa);
        }
        match p.tipo {
            TipoPonto::Dados | TipoPonto::Rack => Some(Self::Telecom),
            TipoPonto::Caixa => None,
            _ => Some(Self::Eletrica),
        }
    }

    /// O ponto é a central da disciplina: o quadro, o rack ou a central de alarme.
    pub fn e_central(self, p: &PontoEletrico) -> bool {
        match self {
            Self::Eletrica => p.tipo == TipoPonto::Quadro,
            Self::Telecom => p.tipo == TipoPonto::Rack,
            // A central de alarme ainda não tem família: um quadro do domínio de incêndio fará esse papel.
            Self::AlarmeIncendio => Self::do_ponto(p) == Some(self) && p.tipo == TipoPonto::Quadro,
        }
    }

    /// O ponto recebe cabo da disciplina (e não é a central).
    pub fn e_ponto(self, p: &PontoEletrico) -> bool {
        Self::do_ponto(p) == Some(self) && !self.e_central(p)
    }

    /// Um cabo da disciplina pode passar pelo bloco no meio do caminho: as caixas de passagem e os pontos da
    /// própria disciplina. Um cabo de telecom não passa por uma caixa de tomada nem por um quadro elétrico.
    pub fn atravessa(self, tipo: TipoPonto) -> bool {
        match self {
            Self::Eletrica => !matches!(tipo, TipoPonto::Dados | TipoPonto::Rack),
            Self::Telecom => matches!(tipo, TipoPonto::Dados | TipoPonto::Caixa),
            Self::AlarmeIncendio => tipo == TipoPonto::Caixa,
        }
    }
}
