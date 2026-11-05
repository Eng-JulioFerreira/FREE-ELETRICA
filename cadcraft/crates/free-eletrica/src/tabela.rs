// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A tabela do quadro de cargas no desenho: um bloco `QUADRO_TABELA_<handle do quadro>` inserido
//! onde o projetista clicou. Regenerar troca o conteúdo do bloco e mantém a inserção.

use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Line, Space, Text, VAlign};

use crate::Analise;
use crate::ponto::br;
use crate::quadro::{self, NOMES_FASES, ResumoQuadro};
use crate::setup::Setup;

/// Prefixo do bloco da tabela.
pub const PREFIXO: &str = "QUADRO_TABELA_";
/// Camada da tabela.
pub const CAMADA: &str = "ELE-QUADROS";
/// Altura do texto e da linha (m).
const ALTURA: f64 = 0.10;
const LINHA: f64 = 0.20;
/// Largura estimada de cada caractere, em alturas de texto.
const LARGURA_CARACTERE: f64 = 0.8;

fn nome_do_bloco(quadro: Handle) -> String {
    format!("{PREFIXO}{}", quadro.hex())
}

/// As tabelas do desenho: (inserção, quadro).
pub fn tabelas(doc: &Drawing) -> Vec<(Handle, Handle)> {
    doc.model
        .iter()
        .filter_map(|e| {
            let EntityKind::Insert(i) = &e.kind else { return None };
            Some((e.handle, Handle::parse_hex(i.block.strip_prefix(PREFIXO)?)?))
        })
        .collect()
}

fn fixo(v: f64, casas: usize) -> String {
    format!("{v:.casas$}").replace('.', ",")
}

/// As linhas de texto da tabela: a primeira é o cabeçalho.
fn linhas_da_tabela(r: &ResumoQuadro) -> Vec<Vec<String>> {
    let fases = usize::from(r.fases_quadro.clamp(1, 3));
    let mut cab: Vec<String> =
        ["CIRCUITO", "DESCRIÇÃO", "CORRENTE (A)", "TENSÃO (V)", "F.P.", "POTÊNCIA (W)", "QUEDA (%)", "SEÇÃO (mm²)", "DISJUNTOR (A)"]
            .map(String::from)
            .to_vec();
    cab.extend(NOMES_FASES.iter().take(fases).map(|f| format!("FASE {f} (VA)")));
    cab.push("ISOLAÇÃO".into());
    let mut v = vec![cab];
    for l in &r.linhas {
        let e = &l.escolha;
        let queda = e.queda.map_or("-".to_string(), |q| fixo(q, 2));
        let mut c = vec![
            l.circuito.id.clone(),
            l.descricao.clone(),
            fixo(e.ib, 2),
            fixo(l.tensao, 0),
            fixo(l.fp, 2),
            fixo(l.potencia_w, 0),
            if l.fora_da_rede > 0 { format!("~{queda}") } else { queda },
            e.secao.map_or("-".to_string(), br),
            e.disjuntor.map_or("-".to_string(), br),
        ];
        c.extend((0..fases).map(|f| l.carga_fase.get(f).copied().filter(|p| *p > 0.0).map_or(String::new(), |p| fixo(p, 0))));
        c.push(l.tipo_cabo.abreviado().to_string());
        v.push(c);
    }
    v
}

fn texto(valor: &str, em: [f64; 2], altura: f64, halign: HAlign) -> EntityKind {
    let p = Vec3::new(em[0], em[1], 0.0);
    EntityKind::Text(Text {
        insert: p,
        align_pt: Some(p),
        height: altura,
        value: valor.to_string(),
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign,
        valign: VAlign::Middle,
    })
}

fn linha(a: [f64; 2], b: [f64; 2]) -> EntityKind {
    EntityKind::Line(Line { a: Vec3::new(a[0], a[1], 0.0), b: Vec3::new(b[0], b[1], 0.0) })
}

/// Monta o conteúdo do bloco, com o canto superior esquerdo na origem, em unidades do desenho.
fn pecas_da_tabela(r: &ResumoQuadro, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let mut p = Vec::new();
    let celulas = linhas_da_tabela(r);
    let colunas = celulas.first().map_or(0, Vec::len);
    let larguras: Vec<f64> = (0..colunas)
        .map(|c| {
            let maior = celulas.iter().filter_map(|l| l.get(c)).map(|t| t.chars().count()).max().unwrap_or(1).max(4);
            maior as f64 * ALTURA * LARGURA_CARACTERE + ALTURA
        })
        .collect();
    let largura_total: f64 = larguras.iter().sum();
    // Título e alimentação.
    let alimentacao = r.alimentacao();
    p.push(texto(&format!("PAINEL: {}", r.tag), [0.0, u(-LINHA / 2.0)], u(ALTURA * 1.5), HAlign::Left));
    p.push(texto(&format!("ALIMENTAÇÃO: {alimentacao}"), [0.0, u(-LINHA * 1.5)], u(ALTURA), HAlign::Left));
    let topo = -LINHA * 2.0;
    // Grade e células.
    for (i, l) in celulas.iter().enumerate() {
        let y = topo - LINHA * i as f64;
        p.push(linha([0.0, u(y)], [u(largura_total), u(y)]));
        let mut x = 0.0;
        for (c, t) in l.iter().enumerate() {
            let w = larguras.get(c).copied().unwrap_or(0.0);
            if c == 1 && i > 0 {
                p.push(texto(t, [u(x + ALTURA / 2.0), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Left));
            } else {
                p.push(texto(t, [u(x + w / 2.0), u(y - LINHA / 2.0)], u(ALTURA), HAlign::Center));
            }
            x += w;
        }
    }
    let fundo = topo - LINHA * celulas.len() as f64;
    p.push(linha([0.0, u(fundo)], [u(largura_total), u(fundo)]));
    let mut x = 0.0;
    p.push(linha([0.0, u(topo)], [0.0, u(fundo)]));
    for w in &larguras {
        x += w;
        p.push(linha([u(x), u(topo)], [u(x), u(fundo)]));
    }
    // Demanda e totais.
    let mut y = fundo - LINHA;
    p.push(texto(&format!("DEMANDA: {} - {}", r.demanda.tabela, r.demanda.edificacao), [0.0, u(y)], u(ALTURA), HAlign::Left));
    for d in &r.demanda.linhas {
        y -= LINHA;
        let t =
            format!("{}: {} VA x {} % = {} VA", d.tipo.nome().to_uppercase(), fixo(d.instalada, 0), fixo(d.fator * 100.0, 1), fixo(d.demandada, 0));
        p.push(texto(&t, [0.0, u(y)], u(ALTURA), HAlign::Left));
    }
    y -= LINHA;
    let total = format!(
        "POTÊNCIA INSTALADA: {} VA   POTÊNCIA DEMANDADA: {} VA   CORRENTE DEMANDADA: {} A",
        fixo(r.demanda.instalada, 0),
        fixo(r.demanda.demandada, 0),
        fixo(r.corrente_demandada, 2)
    );
    p.push(texto(&total, [0.0, u(y)], u(ALTURA), HAlign::Left));
    y -= LINHA;
    let a = &r.alimentador;
    let alim = format!(
        "ALIMENTADOR: {} mm²   DISJUNTOR GERAL: {} A   DR: {} A / 30 mA   DPS: classe II, Uc >= {} V",
        a.secao.map_or("-".to_string(), br),
        a.disjuntor.map_or("-".to_string(), br),
        a.dr.map_or("-".to_string(), br),
        fixo(a.dps_uc, 0)
    );
    p.push(texto(&alim, [0.0, u(y)], u(ALTURA), HAlign::Left));
    p
}

fn gravar_bloco(doc: &mut Drawing, r: &ResumoQuadro, m: f64) {
    let nome = nome_do_bloco(r.quadro);
    let mut b = Block::new(&nome);
    for k in pecas_da_tabela(r, m) {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(nome, Arc::new(b));
}

/// Insere a tabela do quadro em `pos`. Se ele já tem tabela, ela é movida para lá.
pub fn inserir(doc: &mut Drawing, analise: &Analise, setup: &Setup, quadro: Handle, pos: [f64; 2]) -> Result<Handle, String> {
    if !pos.iter().all(|v| v.is_finite()) {
        return Err("posição inválida".into());
    }
    let r = quadro::rotear(doc, analise, setup, quadro)?;
    gravar_bloco(doc, &r, if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 });
    if let Some((h, _)) = tabelas(doc).into_iter().find(|(_, q)| *q == quadro)
        && let Some(mut e) = doc.model.get(h).map(|e| (**e).clone())
    {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert = Vec3::new(pos[0], pos[1], 0.0);
        }
        doc.model.replace(e);
        return Ok(h);
    }
    let ins = Insert {
        block: nome_do_bloco(quadro),
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

/// Refaz o conteúdo das tabelas do desenho, sem mexer na inserção delas. A tabela de um quadro
/// que não existe mais sai. Devolve quantas foram refeitas.
pub fn regenerar(doc: &mut Drawing, analise: &Analise, setup: &Setup) -> Result<usize, String> {
    let mut n = 0;
    for (h, q) in tabelas(doc) {
        match quadro::rotear(doc, analise, setup, q) {
            Ok(r) => {
                gravar_bloco(doc, &r, if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 });
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
