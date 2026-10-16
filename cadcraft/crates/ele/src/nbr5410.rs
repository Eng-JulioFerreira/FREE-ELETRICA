//! Dimensionamento de circuitos terminais pela NBR 5410 (primeira versão).
//!
//! O que está aqui: corrente de projeto, seção mínima (tabela 47), capacidade de condução
//! (tabela 36, cobre, PVC 70 °C, método de referência B1) com fatores de correção informados,
//! e a escolha do disjuntor com Ib ≤ In ≤ Iz (5.3.4.1).
//!
//! Ainda não: queda de tensão (precisa do comprimento dos trechos), fator de agrupamento
//! calculado pelos eletrodutos, outros métodos de instalação e isolações, e seções acima de
//! 120 mm². Os valores das tabelas devem ser conferidos contra a norma vigente.

use serde::Serialize;

use crate::circuito::{Circuito, Natureza};

/// Seções nominais de condutor, em mm².
pub const SECOES: [f64; 12] = [1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0];

/// Tabela 36, método B1, cobre, PVC, 30 °C: dois condutores carregados (A).
const IZ_B1_DOIS: [f64; 12] = [17.5, 24.0, 32.0, 41.0, 57.0, 76.0, 101.0, 125.0, 151.0, 192.0, 232.0, 269.0];
/// Tabela 36, método B1, cobre, PVC, 30 °C: três condutores carregados (A).
const IZ_B1_TRES: [f64; 12] = [15.5, 21.0, 28.0, 36.0, 50.0, 68.0, 89.0, 110.0, 134.0, 171.0, 207.0, 239.0];

/// Correntes nominais de disjuntores termomagnéticos (A).
pub const DISJUNTORES: [f64; 12] = [6.0, 10.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Tensão fase-neutro, usada quando o circuito não informa tensão (V).
    pub tensao_fn: f64,
    /// Fator de correção de temperatura (tabela 40).
    pub fct: f64,
    /// Fator de correção de agrupamento (tabela 42).
    pub fca: f64,
    /// Fases do quadro quando o desenho não diz (sistema de distribuição do projeto).
    pub fases_padrao: u8,
}

impl Default for Config {
    fn default() -> Self {
        Config { tensao_fn: 127.0, fct: 1.0, fca: 1.0, fases_padrao: 2 }
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

/// Capacidade de condução da tabela 36 (B1) para a seção de índice `i`.
pub fn capacidade(i: usize, carregados: u8) -> f64 {
    if carregados >= 3 { IZ_B1_TRES[i] } else { IZ_B1_DOIS[i] }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimensionamento {
    pub tensao: f64,
    /// Corrente de projeto (A).
    pub ib: f64,
    /// Seção do condutor (mm²). `None`: a corrente passa da tabela.
    pub secao: Option<f64>,
    /// Capacidade de condução já corrigida por FCT e FCA (A).
    pub iz: Option<f64>,
    /// Corrente nominal do disjuntor (A).
    pub disjuntor: Option<f64>,
    pub avisos: Vec<String>,
}

pub fn dimensionar(c: &Circuito, cfg: &Config) -> Dimensionamento {
    let mut avisos = Vec::new();
    let tensao = c.tensao.unwrap_or_else(|| {
        avisos.push(format!("circuito sem tensão informada: usando {} V", cfg.tensao_fn));
        cfg.tensao_fn
    });
    let ib = corrente_projeto(c.potencia, tensao, c.fases);
    let carregados = condutores_carregados(c.fases);
    let fator = cfg.fct * cfg.fca;
    let minima = secao_minima(c.natureza);
    for (i, secao) in SECOES.iter().copied().enumerate().filter(|(_, s)| *s >= minima) {
        let iz = capacidade(i, carregados) * fator;
        if iz < ib {
            continue;
        }
        // Ib ≤ In ≤ Iz; se nenhum disjuntor cabe nesta seção, a próxima seção resolve.
        if let Some(disjuntor) = DISJUNTORES.iter().copied().find(|d| *d >= ib && *d <= iz) {
            return Dimensionamento { tensao, ib, secao: Some(secao), iz: Some(iz), disjuntor: Some(disjuntor), avisos };
        }
    }
    avisos.push("corrente acima da tabela implementada (até 120 mm²)".into());
    Dimensionamento { tensao, ib, secao: None, iz: None, disjuntor: None, avisos }
}
