// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A lista de materiais: cabos por isolação, seção e função (a cor do condutor), eletrodutos por tipo e
//! diâmetro, caixas, pontos e disjuntores, lidos do desenho.
//!
//! O cabo é medido condutor a condutor em cada trecho de eletroduto (planta e desníveis até os blocos), vezes
//! a folga de materiais: as fases que o circuito usa no quadro, cada uma na cor dela, o neutro, os retornos
//! que passam no trecho e o condutor de proteção (um só por trecho quando ele é compartilhado). Assim a compra
//! sai em rolos por cor.

use std::collections::{BTreeMap, BTreeSet};

use cadcraft_doc::{Drawing, Handle};
use serde::Serialize;

use crate::nbr5410::TipoCabo;
use crate::ponto::br;
use crate::rota::TipoEletroduto;
use crate::setup::Setup;
use crate::{Analise, fiacao, quadro};

/// O comprimento de um rolo de cabo (m).
pub const ROLO: f64 = 100.0;

/// A função de um condutor, que dá a cor dele (NBR 5410, 6.1.5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FuncaoCondutor {
    FaseR,
    FaseS,
    FaseT,
    Neutro,
    Retorno,
    Protecao,
}

impl FuncaoCondutor {
    pub const TODAS: [FuncaoCondutor; 6] = [Self::FaseR, Self::FaseS, Self::FaseT, Self::Neutro, Self::Retorno, Self::Protecao];

    /// A fase do quadro (0 = R, 1 = S, 2 = T).
    pub fn da_fase(f: usize) -> Self {
        match f {
            0 => Self::FaseR,
            1 => Self::FaseS,
            _ => Self::FaseT,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::FaseR => "Fase R",
            Self::FaseS => "Fase S",
            Self::FaseT => "Fase T",
            Self::Neutro => "Neutro",
            Self::Retorno => "Retorno",
            Self::Protecao => "Terra / PE",
        }
    }

    /// A cor do condutor pelas configurações: o neutro é sempre azul-claro.
    pub fn cor(self, setup: &Setup) -> String {
        let fase = |i: usize| setup.cores_fase.get(i).map_or("-", |c| c.nome()).to_string();
        match self {
            Self::FaseR => fase(0),
            Self::FaseS => fase(1),
            Self::FaseT => fase(2),
            Self::Neutro => crate::setup::COR_NEUTRO.to_string(),
            Self::Retorno => setup.cor_retorno.nome().to_string(),
            Self::Protecao => setup.cor_protecao.nome().to_string(),
        }
    }
}

/// Metros de cabo de um tipo (isolação), uma seção e uma função (a cor).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCabo {
    pub secao: f64,
    pub tipo: TipoCabo,
    /// O nome do cabo no catálogo.
    pub nome: String,
    pub funcao: FuncaoCondutor,
    pub cor: String,
    pub metros: f64,
    /// Rolos de 100 m que cobrem a metragem.
    pub rolos: u32,
    /// O item de compra: `Cabo flexível PVC 70 °C (450/750 V) #2,5 mm² - Branco (Fase S)`.
    pub descricao: String,
}

impl ItemCabo {
    fn novo(nome: &str, tipo: TipoCabo, secao: f64, funcao: FuncaoCondutor, setup: &Setup) -> Self {
        let cor = funcao.cor(setup);
        let descricao = format!("{nome} #{} mm² - {cor} ({})", br(secao), funcao.nome());
        ItemCabo { secao, tipo, nome: nome.to_string(), funcao, cor, metros: 0.0, rolos: 0, descricao }
    }
}

/// Soma os cabos condutor a condutor: (nome do cabo, seção em milésimos, função) → item.
struct Cabos<'a> {
    itens: BTreeMap<(String, i64, FuncaoCondutor), ItemCabo>,
    setup: &'a Setup,
}

impl Cabos<'_> {
    fn somar(&mut self, nome: &str, tipo: TipoCabo, secao: f64, funcao: FuncaoCondutor, metros: f64) {
        if !(secao.is_finite() && secao > 0.0 && metros.is_finite() && metros > 0.0) {
            return;
        }
        let chave = (nome.to_string(), (secao * 1000.0).round() as i64, funcao);
        let setup = self.setup;
        self.itens.entry(chave).or_insert_with(|| ItemCabo::novo(nome, tipo, secao, funcao, setup)).metros += metros;
    }

    /// Os condutores de um circuito num trecho de `metros` (já com a folga).
    fn trecho(&mut self, k: &fiacao::Condutores, metros: f64) {
        use fiacao::TipoCondutor as T;
        let Some(secao) = k.secao else { return };
        for f in k.fases_no_trecho() {
            self.somar(&k.nome_cabo, k.tipo_cabo, secao, FuncaoCondutor::da_fase(f), metros);
        }
        if k.neutro {
            self.somar(&k.nome_cabo, k.tipo_cabo, k.secao_de(T::Neutro).unwrap_or(secao), FuncaoCondutor::Neutro, metros);
        }
        let retornos = k.retornos.len() as f64;
        if retornos > 0.0 {
            self.somar(&k.nome_cabo, k.tipo_cabo, secao, FuncaoCondutor::Retorno, metros * retornos);
        }
        if k.protecao {
            self.somar(&k.nome_cabo, k.tipo_cabo, k.secao_de(T::Protecao).unwrap_or(secao), FuncaoCondutor::Protecao, metros);
        }
    }

    /// Um circuito sem eletrodutos até o quadro: o percurso estimado do roteamento, sem os retornos.
    fn estimado(&mut self, l: &quadro::Linha) {
        let Some(secao) = l.escolha.secao else { return };
        let metros = if l.condutores > 0 { l.cabo / f64::from(l.condutores) } else { 0.0 };
        let k = fiacao::Condutores {
            fases: l.esquema.fases(),
            neutro: l.esquema.neutro(),
            protecao: l.esquema.protecao(),
            secao: Some(secao),
            secao_neutro: l.secao_neutro,
            secao_pe: l.secao_pe,
            tipo_cabo: l.tipo_cabo,
            fases_do_quadro: l.fases.clone(),
            nome_cabo: l.nome_cabo.clone(),
            ..fiacao::Condutores::default()
        };
        self.trecho(&k, metros);
    }

    fn lista(self) -> Vec<ItemCabo> {
        self.itens
            .into_values()
            .map(|mut c| {
                c.rolos = (c.metros / ROLO).ceil().max(0.0) as u32;
                c
            })
            .collect()
    }
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
    /// Disjuntores por polos, corrente e curva: os dos circuitos, o geral e o do DPS.
    pub disjuntores: Vec<ItemContado>,
    /// Os outros componentes dos quadros: DR, DPS, barramentos e as caixas com a capacidade em módulos DIN.
    pub quadros: Vec<ItemContado>,
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
    let mut disjuntores: Vec<String> = Vec::new();
    let mut componentes: Vec<String> = Vec::new();
    let mut grupos: Vec<String> = Vec::new();
    let mut resumos: Vec<quadro::ResumoQuadro> = Vec::new();
    for q in analise.quadros() {
        match quadro::rotear(doc, analise, setup, q.handle) {
            Ok(r) => {
                for l in &r.linhas {
                    disjuntores.extend(l.escolha.disjuntor.map(|d| nome_do_disjuntor(l.esquema.fases(), d, Some(l.curva))));
                }
                componentes_do_quadro(&r, &mut disjuntores, &mut componentes);
                grupos.extend(r.linhas.iter().filter_map(|l| l.grupo_dr.as_ref().map(|g| format!("{}: {g}", r.tag))));
                resumos.push(r);
            }
            Err(e) => m.avisos.push(e),
        }
    }
    m.disjuntores = contar(disjuntores.into_iter());
    m.quadros = contar(componentes.into_iter());
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

    // Os cabos, condutor a condutor em cada trecho. O condutor de proteção compartilhado já saiu do trecho.
    let mut cabos = Cabos { itens: BTreeMap::new(), setup };
    let mut medidos: BTreeSet<(Handle, String)> = BTreeSet::new();
    for t in &trechos {
        for k in &t.circuitos {
            cabos.trecho(k, t.comprimento * setup.fator_materiais);
            medidos.insert((k.quadro, k.circuito.to_uppercase()));
        }
    }
    for r in &resumos {
        for l in &r.linhas {
            if l.escolha.secao.is_none() {
                m.avisos.push(format!("{}, circuito {}: sem seção, os cabos dele ficam fora da lista", r.tag, l.circuito.id));
            } else if !medidos.contains(&(r.quadro, l.circuito.id.to_uppercase())) {
                // Sem eletrodutos até o quadro: o percurso estimado do quadro de cargas.
                cabos.estimado(l);
                m.avisos.push(format!(
                    "{}, circuito {}: sem eletrodutos até o quadro, cabo pelo percurso estimado e sem os retornos",
                    r.tag, l.circuito.id
                ));
            } else if l.fora_da_rede > 0 {
                m.avisos.push(format!(
                    "{}, circuito {}: {} ponto(s) fora da rede de eletrodutos, sem o cabo deles",
                    r.tag, l.circuito.id, l.fora_da_rede
                ));
            }
        }
    }
    m.cabos = cabos.lista();

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

// ---------------------------------------------------------------- a tabela na prancha

/// O bloco da tabela da lista de materiais: uma por desenho.
pub const BLOCO_TABELA: &str = "LISTA_MATERIAIS";
/// A altura de cada linha da tabela (m), para a planta em 1:50 com 1 unidade = 1 m.
const LINHA_TABELA: f64 = 0.26;
/// As colunas: o item, a quantidade, a unidade e os rolos de 100 m dos cabos.
const CABECALHO: [&str; 4] = ["ITEM", "QUANTIDADE", "UNIDADE", "ROLOS DE 100 m"];

/// Uma linha da tabela: um título de grupo (as outras colunas vazias) ou um item.
fn grupo(nome: &str) -> [String; 4] {
    [nome.to_uppercase(), String::new(), String::new(), String::new()]
}

/// As linhas da tabela da lista de materiais, por grupo, sem o cabeçalho.
pub fn linhas_da_tabela(m: &Materiais) -> Vec<[String; 4]> {
    let mut v: Vec<[String; 4]> = Vec::new();
    let contados = |v: &mut Vec<[String; 4]>, nome: &str, lista: &[ItemContado]| {
        if !lista.is_empty() {
            v.push(grupo(nome));
            v.extend(lista.iter().map(|i| [i.nome.clone(), i.quantidade.to_string(), "un".into(), String::new()]));
        }
    };
    if !m.cabos.is_empty() {
        v.push(grupo("Cabos"));
        v.extend(m.cabos.iter().map(|c| [c.descricao.clone(), format!("{:.0}", c.metros.ceil()), "m".into(), c.rolos.to_string()]));
    }
    if !m.eletrodutos.is_empty() {
        v.push(grupo("Eletrodutos"));
        v.extend(m.eletrodutos.iter().map(|e| {
            let nome = e.diametro.map_or_else(|| e.tipo.nome().to_string(), |d| format!("{} Ø{}", e.tipo.nome(), br(d)));
            [nome, format!("{:.1}", e.metros).replace('.', ","), "m".into(), String::new()]
        }));
    }
    contados(&mut v, "Caixas", &m.caixas);
    // Os pontos pela descrição comercial; sem nenhuma, pelo tipo.
    if m.conjuntos.is_empty() {
        contados(&mut v, "Pontos", &m.pontos);
    } else {
        contados(&mut v, "Pontos", &m.conjuntos);
    }
    contados(&mut v, "Disjuntores", &m.disjuntores);
    contados(&mut v, "Quadros e proteções", &m.quadros);
    if !m.cabos_telecom.is_empty() || !m.telecom.is_empty() {
        v.push(grupo("Telecom"));
        v.extend(m.cabos_telecom.iter().map(|c| [c.nome.clone(), format!("{:.0}", c.metros.ceil()), "m".into(), String::new()]));
        v.extend(m.telecom.iter().map(|i| [i.nome.clone(), i.quantidade.to_string(), "un".into(), String::new()]));
    }
    v
}

/// O conteúdo do bloco, com o canto superior esquerdo na origem, em unidades do desenho.
fn pecas_da_tabela(m: &Materiais, metros: f64) -> Vec<cadcraft_doc::EntityKind> {
    use crate::tabela::{ALTURA, ALTURA_TITULO, LARGURA_CARACTERE, linha, texto};
    use cadcraft_doc::HAlign;
    let u = |v: f64| v / metros;
    let largura_de = |t: &str| t.chars().count() as f64 * ALTURA * LARGURA_CARACTERE;
    let linhas = linhas_da_tabela(m);
    let titulo = "LISTA DE MATERIAIS";
    // A largura de cada coluna cabe o maior texto dela.
    let mut larguras = [0.0f64; 4];
    for (c, l) in larguras.iter_mut().enumerate() {
        let maior = linhas.iter().filter_map(|x| x.get(c)).map(|t| largura_de(t)).fold(largura_de(CABECALHO[c]), f64::max);
        *l = maior + 2.0 * ALTURA;
    }
    let soma: f64 = larguras.iter().sum();
    let total = soma.max(titulo.chars().count() as f64 * ALTURA_TITULO * LARGURA_CARACTERE + ALTURA);
    if let Some(primeira) = larguras.first_mut() {
        *primeira += total - soma;
    }
    let mut p = Vec::new();
    let alto_titulo = LINHA_TABELA * 1.5;
    p.push(texto(titulo, [u(total / 2.0), u(-alto_titulo / 2.0)], u(ALTURA_TITULO), HAlign::Center));
    let mut y = -alto_titulo;
    let topo = y;
    let linha_de = |p: &mut Vec<cadcraft_doc::EntityKind>, celulas: &[String], y: f64, titulo_de_grupo: bool| {
        let mut x = 0.0;
        for (c, t) in celulas.iter().enumerate() {
            let w = larguras.get(c).copied().unwrap_or(0.0);
            let meio = y - LINHA_TABELA / 2.0;
            if c == 0 || titulo_de_grupo {
                p.push(texto(t, [u(x + ALTURA), u(meio)], u(ALTURA), HAlign::Left));
            } else {
                p.push(texto(t, [u(x + w / 2.0), u(meio)], u(ALTURA), HAlign::Center));
            }
            x += w;
        }
    };
    p.push(linha([0.0, u(y)], [u(total), u(y)]));
    linha_de(&mut p, &CABECALHO.map(String::from), y, false);
    y -= LINHA_TABELA;
    p.push(linha([0.0, u(y)], [u(total), u(y)]));
    // As divisões verticais não cortam os títulos de grupo.
    let mut faixas: Vec<(f64, f64)> = vec![(topo, y)];
    for l in &linhas {
        let titulo_de_grupo = l.iter().skip(1).all(String::is_empty);
        linha_de(&mut p, l, y, titulo_de_grupo);
        if !titulo_de_grupo {
            faixas.push((y, y - LINHA_TABELA));
        }
        y -= LINHA_TABELA;
        p.push(linha([0.0, u(y)], [u(total), u(y)]));
    }
    let mut x = 0.0;
    for w in larguras.iter().take(larguras.len().saturating_sub(1)) {
        x += w;
        for (a, b) in &faixas {
            p.push(linha([u(x), u(*a)], [u(x), u(*b)]));
        }
    }
    // A moldura.
    for (a, b) in [([0.0, 0.0], [total, 0.0]), ([total, 0.0], [total, y]), ([total, y], [0.0, y]), ([0.0, y], [0.0, 0.0])] {
        p.push(linha([u(a[0]), u(a[1])], [u(b[0]), u(b[1])]));
    }
    p
}

fn gravar_tabela(doc: &mut Drawing, m: &Materiais, metros: f64) {
    let mut b = cadcraft_doc::Block::new(BLOCO_TABELA);
    for k in pecas_da_tabela(m, metros) {
        b.entities.push(cadcraft_doc::Entity::new(doc.new_handle(), k));
    }
    doc.blocks.insert(BLOCO_TABELA.to_string(), std::sync::Arc::new(b));
}

/// A inserção da tabela da lista de materiais.
pub fn tabela_inserida(doc: &Drawing) -> Option<Handle> {
    doc.model.iter().find(|e| matches!(&e.kind, cadcraft_doc::EntityKind::Insert(i) if i.block == BLOCO_TABELA)).map(|e| e.handle)
}

/// Desenha a tabela da lista de materiais em `pos`. Se o desenho já tem uma, ela é refeita e movida para lá.
pub fn inserir_tabela(doc: &mut Drawing, analise: &Analise, setup: &Setup, pos: [f64; 2]) -> Result<Handle, String> {
    use cadcraft_doc::geom::Vec3;
    if !pos.iter().all(|v| v.is_finite()) {
        return Err("posição inválida".into());
    }
    let (metros, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let m = levantar(doc, analise, setup);
    gravar_tabela(doc, &m, metros);
    if let Some(h) = tabela_inserida(doc)
        && let Some(mut e) = doc.model.get(h).map(|e| (**e).clone())
    {
        if let cadcraft_doc::EntityKind::Insert(i) = &mut e.kind {
            i.insert = Vec3::new(pos[0], pos[1], 0.0);
        }
        doc.model.replace(e);
        return Ok(h);
    }
    let ins = cadcraft_doc::Insert {
        block: BLOCO_TABELA.into(),
        insert: Vec3::new(pos[0], pos[1], 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
        rotation: 0.0,
        attribs: Vec::new(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    };
    doc.ensure_layer(crate::tabela::CAMADA);
    let comum = cadcraft_doc::Common { layer: crate::tabela::CAMADA.into(), ..cadcraft_doc::Common::default() };
    doc.add(&cadcraft_doc::Space::Model, comum, cadcraft_doc::EntityKind::Insert(ins)).map_err(|e| e.to_string())
}

/// Refaz a tabela da lista de materiais, se o desenho tem uma, sem mexer na inserção. Devolve 1 se refez.
pub fn regenerar_tabela(doc: &mut Drawing, analise: &Analise, setup: &Setup) -> usize {
    if tabela_inserida(doc).is_none() {
        return 0;
    }
    let (metros, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    let m = levantar(doc, analise, setup);
    gravar_tabela(doc, &m, if metros > 0.0 { metros } else { 1.0 });
    1
}

/// O nome de compra de um disjuntor: `Disjuntor 2P 20 A - curva C`.
pub fn nome_do_disjuntor(polos: u8, corrente: f64, curva: Option<crate::nbr5410::Curva>) -> String {
    let curva = curva.map_or(String::new(), |c| format!(" - curva {}", c.codigo()));
    format!("Disjuntor {}P {} A{curva}", polos.clamp(1, 3), crate::ponto::br(corrente))
}

/// O geral, o disjuntor do DPS, os DR, os DPS, os barramentos e a caixa de um quadro, pela montagem dele.
fn componentes_do_quadro(r: &quadro::ResumoQuadro, disjuntores: &mut Vec<String>, componentes: &mut Vec<String>) {
    use crate::frontal::TipoDispositivo as T;
    let a = &r.alimentador;
    disjuntores.extend(a.disjuntor.map(|d| nome_do_disjuntor(a.polos_geral, d, Some(a.curva_geral))));
    disjuntores.extend(a.disjuntor_dps.map(|d| nome_do_disjuntor(a.polos_dps, d, Some(crate::nbr5410::Curva::C))));
    let mo = crate::frontal::montagem(r);
    let grupos = r.grupos_dr();
    let mut drs = grupos.iter();
    for d in &mo.dispositivos {
        match d.tipo {
            // Os DR aparecem na montagem na ordem dos grupos.
            T::Dr => {
                if let Some(g) = drs.next() {
                    componentes.push(format!("Interruptor DR {}P {} A - {} mA", d.modulos, g.dr.map_or("-".to_string(), br), br(g.sensibilidade)));
                }
            }
            T::Dps => componentes.push(format!("DPS Classe {} - {} V - {} kA", a.classe_dps.codigo(), br(a.dps_tensao), br(a.dps_imax))),
            _ => {}
        }
    }
    componentes.push(format!("Barramento de neutro - {} bornes", mo.bornes_neutro));
    componentes.push(format!("Barramento de terra - {} bornes", mo.bornes_terra));
    let tipo = match mo.barramento_fase {
        quadro::TipoBarramentoFase::Pente => "pente",
        quadro::TipoBarramentoFase::EscamaDePeixe => "espinha de peixe",
    };
    for t in 0..mo.trilhos {
        let polos: u32 = mo.dispositivos.iter().filter(|d| d.trilho == t && matches!(d.tipo, T::Disjuntor | T::Dr)).map(|d| d.modulos).sum();
        if polos > 0 {
            componentes.push(format!("Barramento {tipo} {}F - {polos} polos", mo.fases));
        }
    }
    let capacidade = mo.modulos_por_trilho * u32::try_from(mo.trilhos).unwrap_or(1);
    let montagem = r.config.montagem.map_or(String::new(), |m| format!(" {}", m.nome()));
    componentes.push(format!("Quadro de distribuição{montagem} - {capacidade} módulos DIN ({} trilhos de {})", mo.trilhos, mo.modulos_por_trilho));
}
