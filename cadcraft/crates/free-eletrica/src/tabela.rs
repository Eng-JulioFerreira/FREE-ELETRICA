// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A tabela do quadro de cargas no desenho: um bloco `QUADRO_TABELA_<handle do quadro>` inserido
//! onde o projetista clicou. Regenerar troca o conteúdo do bloco e mantém a inserção.
//!
//! Tudo fica dentro de uma moldura contínua: o cabeçalho, a grade dos circuitos, a linha de total e o
//! rodapé dividido ao meio, com a demanda à esquerda e os totais à direita.

use std::sync::Arc;

use cadcraft_doc::geom::{Polyline, Vec2, Vec3};
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Line, LwPolyline, Space, Text, VAlign};

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
/// Largura estimada de cada caractere, em alturas de texto: a fonte Stroke do CADCraft avança
/// (4 + 1,6) / 6 da altura por letra, com folga.
pub(crate) const LARGURA_CARACTERE: f64 = 0.95;

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

/// As linhas de texto da grade: a primeira é o cabeçalho, as últimas são os circuitos reserva.
fn linhas_da_tabela(r: &ResumoQuadro) -> Vec<Vec<String>> {
    let fases = usize::from(r.fases_quadro.clamp(1, 3));
    let mut cab: Vec<String> = [
        "CIRCUITO",
        "DESCRIÇÃO",
        "CORRENTE (A)",
        "TENSÃO (V)",
        "F.P",
        "POTÊNCIA REAL (W)",
        "Queda de Tensão (%)",
        "Seção do Circuito (mm²)",
        "DISJUNTOR (A)",
    ]
    .map(String::from)
    .to_vec();
    cab.extend(NOMES_FASES.iter().take(fases).map(|f| format!("FASE {f} (VA)")));
    cab.push("Isolação".into());
    let colunas = cab.len();
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
    for id in r.ids_de_reserva() {
        let mut c = vec![String::new(); colunas];
        if let Some(x) = c.first_mut() {
            *x = id;
        }
        if let Some(x) = c.get_mut(1) {
            *x = "RESERVA".into();
        }
        v.push(c);
    }
    v
}

pub(crate) fn texto(valor: &str, em: [f64; 2], altura: f64, halign: HAlign) -> EntityKind {
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

pub(crate) fn linha(a: [f64; 2], b: [f64; 2]) -> EntityKind {
    EntityKind::Line(Line { a: Vec3::new(a[0], a[1], 0.0), b: Vec3::new(b[0], b[1], 0.0) })
}

/// A largura estimada de um texto (m).
fn largura(t: &str, altura: f64) -> f64 {
    t.chars().count() as f64 * altura * LARGURA_CARACTERE
}

/// Quebra um texto em linhas que cabem em `largura_maxima` (m). Uma palavra grande demais é cortada.
fn quebrar(t: &str, largura_maxima: f64, altura: f64) -> Vec<String> {
    let cabem = ((largura_maxima / (altura * LARGURA_CARACTERE)).floor() as usize).max(1);
    let mut linhas: Vec<String> = Vec::new();
    let mut atual = String::new();
    for palavra in t.split_whitespace() {
        let mut resto: Vec<char> = palavra.chars().collect();
        while resto.len() > cabem {
            if !atual.is_empty() {
                linhas.push(std::mem::take(&mut atual));
            }
            linhas.push(resto.drain(..cabem).collect());
        }
        let palavra: String = resto.into_iter().collect();
        if atual.is_empty() {
            atual = palavra;
        } else if atual.chars().count() + 1 + palavra.chars().count() <= cabem {
            atual.push(' ');
            atual.push_str(&palavra);
        } else {
            linhas.push(std::mem::replace(&mut atual, palavra));
        }
    }
    if !atual.is_empty() || linhas.is_empty() {
        linhas.push(atual);
    }
    linhas
}

/// As larguras das colunas para os textos, com folga.
fn larguras_de(celulas: &[Vec<String>]) -> Vec<f64> {
    let colunas = celulas.first().map_or(0, Vec::len);
    (0..colunas)
        .map(|c| celulas.iter().filter_map(|l| l.get(c)).map(|t| largura(t, ALTURA)).fold(4.0 * ALTURA * LARGURA_CARACTERE, f64::max) + ALTURA)
        .collect()
}

/// Monta o conteúdo do bloco, com o canto superior esquerdo na origem, em unidades do desenho.
fn pecas_da_tabela(r: &ResumoQuadro, m: f64) -> Vec<EntityKind> {
    let u = |v: f64| v / m;
    let mut p = Vec::new();
    let hor = |p: &mut Vec<EntityKind>, x0: f64, x1: f64, y: f64| p.push(linha([u(x0), u(y)], [u(x1), u(y)]));
    let ver = |p: &mut Vec<EntityKind>, x: f64, y0: f64, y1: f64| p.push(linha([u(x), u(y0)], [u(x), u(y1)]));
    let esq = |p: &mut Vec<EntityKind>, t: &str, x: f64, y: f64| p.push(texto(t, [u(x + ALTURA / 2.0), u(y)], u(ALTURA), HAlign::Left));
    let cen = |p: &mut Vec<EntityKind>, t: &str, x: f64, w: f64, y: f64| p.push(texto(t, [u(x + w / 2.0), u(y)], u(ALTURA), HAlign::Center));

    let celulas = linhas_da_tabela(r);
    let mut larguras = larguras_de(&celulas);
    let titulo = format!("PAINEL: {}", r.tag);
    let rotulo_total = "POTÊNCIA TOTAL (VA)";
    let fases = usize::from(r.fases_quadro.clamp(1, 3));
    let primeira_fase = larguras.len().saturating_sub(fases + 1);

    // O rodapé: as duas sub-tabelas.
    let mut demanda: Vec<Vec<String>> =
        vec![["TIPO DE CARGA", "POTÊNCIA INSTALADA", "FATOR DE DEMANDA", "POTÊNCIA DEMANDADA"].map(String::from).to_vec()];
    for d in &r.demanda.linhas {
        demanda.push(vec![d.tipo.nome().to_uppercase(), fixo(d.instalada, 0), format!("{} %", fixo(d.fator * 100.0, 1)), fixo(d.demandada, 0)]);
    }
    let totais: Vec<Vec<String>> = vec![
        vec!["POTÊNCIA INSTALADA (VA)".into(), fixo(r.demanda.instalada, 0)],
        vec!["POTÊNCIA DEMANDADA (VA)".into(), fixo(r.demanda.demandada, 0)],
        vec!["CORRENTE INSTALADA (A)".into(), fixo(r.corrente_instalada, 2)],
        vec!["CORRENTE DEMANDADA (A)".into(), fixo(r.corrente_demandada, 2)],
    ];
    let mut larg_demanda = larguras_de(&demanda);
    let mut larg_totais = larguras_de(&totais);
    let (min_demanda, min_totais): (f64, f64) = (larg_demanda.iter().sum(), larg_totais.iter().sum());

    // A largura total: a grade, o dobro da maior metade do rodapé e o título. A sobra vai para a descrição.
    let total = larguras
        .iter()
        .sum::<f64>()
        .max(2.0 * min_demanda.max(min_totais))
        .max(largura(&titulo, ALTURA * 1.5) + ALTURA)
        .max(larguras.iter().take(primeira_fase).sum::<f64>().max(largura(rotulo_total, ALTURA) + ALTURA));
    let sobra = total - larguras.iter().sum::<f64>();
    if let Some(d) = larguras.get_mut(1) {
        *d += sobra;
    }
    let meia = total / 2.0;
    if let Some(c) = larg_demanda.first_mut() {
        *c += meia - min_demanda;
    }
    if let Some(c) = larg_totais.first_mut() {
        *c += meia - min_totais;
    }

    // Cabeçalho.
    let mut y = 0.0;
    let alto_titulo = LINHA * 1.5;
    p.push(texto(&titulo, [u(total / 2.0), u(y - alto_titulo / 2.0)], u(ALTURA * 1.5), HAlign::Center));
    y -= alto_titulo;
    hor(&mut p, 0.0, total, y);
    let a = &r.alimentador;
    let especificacao = format!(
        "ALIMENTADOR: {} mm² | DISJUNTOR GERAL: {} A | DR: {} A / 30 mA | DPS: classe II, Uc >= {} V",
        a.secao.map_or("-".to_string(), br),
        a.disjuntor.map_or("-".to_string(), br),
        a.dr.map_or("-".to_string(), br),
        fixo(a.dps_uc, 0)
    );
    let observacao = match &r.config.observacoes {
        Some(o) => format!("Observações: {o}"),
        None => "Observações:".to_string(),
    };
    let campos = [
        format!("ALIMENTAÇÃO: {}", r.alimentacao()),
        format!("NÚMERO DE MÓDULOS: {}", r.modulos()),
        format!("MONTAGEM: {}", r.config.montagem.map_or("-", crate::quadro::Montagem::nome)),
        observacao,
        especificacao,
    ];
    for campo in &campos {
        for l in quebrar(campo, total - ALTURA, ALTURA) {
            esq(&mut p, &l, 0.0, y - LINHA / 2.0);
            y -= LINHA;
        }
        hor(&mut p, 0.0, total, y);
    }

    // A grade dos circuitos.
    let topo = y;
    for (i, l) in celulas.iter().enumerate() {
        let mut x = 0.0;
        for (c, t) in l.iter().enumerate() {
            let w = larguras.get(c).copied().unwrap_or(0.0);
            if c == 1 && i > 0 {
                esq(&mut p, t, x, y - LINHA / 2.0);
            } else {
                cen(&mut p, t, x, w, y - LINHA / 2.0);
            }
            x += w;
        }
        y -= LINHA;
        hor(&mut p, 0.0, total, y);
    }
    let fim_da_grade = y;
    let mut x = 0.0;
    for w in larguras.iter().take(larguras.len().saturating_sub(1)) {
        x += w;
        ver(&mut p, x, topo, fim_da_grade);
    }

    // A linha de total: o rótulo ocupa até a primeira fase.
    let antes_das_fases: f64 = larguras.iter().take(primeira_fase).sum();
    esq(&mut p, rotulo_total, 0.0, y - LINHA / 2.0);
    let mut x = antes_das_fases;
    ver(&mut p, x, y, y - LINHA);
    for f in 0..fases {
        let w = larguras.get(primeira_fase + f).copied().unwrap_or(0.0);
        cen(&mut p, &fixo(r.carga_por_fase.get(f).copied().unwrap_or(0.0), 0), x, w, y - LINHA / 2.0);
        x += w;
        ver(&mut p, x, y, y - LINHA);
    }
    y -= LINHA;
    hor(&mut p, 0.0, total, y);

    // O rodapé, dividido ao meio.
    let inicio = y;
    let titulo_demanda = quebrar(&format!("DEMANDA: {} - {}", r.demanda.tabela, r.demanda.edificacao), meia - ALTURA, ALTURA);
    let linhas_esq = titulo_demanda.len() + demanda.len();
    let linhas_dir = 1 + totais.len();
    let fundo = inicio - LINHA * linhas_esq.max(linhas_dir) as f64;
    let mut ye = inicio;
    for l in &titulo_demanda {
        esq(&mut p, l, 0.0, ye - LINHA / 2.0);
        ye -= LINHA;
    }
    hor(&mut p, 0.0, meia, ye);
    let inicio_demanda = ye;
    for l in &demanda {
        let mut x = 0.0;
        for (c, t) in l.iter().enumerate() {
            let w = larg_demanda.get(c).copied().unwrap_or(0.0);
            cen(&mut p, t, x, w, ye - LINHA / 2.0);
            x += w;
        }
        ye -= LINHA;
        hor(&mut p, 0.0, meia, ye);
    }
    let mut x = 0.0;
    for w in larg_demanda.iter().take(larg_demanda.len().saturating_sub(1)) {
        x += w;
        ver(&mut p, x, inicio_demanda, fundo);
    }
    let mut yd = inicio;
    cen(&mut p, "TOTAIS DO PAINEL", meia, meia, yd - LINHA / 2.0);
    yd -= LINHA;
    hor(&mut p, meia, total, yd);
    let inicio_totais = yd;
    for l in &totais {
        let mut x = meia;
        for (c, t) in l.iter().enumerate() {
            let w = larg_totais.get(c).copied().unwrap_or(0.0);
            cen(&mut p, t, x, w, yd - LINHA / 2.0);
            x += w;
        }
        yd -= LINHA;
        hor(&mut p, meia, total, yd);
    }
    ver(&mut p, meia + larg_totais.first().copied().unwrap_or(0.0), inicio_totais, fundo);
    ver(&mut p, meia, inicio, fundo);

    // A moldura contínua, em volta de tudo.
    let cantos = [Vec2::new(0.0, 0.0), Vec2::new(u(total), 0.0), Vec2::new(u(total), u(fundo)), Vec2::new(0.0, u(fundo))];
    p.push(EntityKind::LwPolyline(LwPolyline {
        vertices: Polyline::from_points(&cantos, true).vertices,
        closed: true,
        const_width: 0.0,
        elevation: 0.0,
        plinegen: false,
    }));
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
