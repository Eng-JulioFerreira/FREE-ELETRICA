// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! O diagrama unifilar de um quadro no desenho: um bloco `UNIFILAR_<handle do quadro>`. Mostra o
//! alimentador, a barra, os grupos de DR e os circuitos, com o esquema, a seção, o disjuntor e a curva.
//! Regenerar troca o conteúdo do bloco e mantém a inserção, como a tabela do quadro.

use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space};

use crate::Analise;
use crate::ponto::br;
use crate::quadro::{self, Linha, ResumoQuadro};
use crate::setup::Setup;
use crate::tabela::{CAMADA, linha, texto};

/// Prefixo do bloco do diagrama.
pub const PREFIXO: &str = "UNIFILAR_";
/// Altura do texto (m).
const ALTURA: f64 = 0.10;
/// Distância entre os circuitos (m).
const PASSO: f64 = 0.32;
/// Onde a barra do quadro fica (m).
const BARRA: f64 = 0.9;
/// Onde a derivação dos circuitos começa (m).
const DERIVACAO: f64 = 1.9;
/// A distância padrão entre o quadro e o diagrama (m).
const AFASTAMENTO: f64 = 3.0;

fn nome_do_bloco(quadro: Handle) -> String {
    format!("{PREFIXO}{}", quadro.hex())
}

/// Os diagramas do desenho: (inserção, quadro).
pub fn unifilares(doc: &Drawing) -> Vec<(Handle, Handle)> {
    doc.model
        .iter()
        .filter_map(|e| {
            let EntityKind::Insert(i) = &e.kind else { return None };
            Some((e.handle, Handle::parse_hex(i.block.strip_prefix(PREFIXO)?)?))
        })
        .collect()
}

/// O símbolo de um disjuntor na linha, de `x` a `x + 0,17`.
fn disjuntor(p: &mut Vec<EntityKind>, x: f64, y: f64, u: &dyn Fn(f64) -> f64) {
    let (x0, x1, y0, y1) = (x, x + 0.17, y - 0.05, y + 0.05);
    for (a, b) in [([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0]), ([x0, y0], [x1, y1])] {
        p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
    }
}

/// O texto de um circuito no diagrama.
fn rotulo(l: &Linha) -> String {
    let secao = l.escolha.secao.map_or("-".to_string(), |s| format!("#{}", br(s)));
    let disjuntor = l.escolha.disjuntor.map_or("-".to_string(), |d| format!("{} A/{}", br(d), l.curva.codigo()));
    let descricao = if l.descricao.is_empty() { String::new() } else { format!(" {}", l.descricao) };
    format!("{}{descricao} | {} {secao} | {disjuntor} | {} VA", l.circuito.id, l.esquema.codigo(), br(l.circuito.potencia.round()))
}

/// O conteúdo do bloco, com o canto superior esquerdo na origem, em unidades do desenho.
fn pecas(r: &ResumoQuadro, alimentado_por: &str, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let mut p = Vec::new();
    p.push(texto(&format!("DIAGRAMA UNIFILAR: {}", r.tag), [0.0, u(-0.05)], u(ALTURA * 1.5), HAlign::Left));
    p.push(texto(&format!("{} | Alimentado por: {alimentado_por}", r.alimentacao()), [0.0, u(-0.27)], u(ALTURA), HAlign::Left));
    // O alimentador e o disjuntor geral.
    let topo = -0.65;
    p.push(linha([0.0, u(topo)], [u(0.3), u(topo)]));
    disjuntor(&mut p, 0.3, topo, &u);
    p.push(linha([u(0.47), u(topo)], [u(BARRA), u(topo)]));
    let a = &r.alimentador;
    let geral = format!(
        "{} | {}",
        a.secao.map_or("-".to_string(), |s| format!("#{}", br(s))),
        a.disjuntor.map_or("-".to_string(), |d| format!("{} A", br(d)))
    );
    p.push(texto(&geral, [0.0, u(topo + 0.13)], u(ALTURA * 0.8), HAlign::Left));

    let grupos = r.grupos_dr();
    let mut y = topo - 0.3;
    let mut fundo = topo;
    let mut circuito = |p: &mut Vec<EntityKind>, l: &Linha, y: f64| {
        p.push(linha([u(DERIVACAO), u(y)], [u(DERIVACAO + 0.2), u(y)]));
        disjuntor(p, DERIVACAO + 0.2, y, &u);
        p.push(linha([u(DERIVACAO + 0.37), u(y)], [u(DERIVACAO + 0.7), u(y)]));
        p.push(texto(&rotulo(l), [u(DERIVACAO + 0.75), u(y)], u(ALTURA), HAlign::Left));
        fundo = y;
    };
    for g in &grupos {
        let membros: Vec<&Linha> = r.linhas.iter().filter(|l| g.circuitos.iter().any(|c| c.eq_ignore_ascii_case(&l.circuito.id))).collect();
        // A barra até o DR, o DR e a barra dos circuitos dele.
        p.push(linha([u(BARRA), u(y)], [u(1.25), u(y)]));
        for (a, b) in [
            ([1.25, y - 0.1], [1.6, y - 0.1]),
            ([1.6, y - 0.1], [1.6, y + 0.1]),
            ([1.6, y + 0.1], [1.25, y + 0.1]),
            ([1.25, y + 0.1], [1.25, y - 0.1]),
        ] {
            p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
        }
        let dr = g.dr.map_or(g.nome.clone(), |d| format!("{} {} A/30 mA", g.nome, br(d)));
        p.push(texto(&dr, [u(1.0), u(y + 0.2)], u(ALTURA * 0.8), HAlign::Left));
        p.push(linha([u(1.6), u(y)], [u(DERIVACAO), u(y)]));
        let inicio = y;
        for l in membros {
            circuito(&mut p, l, y);
            y -= PASSO;
        }
        p.push(linha([u(DERIVACAO), u(inicio)], [u(DERIVACAO), u(y + PASSO)]));
        // Espaço para o rótulo do próximo DR.
        y -= 0.12;
    }
    for l in r.linhas.iter().filter(|l| l.grupo_dr.is_none()) {
        p.push(linha([u(BARRA), u(y)], [u(DERIVACAO), u(y)]));
        circuito(&mut p, l, y);
        y -= PASSO;
    }
    // A barra do quadro, até o último circuito.
    p.push(linha([u(BARRA), u(topo)], [u(BARRA), u(fundo.min(topo))]));
    p
}

/// O texto de quem alimenta o quadro, para o título do diagrama.
fn quem_alimenta(r: &ResumoQuadro, analise: &Analise) -> String {
    match r.config.alimentado_por.as_deref() {
        None => "não definido".to_string(),
        Some(quadro::CONCESSIONARIA) => "concessionária".to_string(),
        Some(t) => Handle::parse_hex(t).and_then(|h| analise.ponto(h)).map_or_else(|| "quadro removido".to_string(), |q| q.rotulo()),
    }
}

fn gravar_bloco(doc: &mut Drawing, r: &ResumoQuadro, analise: &Analise) {
    let m = if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 };
    let nome = nome_do_bloco(r.quadro);
    let mut b = Block::new(&nome);
    for k in pecas(r, &quem_alimenta(r, analise), m) {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(nome, Arc::new(b));
}

/// Gera ou atualiza o diagrama do quadro. Com `pos`, ele vai para lá; sem ela, um diagrama existente
/// fica onde está e um novo nasce ao lado do quadro.
pub fn gerar(doc: &mut Drawing, analise: &Analise, setup: &Setup, quadro: Handle, pos: Option<[f64; 2]>) -> Result<Handle, String> {
    if pos.is_some_and(|p| !p.iter().all(|v| v.is_finite())) {
        return Err("posição inválida".into());
    }
    let r = quadro::rotear(doc, analise, setup, quadro)?;
    gravar_bloco(doc, &r, analise);
    if let Some((h, _)) = unifilares(doc).into_iter().find(|(_, q)| *q == quadro) {
        if let (Some(p), Some(mut e)) = (pos, doc.model.get(h).map(|e| (**e).clone())) {
            if let EntityKind::Insert(i) = &mut e.kind {
                i.insert = Vec3::new(p[0], p[1], 0.0);
            }
            doc.model.replace(e);
        }
        return Ok(h);
    }
    let m = if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 };
    let junto = analise.ponto(quadro).map_or([0.0, 0.0], |q| [q.posicao[0] + AFASTAMENTO / m, q.posicao[1]]);
    let p = pos.unwrap_or(junto);
    let ins = Insert {
        block: nome_do_bloco(quadro),
        insert: Vec3::new(p[0], p[1], 0.0),
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

/// Refaz os diagramas do desenho, sem mexer na inserção deles. O de um quadro que não existe mais
/// sai. Devolve quantos foram refeitos.
pub fn regenerar(doc: &mut Drawing, analise: &Analise, setup: &Setup) -> Result<usize, String> {
    let mut n = 0;
    for (h, q) in unifilares(doc) {
        match quadro::rotear(doc, analise, setup, q) {
            Ok(r) => {
                gravar_bloco(doc, &r, analise);
                n += 1;
            }
            Err(_) => {
                doc.model.remove(h);
                doc.blocks.remove(&nome_do_bloco(q));
            }
        }
    }
    Ok(n)
}
