// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Inspeção do projeto: o estado de cada ponto e de cada eletroduto, para o modo de inspeção visual
//! e para a conferência sem interface.
//!
//! - Vermelho: ponto sem circuito, ou interruptor sem luminária vinculada.
//! - Laranja: ponto com circuito mas sem caminho de eletrodutos até o quadro; eletroduto solto.
//! - Amarelo: eletroduto com ocupação acima do limite; ponto de circuito com queda acima do limite.
//! - Verde: ponto conectado, dimensionado e válido.

use std::collections::HashMap;

use cadcraft_doc::{Drawing, Handle};
use serde::Serialize;

use crate::Analise;
use crate::fiacao::{self, letras};
use crate::ponto::TipoPonto;
use crate::quadro;
use crate::rota::{self, Rede};
use crate::setup::Setup;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Nivel {
    Vermelho,
    Laranja,
    Amarelo,
    Verde,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(serialize_with = "hex")]
    pub handle: Handle,
    pub pos: [f64; 2],
    pub nivel: Nivel,
    pub motivo: String,
    /// É um eletroduto (e não um bloco).
    pub eletroduto: bool,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

/// O estado de cada ponto (menos caixas) e os eletrodutos com problema, do mais grave ao menos.
pub fn inspecionar(doc: &Drawing, analise: &Analise, setup: &Setup) -> Vec<Item> {
    let (metros, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    let rede = Rede::montar(doc, setup, &analise.pontos, metros);
    // Circuito de cada quadro com queda acima do limite e pontos sem caminho até o quadro.
    let mut queda_alta: HashMap<(Handle, String), String> = HashMap::new();
    let mut sem_caminho: Vec<Handle> = Vec::new();
    for q in analise.quadros() {
        let Ok(r) = quadro::rotear(doc, analise, setup, q.handle) else { continue };
        let arvore = rede.no_de(q.handle).filter(|_| rede.conectado(q.handle)).map(|n| rede.arvore(n));
        for l in &r.linhas {
            if l.escolha.queda.is_some_and(|v| v > l.queda_admissivel) {
                queda_alta.insert(
                    (q.handle, l.circuito.id.to_uppercase()),
                    format!("queda de {:.2} % no circuito {}", l.escolha.queda.unwrap_or(0.0), l.circuito.id),
                );
            }
            for p in &l.circuito.pontos {
                let alcancavel = arvore.as_ref().zip(rede.no_de(*p)).is_some_and(|(a, n)| a.distancia.get(n).is_some_and(|d| d.is_finite()));
                if !alcancavel {
                    sem_caminho.push(*p);
                }
            }
        }
    }
    let mut itens = Vec::new();
    for p in analise.pontos.iter().filter(|p| p.tipo != TipoPonto::Caixa && p.tipo.e_de_forca()) {
        let item = |nivel, motivo: String| Item { handle: p.handle, pos: p.posicao, nivel, motivo, eletroduto: false };
        if p.tipo == TipoPonto::Quadro {
            itens.push(if rede.conectado(p.handle) {
                item(Nivel::Verde, "quadro ligado".into())
            } else {
                item(Nivel::Laranja, "nenhum eletroduto liga o quadro".into())
            });
            continue;
        }
        if p.circuito.is_none() {
            itens.push(item(Nivel::Vermelho, format!("{} sem circuito", p.tipo.nome())));
            continue;
        }
        if p.tipo == TipoPonto::Interruptor {
            let ls = p.comando.as_deref().map(letras).unwrap_or_default();
            let vinculada = analise.pontos.iter().any(|o| {
                o.tipo == TipoPonto::Iluminacao
                    && o.quadro == p.quadro
                    && o.comando.as_deref().is_some_and(|c| letras(c).iter().any(|l| ls.contains(l)))
            });
            if !vinculada {
                itens.push(item(Nivel::Vermelho, "interruptor sem luminária vinculada".into()));
                continue;
            }
        }
        if p.quadro.is_none() {
            itens.push(item(Nivel::Laranja, "ponto sem quadro".into()));
            continue;
        }
        if sem_caminho.contains(&p.handle) {
            // No caminho de um eletroduto o bloco já é uma caixa: só falta o corte (`eletroduto::encaixar`),
            // que a inspeção não faz porque não muda o desenho.
            let em_linha = doc.model.iter().any(|e| {
                setup.e_camada_eletroduto(&e.common.layer)
                    && rota::tracado(&e.kind, 0.005 / metros.max(1e-9))
                        .and_then(|t| rota::perto_do_tracado(&t, p.posicao))
                        .is_some_and(|(_, d, _)| d * metros <= crate::eletroduto::TOLERANCIA_EM_LINHA)
            });
            let motivo = if em_linha {
                "em linha com um eletroduto: Atualizar tags e tabelas liga os trechos"
            } else {
                "sem caminho de eletrodutos até o quadro"
            };
            itens.push(item(Nivel::Laranja, motivo.into()));
            continue;
        }
        if let Some(m) = p.quadro.zip(p.circuito.as_ref()).and_then(|(q, c)| queda_alta.get(&(q, c.to_uppercase()))) {
            itens.push(item(Nivel::Amarelo, m.clone()));
            continue;
        }
        itens.push(item(Nivel::Verde, "conectado, dimensionado e válido".into()));
    }
    let (trechos, soltos, _) = fiacao::todos_os_trechos(doc, analise, setup);
    for s in soltos {
        itens.push(Item { handle: s.eletroduto, pos: s.meio, nivel: Nivel::Laranja, motivo: format!("eletroduto: {}", s.motivo), eletroduto: true });
    }
    for t in trechos.iter().filter(|t| t.ocupacao_excedida()) {
        let oc = t.ocupacao.unwrap_or(0.0) * 100.0;
        itens.push(Item {
            handle: t.handle,
            pos: t.meio,
            nivel: Nivel::Amarelo,
            motivo: format!("eletroduto {} com ocupação de {oc:.0} %", t.tipo.abreviado()),
            eletroduto: true,
        });
    }
    itens.sort_by_key(|i| i.nivel);
    itens
}
