// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. CRAFT ELE: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Escritas do CRAFT ELE no desenho. Um valor vai para o atributo do bloco quando ele existe
//! (o projetista vê e edita) e, senão, para o XDATA do CRAFT ELE (viaja com o bloco, invisível).

use cadcraft_doc::{Drawing, EntityKind, Handle};

use crate::ponto::{TAGS_CIRCUITO, TAGS_COMANDO, TAGS_QUADRO, ler_ponto};
use crate::quadro::{CHAVE_DISJUNTOR, CHAVE_SECAO};
use crate::xdata;

/// Grava (`Some`) ou limpa (`None`) o valor de `tags` no bloco `h`. A primeira tag é a chave
/// do XDATA.
pub fn definir(doc: &mut Drawing, h: Handle, tags: &[&str], valor: Option<&str>) -> Result<(), String> {
    let mut e = (**doc.model.get(h).ok_or("objeto não encontrado")?).clone();
    let chave = tags.first().copied().ok_or("sem tag")?;
    let EntityKind::Insert(ins) = &mut e.kind else { return Err("o objeto não é um bloco".into()) };
    match ins.attribs.iter_mut().find(|a| tags.iter().any(|t| a.tag.trim().eq_ignore_ascii_case(t))) {
        Some(a) => {
            a.text.value = valor.unwrap_or("").to_string();
            // O atributo manda: um valor antigo no XDATA não pode ficar por trás dele.
            xdata::gravar(&mut e.common.xdata, chave, None);
        }
        None => xdata::gravar(&mut e.common.xdata, chave, valor),
    }
    doc.model.replace(e);
    Ok(())
}

/// Liga o ponto ao quadro. No XDATA vai o handle do quadro; num atributo QUADRO, a TAG dele.
pub fn ligar_ao_quadro(doc: &mut Drawing, ponto: Handle, quadro: Handle) -> Result<(), String> {
    let p = ler_ponto(doc, ponto).ok_or("o objeto não é um ponto elétrico")?;
    let tag_quadro = ler_ponto(doc, quadro).and_then(|q| q.tag);
    let valor = match (p.tem_atributo(TAGS_QUADRO), tag_quadro) {
        (true, Some(tag)) => tag,
        _ => quadro.hex(),
    };
    definir(doc, ponto, TAGS_QUADRO, Some(&valor))
}

/// Põe o ponto no circuito `circuito` do quadro `quadro`.
pub fn atribuir_circuito(doc: &mut Drawing, ponto: Handle, quadro: Handle, circuito: &str) -> Result<(), String> {
    definir(doc, ponto, TAGS_CIRCUITO, Some(circuito))?;
    ligar_ao_quadro(doc, ponto, quadro)
}

/// Grava a letra de comando do ponto.
pub fn definir_comando(doc: &mut Drawing, ponto: Handle, letra: &str) -> Result<(), String> {
    definir(doc, ponto, TAGS_COMANDO, Some(letra))
}

/// Grava (ou limpa, com `None`) a seção e o disjuntor escolhidos à mão para um circuito, no
/// XDATA do quadro.
pub fn definir_sobrescrita(doc: &mut Drawing, quadro: Handle, circuito: &str, secao: Option<f64>, disjuntor: Option<f64>) -> Result<(), String> {
    let mut e = (**doc.model.get(quadro).ok_or("quadro não encontrado")?).clone();
    if !matches!(e.kind, EntityKind::Insert(_)) {
        return Err("o quadro não é um bloco".into());
    }
    let id = circuito.trim().to_uppercase();
    xdata::gravar(&mut e.common.xdata, &format!("{CHAVE_SECAO}{id}"), secao.map(|s| s.to_string()).as_deref());
    xdata::gravar(&mut e.common.xdata, &format!("{CHAVE_DISJUNTOR}{id}"), disjuntor.map(|d| d.to_string()).as_deref());
    doc.model.replace(e);
    Ok(())
}
