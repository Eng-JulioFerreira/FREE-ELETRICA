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
    pub disjuntores: Vec<ItemContado>,
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
    let mut disjuntores: Vec<f64> = Vec::new();
    let mut grupos: Vec<String> = Vec::new();
    for q in analise.quadros() {
        match quadro::rotear(doc, analise, setup, q.handle) {
            Ok(r) => {
                for l in &r.linhas {
                    if let Some(secao) = l.escolha.secao {
                        let item = cabos.entry((chave(secao), l.nome_cabo.clone())).or_insert_with(|| ItemCabo {
                            secao,
                            tipo: l.tipo_cabo,
                            nome: l.nome_cabo.clone(),
                            metros: 0.0,
                        });
                        item.metros += l.cabo;
                    }
                    disjuntores.extend(l.escolha.disjuntor);
                }
                disjuntores.extend(r.alimentador.disjuntor);
                grupos.extend(r.linhas.iter().filter_map(|l| l.grupo_dr.as_ref().map(|g| format!("{}: {g}", r.tag))));
            }
            Err(e) => m.avisos.push(e),
        }
    }
    disjuntores.sort_by(f64::total_cmp);
    m.disjuntores = contar_disjuntores(&disjuntores);
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

fn contar_disjuntores(ordenados: &[f64]) -> Vec<ItemContado> {
    let mut v: Vec<ItemContado> = Vec::new();
    for d in ordenados {
        let nome = format!("{} A", crate::ponto::br(*d));
        match v.last_mut() {
            Some(u) if u.nome == nome => u.quantidade += 1,
            _ => v.push(ItemContado { nome, quantidade: 1 }),
        }
    }
    v
}
