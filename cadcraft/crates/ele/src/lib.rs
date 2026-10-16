//! CRAFT ELE: projeto elétrico de baixa tensão (NBR 5410) sobre o desenho do CADCraft.
//!
//! Lê os pontos elétricos (blocos com atributos), agrupa em circuitos e dimensiona o quadro.
//! Não depende da interface: o painel Elétrica do `cadcraft-ui-egui` só mostra o que está aqui.

pub mod circuito;
pub mod nbr5410;
pub mod ponto;
pub mod quadro;

#[cfg(test)]
mod tests;

use cadcraft_doc::Drawing;
use serde::Serialize;

pub use circuito::{Circuito, Natureza};
pub use nbr5410::Config;
pub use ponto::{PontoEletrico, TipoPonto};
pub use quadro::QuadroDeCargas;

/// O que o desenho tem de instalação elétrica.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analise {
    pub pontos: Vec<PontoEletrico>,
    pub circuitos: Vec<Circuito>,
}

impl Analise {
    pub fn contar(&self, tipo: TipoPonto) -> usize {
        self.pontos.iter().filter(|p| p.tipo == tipo).count()
    }

    /// Avisos de preenchimento de cada ponto: (rótulo do ponto, aviso).
    pub fn avisos_pontos(&self) -> Vec<(String, String)> {
        self.pontos.iter().flat_map(|p| p.avisos().into_iter().map(move |a| (p.rotulo(), a))).collect()
    }
}

pub fn analisar(doc: &Drawing) -> Analise {
    let pontos = ponto::ler_pontos(doc);
    let circuitos = circuito::agrupar(&pontos);
    Analise { pontos, circuitos }
}

pub fn dimensionar_quadro(analise: &Analise, cfg: &Config) -> QuadroDeCargas {
    quadro::montar(analise, cfg)
}
