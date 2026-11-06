// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A Legenda de Símbolos da prancha: um bloco com o símbolo e o nome de cada tipo de ponto usado no desenho,
//! uma por disciplina: `LEGENDA_SIMBOLOS_ELETRICA` (os pontos elétricos e as caixas) e
//! `LEGENDA_SIMBOLOS_TELECOM`. `LEGENDA_SIMBOLOS`, das versões anteriores, tem todos. O nome vem do atributo
//! `LEGENDA` do ponto ou da família. A da elétrica vem com uma segunda tabela ao lado, a simbologia do diagrama
//! unifilar (`LEGENDA_SIMBOLOGIA_UNIFILAR`). Regenerar troca o conteúdo dos blocos e mantém as inserções.

use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space};

use crate::Analise;
use crate::disciplina::Disciplina;
use crate::tabela::{CAMADA, linha, texto};

/// O bloco da legenda com todos os pontos (das versões anteriores).
pub const BLOCO: &str = "LEGENDA_SIMBOLOS";

/// O bloco da legenda de uma disciplina; `None`: a de todas.
pub fn bloco(disciplina: Option<Disciplina>) -> &'static str {
    match disciplina {
        None => BLOCO,
        Some(Disciplina::Eletrica) => "LEGENDA_SIMBOLOS_ELETRICA",
        Some(Disciplina::Telecom) => "LEGENDA_SIMBOLOS_TELECOM",
        Some(Disciplina::AlarmeIncendio) => "LEGENDA_SIMBOLOS_INCENDIO",
    }
}

/// O título da legenda de uma disciplina.
pub fn titulo(disciplina: Option<Disciplina>) -> &'static str {
    match disciplina {
        None => "LEGENDA DE SÍMBOLOS",
        Some(Disciplina::Eletrica) => "LEGENDA DE SÍMBOLOS ELÉTRICOS",
        Some(Disciplina::Telecom) => "LEGENDA DE SÍMBOLOS TELECOM",
        Some(Disciplina::AlarmeIncendio) => "LEGENDA DE SÍMBOLOS DE INCÊNDIO",
    }
}

/// A disciplina do bloco de uma legenda. `None` por fora: não é legenda; `Some(None)`: a de todas.
fn disciplina_do_bloco(nome: &str) -> Option<Option<Disciplina>> {
    std::iter::once(None).chain(Disciplina::TODAS.into_iter().map(Some)).find(|d| bloco(*d) == nome)
}

/// O ponto entra na legenda da disciplina: as caixas de passagem, sem disciplina, entram na elétrica.
fn da_disciplina(p: &crate::ponto::PontoEletrico, disciplina: Option<Disciplina>) -> bool {
    match disciplina {
        None => true,
        Some(Disciplina::Eletrica) => matches!(Disciplina::do_ponto(p), None | Some(Disciplina::Eletrica)),
        Some(d) => Disciplina::do_ponto(p) == Some(d),
    }
}
const ALTURA: f64 = crate::tabela::ALTURA;
/// A altura de cada linha, a largura da coluna do símbolo e a altura da faixa do título (m), para a planta
/// em 1:50 com 1 unidade = 1 m.
const LINHA: f64 = 0.28;
const COLUNA_SIMBOLO: f64 = 0.50;
const TITULO: f64 = 0.30;
/// A fração da célula que o símbolo pode ocupar: um símbolo maior que isso é reduzido.
const OCUPACAO: f64 = 0.85;

/// Uma legenda do desenho, de qualquer disciplina: a inserção dela.
pub fn inserida(doc: &Drawing) -> Option<Handle> {
    legendas(doc).first().map(|(h, _)| *h)
}

/// A legenda de uma disciplina no desenho.
pub fn inserida_de(doc: &Drawing, disciplina: Option<Disciplina>) -> Option<Handle> {
    legendas(doc).into_iter().find(|(_, d)| *d == disciplina).map(|(h, _)| h)
}

/// As legendas do desenho: (inserção, disciplina).
pub fn legendas(doc: &Drawing) -> Vec<(Handle, Option<Disciplina>)> {
    doc.model
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Insert(i) => disciplina_do_bloco(&i.block).map(|d| (e.handle, d)),
            _ => None,
        })
        .collect()
}

/// As linhas da legenda de uma disciplina (`None`: todas): (bloco, nome), sem repetir, na ordem do tipo de
/// ponto e do nome.
pub fn linhas(analise: &Analise, disciplina: Option<Disciplina>) -> Vec<(String, String)> {
    let mut v: Vec<(crate::ponto::TipoPonto, String, String)> = analise
        .pontos
        .iter()
        .filter(|p| da_disciplina(p, disciplina))
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

/// O bloco do quadro de simbologia do diagrama unifilar: a Legenda de símbolos da aba Elétrica o põe ao lado
/// da legenda da planta, como uma segunda tabela. O comando do unifilar desenha só o diagrama.
pub const BLOCO_UNIFILAR: &str = "LEGENDA_SIMBOLOGIA_UNIFILAR";
/// O espaço entre as duas tabelas lado a lado (m).
const ENTRE_TABELAS: f64 = 0.50;

/// A inserção do quadro de simbologia do unifilar.
pub fn simbologia_unifilar(doc: &Drawing) -> Option<Handle> {
    doc.model.iter().find(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block == BLOCO_UNIFILAR)).map(|e| e.handle)
}

fn gravar_simbologia(doc: &mut Drawing, m: f64) {
    crate::fiacao::garantir_blocos(doc, m);
    let mut b = Block::new(BLOCO_UNIFILAR);
    for (k, _) in crate::unifilar::pecas_da_simbologia(m).0 {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(BLOCO_UNIFILAR.to_string(), Arc::new(b));
}

/// Põe a inserção `nome` em `pos`: move a que existe (`existente`) ou cria uma.
fn inserir_bloco(doc: &mut Drawing, nome: &str, existente: Option<Handle>, pos: [f64; 2]) -> Result<Handle, String> {
    if let Some(h) = existente
        && let Some(mut e) = doc.model.get(h).map(|e| (**e).clone())
    {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert = Vec3::new(pos[0], pos[1], 0.0);
        }
        doc.model.replace(e);
        return Ok(h);
    }
    let ins = Insert {
        block: nome.into(),
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

/// O conteúdo do bloco da legenda e a largura dela (m).
fn pecas(doc: &Drawing, analise: &Analise, m: f64, disciplina: Option<Disciplina>) -> (Vec<EntityKind>, f64) {
    let u = |v: f64| v / m;
    let lista = linhas(analise, disciplina);
    let titulo = titulo(disciplina);
    let maior = lista.iter().map(|(_, n)| n.chars().count()).max().unwrap_or(0).max(20);
    let largura = (COLUNA_SIMBOLO + maior as f64 * ALTURA * crate::tabela::LARGURA_CARACTERE + 2.0 * ALTURA)
        .max(titulo.chars().count() as f64 * crate::tabela::ALTURA_TITULO * crate::tabela::LARGURA_CARACTERE + ALTURA);
    let fundo = -TITULO - LINHA * lista.len() as f64;
    let mut p = vec![texto(titulo, [u(largura / 2.0), u(-TITULO / 2.0)], u(crate::tabela::ALTURA_TITULO), HAlign::Center)];
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
    (p, largura)
}

/// Grava o bloco da legenda e devolve a largura dela (m).
fn gravar_bloco(doc: &mut Drawing, analise: &Analise, m: f64, disciplina: Option<Disciplina>) -> f64 {
    let nome = bloco(disciplina);
    let mut b = Block::new(nome);
    let (lista, largura) = pecas(doc, analise, m, disciplina);
    for k in lista {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(nome.to_string(), Arc::new(b));
    largura
}

/// Gera a legenda da disciplina (`None`: todas) em `pos`. Se o desenho já tem uma dessa disciplina, ela é
/// refeita e movida para lá. A da elétrica traz, ao lado, a simbologia do diagrama unifilar
/// ([`BLOCO_UNIFILAR`]), num bloco à parte.
pub fn inserir(doc: &mut Drawing, analise: &Analise, pos: [f64; 2], metros: f64, disciplina: Option<Disciplina>) -> Result<Handle, String> {
    if !pos.iter().all(|v| v.is_finite()) {
        return Err("posição inválida".into());
    }
    let m = if metros > 0.0 { metros } else { 1.0 };
    let largura = gravar_bloco(doc, analise, m, disciplina);
    let existente = inserida_de(doc, disciplina);
    let h = inserir_bloco(doc, bloco(disciplina), existente, pos)?;
    if disciplina == Some(Disciplina::Eletrica) {
        gravar_simbologia(doc, m);
        let ao_lado = [pos[0] + (largura + ENTRE_TABELAS) / m, pos[1]];
        let existente = simbologia_unifilar(doc);
        inserir_bloco(doc, BLOCO_UNIFILAR, existente, ao_lado)?;
    }
    Ok(h)
}

/// Refaz as legendas do desenho, sem mexer nas inserções. Devolve quantas refez.
pub fn regenerar(doc: &mut Drawing, analise: &Analise, metros: f64) -> usize {
    let m = if metros > 0.0 { metros } else { 1.0 };
    let mut disciplinas: Vec<Option<Disciplina>> = legendas(doc).into_iter().map(|(_, d)| d).collect();
    disciplinas.dedup();
    for d in &disciplinas {
        gravar_bloco(doc, analise, m, *d);
    }
    let simbologia = simbologia_unifilar(doc).is_some();
    if simbologia {
        gravar_simbologia(doc, m);
    }
    disciplinas.len() + usize::from(simbologia)
}
