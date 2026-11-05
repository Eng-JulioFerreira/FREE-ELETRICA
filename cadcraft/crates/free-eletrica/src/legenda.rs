// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A Legenda de Símbolos da prancha: um bloco `LEGENDA_SIMBOLOS` com o símbolo e o nome de cada tipo de
//! ponto usado no desenho. O nome vem do atributo `LEGENDA` do ponto ou da família. Regenerar troca o
//! conteúdo do bloco e mantém a inserção.

use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space};

use crate::Analise;
use crate::tabela::{CAMADA, linha, texto};

/// O bloco da legenda.
pub const BLOCO: &str = "LEGENDA_SIMBOLOS";
const ALTURA: f64 = 0.10;
/// A altura de cada linha e a largura da coluna do símbolo (m).
const LINHA: f64 = 0.70;
const COLUNA_SIMBOLO: f64 = 0.80;
const TITULO: f64 = 0.35;

/// A legenda do desenho: a inserção dela.
pub fn inserida(doc: &Drawing) -> Option<Handle> {
    doc.model.iter().find(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block == BLOCO)).map(|e| e.handle)
}

/// As linhas da legenda: (bloco, nome), sem repetir, na ordem do tipo de ponto e do nome.
pub fn linhas(analise: &Analise) -> Vec<(String, String)> {
    let mut v: Vec<(crate::ponto::TipoPonto, String, String)> = analise
        .pontos
        .iter()
        .map(|p| (p.tipo, p.bloco.clone(), p.legenda.clone().unwrap_or_else(|| format!("{} ({})", p.tipo.nome(), p.bloco))))
        .collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|(_, b, n)| (b, n)).collect()
}

fn pecas(analise: &Analise, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let lista = linhas(analise);
    let maior = lista.iter().map(|(_, n)| n.chars().count()).max().unwrap_or(0).max(20);
    let largura = COLUNA_SIMBOLO + maior as f64 * ALTURA * 0.8 + 2.0 * ALTURA;
    let fundo = -TITULO - LINHA * lista.len() as f64;
    let mut p = vec![texto("LEGENDA DE SÍMBOLOS", [u(largura / 2.0), u(-TITULO / 2.0)], u(ALTURA * 1.4), HAlign::Center)];
    for (a, b) in [([0.0, 0.0], [largura, 0.0]), ([largura, 0.0], [largura, fundo]), ([largura, fundo], [0.0, fundo]), ([0.0, fundo], [0.0, 0.0])] {
        p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
    }
    for (i, (bloco, nome)) in lista.iter().enumerate() {
        let y = -TITULO - LINHA * i as f64;
        p.push(linha([0.0, u(y)], [u(largura), u(y)]));
        p.push(linha([u(COLUNA_SIMBOLO), u(y)], [u(COLUNA_SIMBOLO), u(y - LINHA)]));
        p.push(EntityKind::Insert(Insert {
            block: bloco.clone(),
            insert: Vec3::new(u(COLUNA_SIMBOLO / 2.0), u(y - LINHA + 0.12), 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            rotation: 0.0,
            attribs: Vec::new(),
            cols: 1,
            rows: 1,
            col_spacing: 0.0,
            row_spacing: 0.0,
        }));
        p.push(texto(nome, [u(COLUNA_SIMBOLO + ALTURA), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Left));
    }
    p
}

fn gravar_bloco(doc: &mut Drawing, analise: &Analise, m: f64) {
    let mut b = Block::new(BLOCO);
    for k in pecas(analise, m) {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(BLOCO.to_string(), Arc::new(b));
}

/// Gera a legenda em `pos`. Se o desenho já tem uma, ela é refeita e movida para lá.
pub fn inserir(doc: &mut Drawing, analise: &Analise, pos: [f64; 2], metros: f64) -> Result<Handle, String> {
    if !pos.iter().all(|v| v.is_finite()) {
        return Err("posição inválida".into());
    }
    let m = if metros > 0.0 { metros } else { 1.0 };
    gravar_bloco(doc, analise, m);
    if let Some(h) = inserida(doc)
        && let Some(mut e) = doc.model.get(h).map(|e| (**e).clone())
    {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert = Vec3::new(pos[0], pos[1], 0.0);
        }
        doc.model.replace(e);
        return Ok(h);
    }
    let ins = Insert {
        block: BLOCO.into(),
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

/// Refaz a legenda do desenho, sem mexer na inserção. Devolve quantas refez.
pub fn regenerar(doc: &mut Drawing, analise: &Analise, metros: f64) -> usize {
    if inserida(doc).is_none() {
        return 0;
    }
    gravar_bloco(doc, analise, if metros > 0.0 { metros } else { 1.0 });
    1
}
