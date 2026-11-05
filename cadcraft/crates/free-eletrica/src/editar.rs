// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Escritas do Free Elétrica no desenho. Um valor vai para o atributo do bloco quando ele existe
//! (o projetista vê e edita) e, senão, para o XDATA do Free Elétrica (viaja com o bloco, invisível).

use cadcraft_doc::{Drawing, EntityKind, Handle};

use crate::nbr5410::{Esquema, Isolacao, TipoCabo};
use crate::ponto::{CHAVE_ALTURA, TAGS_ALTURA, TAGS_CIRCUITO, TAGS_COMANDO, TAGS_QUADRO, ler_ponto};
use crate::quadro::{
    CHAVE_CABO, CHAVE_DESCRICAO, CHAVE_DISJUNTOR, CHAVE_ESQUEMA, CHAVE_FASE, CHAVE_ISOLACAO, CHAVE_ISOLACAO_CIRCUITO, CHAVE_SECAO, CHAVE_TENSAO,
    NOMES_FASES, fases_de_texto,
};
use crate::xdata;

/// Grava (ou apaga) uma chave do XDATA do Free Elétrica num bloco.
fn gravar_xdata(doc: &mut Drawing, h: Handle, chave: &str, valor: Option<&str>) -> Result<(), String> {
    let mut e = (**doc.model.get(h).ok_or("objeto não encontrado")?).clone();
    if !matches!(e.kind, EntityKind::Insert(_)) {
        return Err("o objeto não é um bloco".into());
    }
    xdata::gravar(&mut e.common.xdata, chave, valor);
    doc.model.replace(e);
    Ok(())
}

/// Os pontos do circuito `circuito` do quadro `quadro`.
fn pontos_do_circuito(doc: &Drawing, quadro: Handle, circuito: &str) -> Vec<Handle> {
    crate::analisar(doc)
        .pontos
        .iter()
        .filter(|p| p.quadro == Some(quadro) && p.circuito.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(circuito.trim())))
        .map(|p| p.handle)
        .collect()
}

/// Apaga do quadro as escolhas e a descrição de um circuito.
fn esquecer_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str) -> Result<(), String> {
    let id = circuito.trim().to_uppercase();
    for prefixo in [CHAVE_SECAO, CHAVE_DISJUNTOR, CHAVE_DESCRICAO, CHAVE_ISOLACAO_CIRCUITO, CHAVE_CABO, CHAVE_ESQUEMA, CHAVE_TENSAO, CHAVE_FASE] {
        gravar_xdata(doc, quadro, &format!("{prefixo}{id}"), None)?;
    }
    Ok(())
}

fn chave_do_circuito(prefixo: &str, circuito: &str) -> Result<String, String> {
    let id = circuito.trim().to_uppercase();
    if id.is_empty() {
        return Err("informe o circuito".into());
    }
    Ok(format!("{prefixo}{id}"))
}

/// Isolação dos condutores de um circuito; `None` volta à do quadro.
pub fn definir_isolacao_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, isolacao: Option<Isolacao>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CABO, circuito)?, None)?;
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ISOLACAO_CIRCUITO, circuito)?, isolacao.map(Isolacao::codigo))
}

/// Tipo de cabo de um circuito; `None` volta ao padrão.
pub fn definir_cabo_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, cabo: Option<TipoCabo>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ISOLACAO_CIRCUITO, circuito)?, None)?;
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CABO, circuito)?, cabo.map(TipoCabo::codigo))
}

/// Destrava as fases de todos os circuitos do quadro: o balanceamento automático volta a valer para
/// todos. Devolve quantos estavam travados.
pub fn liberar_fases(doc: &mut Drawing, quadro: Handle) -> Result<usize, String> {
    let p = ler_ponto(doc, quadro).ok_or("quadro não encontrado")?;
    let chaves: Vec<String> = p.extras.iter().filter(|(k, _)| k.to_uppercase().starts_with(CHAVE_FASE)).map(|(k, _)| k.clone()).collect();
    for k in &chaves {
        gravar_xdata(doc, quadro, k, None)?;
    }
    Ok(chaves.len())
}

/// Isolação dos condutores do quadro; `None` volta à das configurações.
pub fn definir_isolacao(doc: &mut Drawing, quadro: Handle, isolacao: Option<Isolacao>) -> Result<(), String> {
    gravar_xdata(doc, quadro, CHAVE_ISOLACAO, isolacao.map(Isolacao::codigo))
}

/// Esquema de condutores de um circuito; `None` volta ao que os pontos dizem.
pub fn definir_esquema(doc: &mut Drawing, quadro: Handle, circuito: &str, esquema: Option<Esquema>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ESQUEMA, circuito)?, esquema.map(Esquema::codigo))
}

/// Tensão de um circuito (V); `None` volta à dos pontos.
pub fn definir_tensao_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, tensao: Option<f64>) -> Result<(), String> {
    if let Some(t) = tensao
        && !(t.is_finite() && t > 0.0)
    {
        return Err(format!("tensão {t} inválida"));
    }
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_TENSAO, circuito)?, tensao.map(|t| t.to_string()).as_deref())
}

/// Trava as fases de um circuito ("R", "RS"...); `None` volta ao balanceamento automático.
pub fn travar_fases(doc: &mut Drawing, quadro: Handle, circuito: &str, fases: Option<&str>) -> Result<(), String> {
    let normal = match fases {
        Some(t) => {
            let v = fases_de_texto(t).ok_or_else(|| format!("fases \"{t}\" inválidas: use R, S, T"))?;
            Some(v.iter().filter_map(|f| NOMES_FASES.get(*f).copied()).collect::<String>())
        }
        None => None,
    };
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_FASE, circuito)?, normal.as_deref())
}

/// Tira pontos do circuito deles (ficam sem circuito e sem quadro). Devolve quantos saíram.
pub fn remover_do_circuito(doc: &mut Drawing, pontos: &[Handle]) -> Result<usize, String> {
    let mut n = 0;
    for h in pontos {
        let p = ler_ponto(doc, *h).ok_or_else(|| format!("o objeto {} não é um ponto elétrico", h.hex()))?;
        if p.circuito.is_some() {
            definir(doc, *h, TAGS_CIRCUITO, None)?;
            definir(doc, *h, TAGS_QUADRO, None)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Grava no XDATA do interruptor as luminárias da letra: `target_handles.<letra>`.
pub fn vincular_luminaria(doc: &mut Drawing, interruptor: Handle, letra: &str, luminaria: Handle) -> Result<(), String> {
    let chave = format!("target_handles.{}", letra.trim().to_lowercase());
    let p = ler_ponto(doc, interruptor).ok_or("o interruptor não é um ponto elétrico")?;
    let atual = p.extras.iter().find(|(k, _)| k.eq_ignore_ascii_case(&chave)).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut lista: Vec<String> = atual.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
    if !lista.iter().any(|h| h.eq_ignore_ascii_case(&luminaria.hex())) {
        lista.push(luminaria.hex());
    }
    gravar_xdata(doc, interruptor, &chave, Some(&lista.join(",")))
}

/// Descrição do circuito no quadro de cargas (`None` apaga). Um circuito descrito existe no
/// quadro mesmo sem pontos.
pub fn definir_descricao(doc: &mut Drawing, quadro: Handle, circuito: &str, descricao: Option<&str>) -> Result<(), String> {
    let id = circuito.trim().to_uppercase();
    if id.is_empty() {
        return Err("informe o circuito".into());
    }
    gravar_xdata(doc, quadro, &format!("{CHAVE_DESCRICAO}{id}"), descricao)
}

/// Exclui o circuito do quadro: os pontos dele ficam sem circuito e sem quadro, e as escolhas e a
/// descrição dele somem. Devolve quantos pontos foram liberados.
pub fn excluir_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str) -> Result<usize, String> {
    let pontos = pontos_do_circuito(doc, quadro, circuito);
    for h in &pontos {
        definir(doc, *h, TAGS_CIRCUITO, None)?;
        definir(doc, *h, TAGS_QUADRO, None)?;
    }
    esquecer_circuito(doc, quadro, circuito)?;
    Ok(pontos.len())
}

/// Junta o circuito `de` no circuito `para` do mesmo quadro: os pontos de `de` passam para `para`
/// e as escolhas e a descrição de `de` somem. Devolve quantos pontos mudaram.
pub fn juntar_circuitos(doc: &mut Drawing, quadro: Handle, de: &str, para: &str) -> Result<usize, String> {
    if de.trim().eq_ignore_ascii_case(para.trim()) {
        return Err("escolha dois circuitos diferentes".into());
    }
    if para.trim().is_empty() {
        return Err("informe o circuito de destino".into());
    }
    let pontos = pontos_do_circuito(doc, quadro, de);
    for h in &pontos {
        atribuir_circuito(doc, *h, quadro, para.trim())?;
    }
    esquecer_circuito(doc, quadro, de)?;
    Ok(pontos.len())
}

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

/// O texto que vai no atributo ALTURA de um ponto fora da legenda: `h=1,45m`.
pub fn texto_de_altura(altura: f64) -> String {
    format!("h={altura:.2}m").replace('.', ",")
}

/// A altura de uma inserção. Na legenda do projeto (ou com `None`), o atributo visível fica vazio
/// e a planta não polui; fora dela, a altura exata vai para o XDATA, que o cálculo usa, e o atributo
/// mostra `h=1,45m`.
pub fn definir_altura(doc: &mut Drawing, ponto: Handle, altura: Option<f64>, setup: &crate::setup::Setup) -> Result<(), String> {
    if let Some(a) = altura
        && !(a.is_finite() && (0.0..=50.0).contains(&a))
    {
        return Err(format!("altura {a} inválida: use de 0 a 50 m"));
    }
    let p = ler_ponto(doc, ponto).ok_or("o objeto não é um ponto elétrico")?;
    let legenda = p.altura_legenda(setup);
    let fora = altura.filter(|a| (a - legenda).abs() >= 0.005);
    let texto = fora.map(texto_de_altura);
    definir(doc, ponto, TAGS_ALTURA, if p.tem_atributo(TAGS_ALTURA) { Some(texto.as_deref().unwrap_or("")) } else { None })?;
    gravar_xdata(doc, ponto, CHAVE_ALTURA, fora.map(|a| a.to_string()).as_deref())
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
