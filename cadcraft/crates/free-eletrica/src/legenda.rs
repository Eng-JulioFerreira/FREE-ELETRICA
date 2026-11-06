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
const ALTURA: f64 = crate::tabela::ALTURA;
/// A altura de cada linha, a largura da coluna do símbolo e a altura da faixa do título (m), para a planta
/// em 1:50 com 1 unidade = 1 m.
const LINHA: f64 = 0.28;
const COLUNA_SIMBOLO: f64 = 0.50;
const TITULO: f64 = 0.30;
/// A fração da célula que o símbolo pode ocupar: um símbolo maior que isso é reduzido.
const OCUPACAO: f64 = 0.85;

/// A legenda do desenho: a inserção dela.
pub fn inserida(doc: &Drawing) -> Option<Handle> {
    doc.model.iter().find(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block == BLOCO)).map(|e| e.handle)
}

/// As linhas da legenda: (bloco, nome), sem repetir, na ordem do tipo de ponto e do nome.
pub fn linhas(analise: &Analise) -> Vec<(String, String)> {
    let mut v: Vec<(crate::ponto::TipoPonto, String, String)> = analise
        .pontos
        .iter()
        .map(|p| {
            // Os módulos empilhados entram com o bloco de origem: o símbolo e o nome são os dele.
            let bloco = crate::biblioteca::bloco_de_origem(&p.bloco).to_string();
            let nome = p.legenda.clone().unwrap_or_else(|| format!("{} ({bloco})", p.tipo.nome()));
            (p.tipo, bloco, nome)
        })
        .collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|(_, b, n)| (b, n)).collect()
}

/// O retângulo que cobre o desenho do bloco (sem atributos nem textos), em relação à base dele: (mínimo, máximo).
pub(crate) fn caixa_do_desenho(b: &Block) -> Option<([f64; 2], [f64; 2])> {
    let mut v: Vec<[f64; 2]> = Vec::new();
    for e in b.entities.iter() {
        match &e.kind {
            EntityKind::Line(l) => v.extend([[l.a.x, l.a.y], [l.b.x, l.b.y]]),
            EntityKind::Circle(c) => v.extend([[c.center.x - c.radius, c.center.y - c.radius], [c.center.x + c.radius, c.center.y + c.radius]]),
            EntityKind::Arc(a) => {
                // As pontas e os pontos cardeais que o arco varre.
                let em = |ang: f64| [a.center.x + a.radius * ang.cos(), a.center.y + a.radius * ang.sin()];
                let varrido = (a.end - a.start).rem_euclid(std::f64::consts::TAU);
                v.extend([em(a.start), em(a.end)]);
                for k in 0..4 {
                    let ang = f64::from(k) * std::f64::consts::FRAC_PI_2;
                    if (ang - a.start).rem_euclid(std::f64::consts::TAU) <= varrido {
                        v.push(em(ang));
                    }
                }
            }
            EntityKind::LwPolyline(p) => v.extend(p.vertices.iter().map(|x| [x.p.x, x.p.y])),
            EntityKind::Solid(s) => v.extend(s.corners.iter().map(|c| [c.x, c.y])),
            _ => {}
        }
    }
    let v: Vec<[f64; 2]> = v.into_iter().filter(|p| p[0].is_finite() && p[1].is_finite()).map(|p| [p[0] - b.base.x, p[1] - b.base.y]).collect();
    let min = [v.iter().map(|p| p[0]).reduce(f64::min)?, v.iter().map(|p| p[1]).reduce(f64::min)?];
    let max = [v.iter().map(|p| p[0]).reduce(f64::max)?, v.iter().map(|p| p[1]).reduce(f64::max)?];
    Some((min, max))
}

/// O símbolo `bloco` no meio da célula de centro `centro` e tamanho `celula` (no desenho): o centro do retângulo
/// que o cobre cai no centro da célula, reduzido se não couber.
fn simbolo_na_celula(doc: &Drawing, bloco: &str, centro: [f64; 2], celula: [f64; 2]) -> EntityKind {
    let (escala, meio) = match doc.blocks.get(bloco).and_then(|b| caixa_do_desenho(b)) {
        Some((min, max)) => {
            let (w, h) = (max[0] - min[0], max[1] - min[1]);
            let cabe = |lado: f64, espaco: f64| if lado > 1e-12 { OCUPACAO * espaco / lado } else { f64::INFINITY };
            let e = cabe(w, celula[0]).min(cabe(h, celula[1])).min(1.0);
            (e, [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0])
        }
        None => (1.0, [0.0, 0.0]),
    };
    EntityKind::Insert(Insert {
        block: bloco.to_string(),
        insert: Vec3::new(centro[0] - meio[0] * escala, centro[1] - meio[1] * escala, 0.0),
        scale: Vec3::new(escala, escala, 1.0),
        rotation: 0.0,
        attribs: Vec::new(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    })
}

fn pecas(doc: &Drawing, analise: &Analise, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let lista = linhas(analise);
    let maior = lista.iter().map(|(_, n)| n.chars().count()).max().unwrap_or(0).max(20);
    let largura = (COLUNA_SIMBOLO + maior as f64 * ALTURA * crate::tabela::LARGURA_CARACTERE + 2.0 * ALTURA)
        .max("LEGENDA DE SÍMBOLOS".chars().count() as f64 * crate::tabela::ALTURA_TITULO * crate::tabela::LARGURA_CARACTERE + ALTURA);
    let fundo = -TITULO - LINHA * lista.len() as f64;
    let mut p = vec![texto("LEGENDA DE SÍMBOLOS", [u(largura / 2.0), u(-TITULO / 2.0)], u(crate::tabela::ALTURA_TITULO), HAlign::Center)];
    for (a, b) in [([0.0, 0.0], [largura, 0.0]), ([largura, 0.0], [largura, fundo]), ([largura, fundo], [0.0, fundo]), ([0.0, fundo], [0.0, 0.0])] {
        p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
    }
    for (i, (bloco, nome)) in lista.iter().enumerate() {
        let y = -TITULO - LINHA * i as f64;
        p.push(linha([0.0, u(y)], [u(largura), u(y)]));
        p.push(linha([u(COLUNA_SIMBOLO), u(y)], [u(COLUNA_SIMBOLO), u(y - LINHA)]));
        p.push(simbolo_na_celula(doc, bloco, [u(COLUNA_SIMBOLO / 2.0), u(y - LINHA / 2.0)], [u(COLUNA_SIMBOLO), u(LINHA)]));
        p.push(texto(nome, [u(COLUNA_SIMBOLO + ALTURA), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Left));
    }
    p
}
fn gravar_bloco(doc: &mut Drawing, analise: &Analise, m: f64) {
    let mut b = Block::new(BLOCO);
    for k in pecas(doc, analise, m) {
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
