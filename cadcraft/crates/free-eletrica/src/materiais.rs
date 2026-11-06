// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A lista de materiais: cabos por seção e tipo, eletrodutos por tipo e diâmetro, caixas, pontos e
//! disjuntores, lidos do desenho.

use std::collections::BTreeMap;

use cadcraft_doc::Drawing;
use serde::Serialize;

use crate::nbr5410::TipoCabo;
use crate::rota::TipoEletroduto;
use crate::setup::Setup;
use crate::{Analise, fiacao, quadro};

/// Metros de cabo de uma seção e um tipo.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCabo {
    pub secao: f64,
    pub tipo: TipoCabo,
    /// O nome do cabo no catálogo.
    pub nome: String,
    pub metros: f64,
}

/// Metros de eletroduto de um tipo e um diâmetro. Sem fiação não há diâmetro calculado.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEletroduto {
    pub tipo: TipoEletroduto,
    pub diametro: Option<f64>,
    pub metros: f64,
}

/// Quantidade de um item.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemContado {
    pub nome: String,
    pub quantidade: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Materiais {
    pub cabos: Vec<ItemCabo>,
    pub eletrodutos: Vec<ItemEletroduto>,
    pub caixas: Vec<ItemContado>,
    pub pontos: Vec<ItemContado>,
    /// Os pontos pela descrição comercial, a do bloco ou a da família.
    pub conjuntos: Vec<ItemContado>,
    /// Disjuntores por polos, corrente e curva: os dos circuitos, o geral e o do DPS.
    pub disjuntores: Vec<ItemContado>,
    /// Os outros componentes dos quadros: DR, DPS, barramentos e as caixas com a capacidade em módulos DIN.
    pub quadros: Vec<ItemContado>,
    /// Circuitos por grupo de DR.
    pub grupos_dr: Vec<ItemContado>,
    /// Pontos sem caixa física: blocos sem família, que não dizem qual é.
    pub pontos_sem_caixa: usize,
    /// Eletrodutos soltos, fora da metragem.
    pub eletrodutos_soltos: usize,
    /// Cabos de telecom por nome (NBR 14565): o comprimento de cada enlace, com as folgas, vezes os cabos dele.
    pub cabos_telecom: Vec<crate::telecom::MetrosDeCabo>,
    /// Terminações de telecom: no ponto e as portas no rack.
    pub telecom: Vec<ItemContado>,
    pub avisos: Vec<String>,
}

/// Conta por nome, em ordem alfabética.
fn contar(nomes: impl Iterator<Item = String>) -> Vec<ItemContado> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for n in nomes {
        *m.entry(n).or_default() += 1;
    }
    m.into_iter().map(|(nome, quantidade)| ItemContado { nome, quantidade }).collect()
}

/// Levanta os materiais do desenho.
pub fn levantar(doc: &Drawing, analise: &Analise, setup: &Setup) -> Materiais {
    let mut m = Materiais::default();
    // Chaves numéricas em milésimos: f64 não ordena num mapa.
    let chave = |v: f64| (v * 1000.0).round() as i64;
    let mut cabos: BTreeMap<(i64, String), ItemCabo> = BTreeMap::new();
    let mut disjuntores: Vec<String> = Vec::new();
    let mut componentes: Vec<String> = Vec::new();
    let mut grupos: Vec<String> = Vec::new();
    for q in analise.quadros() {
        match quadro::rotear(doc, analise, setup, q.handle) {
            Ok(r) => {
                for l in &r.linhas {
                    // Fases, neutro e PE, cada um na seção dele (tabelas 48 e 58).
                    for (secao, metros) in &l.cabo_por_secao {
                        let item = cabos.entry((chave(*secao), l.nome_cabo.clone())).or_insert_with(|| ItemCabo {
                            secao: *secao,
                            tipo: l.tipo_cabo,
                            nome: l.nome_cabo.clone(),
                            metros: 0.0,
                        });
                        item.metros += metros;
                    }
                    disjuntores.extend(l.escolha.disjuntor.map(|d| nome_do_disjuntor(l.esquema.fases(), d, Some(l.curva))));
                }
                componentes_do_quadro(&r, &mut disjuntores, &mut componentes);
                grupos.extend(r.linhas.iter().filter_map(|l| l.grupo_dr.as_ref().map(|g| format!("{}: {g}", r.tag))));
            }
            Err(e) => m.avisos.push(e),
        }
    }
    m.disjuntores = contar(disjuntores.into_iter());
    m.quadros = contar(componentes.into_iter());
    m.grupos_dr = contar(grupos.into_iter());

    let (trechos, soltos, avisos) = fiacao::todos_os_trechos(doc, analise, setup);
    m.avisos.extend(avisos);
    m.eletrodutos_soltos = soltos.len();
    let mut dutos: BTreeMap<(&'static str, i64), ItemEletroduto> = BTreeMap::new();
    for t in &trechos {
        let diametro = t.eletroduto.as_ref().map(|e| e.nominal);
        let item = dutos.entry((t.tipo.codigo(), diametro.map_or(-1, chave))).or_insert(ItemEletroduto { tipo: t.tipo, diametro, metros: 0.0 });
        item.metros += t.comprimento;
    }
    m.eletrodutos = dutos.into_values().collect();
    // O condutor de proteção compartilhado não vai para a compra.
    for t in &trechos {
        for (secao, tipo) in &t.pe_removidos {
            if let Some(c) = cabos.values_mut().find(|c| chave(c.secao) == chave(*secao) && c.tipo == *tipo) {
                c.metros = (c.metros - t.comprimento * setup.fator_materiais).max(0.0);
            }
        }
    }
    m.cabos = cabos.into_values().collect();

    m.pontos = contar(analise.pontos.iter().map(|p| p.tipo.nome().to_string()));
    m.conjuntos = contar(analise.pontos.iter().filter_map(|p| p.desc_material.clone()));
    // Pontos empilhados na mesma caixa da planta: uma caixa física por altura (uma tomada a 0,30 m e um
    // interruptor a 1,20 m no mesmo lugar são duas caixas 4x2; duas tomadas a 0,30 m, uma só).
    let mut caixas: Vec<String> = Vec::new();
    let mut vistas: Vec<(cadcraft_doc::Handle, i64)> = Vec::new();
    for p in &analise.pontos {
        let Some(c) = p.caixa.filter(|c| *c != crate::familia::CaixaFisica::Nenhuma) else { continue };
        let grupo = p.host.filter(|h| analise.ponto(*h).is_some()).unwrap_or(p.handle);
        let altura = (crate::rota::altura(p, setup) * 100.0).round() as i64;
        if !vistas.contains(&(grupo, altura)) {
            vistas.push((grupo, altura));
            caixas.push(c.nome().to_string());
        }
    }
    m.caixas = contar(caixas.into_iter());
    m.pontos_sem_caixa = analise.pontos.iter().filter(|p| p.caixa.is_none()).count();
    let (cabos_telecom, itens) = crate::telecom::materiais(&crate::telecom::rotear(doc, analise, setup));
    m.cabos_telecom = cabos_telecom;
    m.telecom = itens.into_iter().map(|(nome, quantidade)| ItemContado { nome, quantidade }).collect();
    m
}

/// O nome de compra de um disjuntor: `Disjuntor 2P 20 A - curva C`.
pub fn nome_do_disjuntor(polos: u8, corrente: f64, curva: Option<crate::nbr5410::Curva>) -> String {
    let curva = curva.map_or(String::new(), |c| format!(" - curva {}", c.codigo()));
    format!("Disjuntor {}P {} A{curva}", polos.clamp(1, 3), crate::ponto::br(corrente))
}

/// O geral, o disjuntor do DPS, os DR, os DPS, os barramentos e a caixa de um quadro, pela montagem dele.
fn componentes_do_quadro(r: &quadro::ResumoQuadro, disjuntores: &mut Vec<String>, componentes: &mut Vec<String>) {
    use crate::frontal::TipoDispositivo as T;
    let a = &r.alimentador;
    let fases = r.fases_quadro.clamp(1, 3);
    disjuntores.extend(a.disjuntor.map(|d| nome_do_disjuntor(fases, d, Some(crate::nbr5410::Curva::C))));
    disjuntores.extend(a.disjuntor_dps.map(|d| nome_do_disjuntor(fases, d, Some(crate::nbr5410::Curva::C))));
    let mo = crate::frontal::montagem(r);
    for d in &mo.dispositivos {
        match d.tipo {
            T::Dr => componentes.push(format!("Interruptor DR {}P {} - 30 mA", d.modulos, d.topo.trim_start_matches("DR ").replace('A', " A"))),
            T::Dps => componentes.push(format!("DPS Classe {} - {} V", a.classe_dps.codigo(), crate::ponto::br(a.dps_uc.ceil()))),
            _ => {}
        }
    }
    componentes.push(format!("Barramento de neutro - {} bornes", mo.bornes_neutro));
    componentes.push(format!("Barramento de terra - {} bornes", mo.bornes_terra));
    let tipo = match mo.barramento_fase {
        quadro::TipoBarramentoFase::Pente => "pente",
        quadro::TipoBarramentoFase::EscamaDePeixe => "espinha de peixe",
    };
    for t in 0..mo.trilhos {
        let polos: u32 = mo.dispositivos.iter().filter(|d| d.trilho == t && matches!(d.tipo, T::Disjuntor | T::Dr)).map(|d| d.modulos).sum();
        if polos > 0 {
            componentes.push(format!("Barramento {tipo} {}F - {polos} polos", mo.fases));
        }
    }
    let capacidade = mo.modulos_por_trilho * u32::try_from(mo.trilhos).unwrap_or(1);
    let montagem = r.config.montagem.map_or(String::new(), |m| format!(" {}", m.nome()));
    componentes.push(format!("Quadro de distribuição{montagem} - {capacidade} módulos DIN ({} trilhos de {})", mo.trilhos, mo.modulos_por_trilho));
}
