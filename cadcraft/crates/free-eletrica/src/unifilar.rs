// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! O diagrama unifilar de um quadro no desenho: um bloco `UNIFILAR_<handle do quadro>`. À esquerda, o
//! alimentador com fase, neutro e terra, e o disjuntor geral até a barra. À direita, os ramais: os circuitos
//! sem DR saem da barra; os de um grupo saem da sub-barra depois do módulo DR; no fim, as reservas. Cada
//! ramal tem o disjuntor, os traços de fiação, a potência, a fase e uma caixa com o circuito.
//! Regenerar troca o conteúdo do bloco e mantém a inserção, como a tabela do quadro.

use std::sync::Arc;

use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Arc as Arco, Block, Circle, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space};

use crate::Analise;
use crate::ponto::br;
use crate::quadro::{self, Linha, ResumoQuadro};
use crate::setup::Setup;
use crate::tabela::{CAMADA, linha, texto};

/// Prefixo do bloco do diagrama.
pub const PREFIXO: &str = "UNIFILAR_";
/// Altura do texto (m).
const ALTURA: f64 = 0.10;
/// Distância entre os ramais e a folga a mais depois de cada grupo de DR (m).
const PASSO: f64 = 0.55;
const FOLGA_GRUPO: f64 = 0.20;
/// As colunas do diagrama, da esquerda para a direita (m).
const BARRA: f64 = 2.0;
const DR_X: f64 = BARRA + 0.30;
const DR_LARGURA: f64 = 0.45;
const SUB_BARRA: f64 = BARRA + 1.15;
const DISJUNTOR_X: f64 = SUB_BARRA + 0.30;
const DISJUNTOR_LARGURA: f64 = 0.20;
const MARCAS_X: f64 = DISJUNTOR_X + 1.00;
const DADOS_X: f64 = MARCAS_X + 0.60;
const CAIXA_X: f64 = DADOS_X + 1.40;
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

/// As peças do bloco, em metros até a conversão, com a camada própria quando têm (as marcas de fiação vão
/// na camada das tags da prancha).
struct Pecas {
    lista: Vec<(EntityKind, Option<&'static str>)>,
    m: f64,
}

impl Pecas {
    fn u(&self, v: f64) -> f64 {
        v / self.m
    }

    fn linha(&mut self, a: [f64; 2], b: [f64; 2]) {
        let k = linha([self.u(a[0]), self.u(a[1])], [self.u(b[0]), self.u(b[1])]);
        self.lista.push((k, None));
    }

    fn texto(&mut self, t: &str, em: [f64; 2], altura: f64, alinhar: HAlign) {
        let k = texto(t, [self.u(em[0]), self.u(em[1])], self.u(altura), alinhar);
        self.lista.push((k, None));
    }

    fn retangulo(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        for (a, b) in [([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0])] {
            self.linha(a, b);
        }
    }

    /// O disjuntor termomagnético: os dois terminais na linha, o arco da abertura entre eles e, no topo do
    /// arco, o traço em T do disparo. De `x` a `x + largura`.
    fn disjuntor(&mut self, x: f64, y: f64, largura: f64) {
        let r = largura / 2.0;
        let c = Vec3::new(self.u(x + r), self.u(y), 0.0);
        let k = EntityKind::Arc(Arco { center: c, radius: self.u(r), start: 0.0, end: std::f64::consts::PI });
        self.lista.push((k, None));
        for t in [x, x + largura] {
            let k = EntityKind::Circle(Circle { center: Vec3::new(self.u(t), self.u(y), 0.0), radius: self.u(0.018) });
            self.lista.push((k, None));
        }
        let topo = y + r;
        self.linha([x + r, topo], [x + r, topo + 0.04]);
        self.linha([x + r - 0.025, topo + 0.04], [x + r + 0.025, topo + 0.04]);
    }

    /// Um nó circular na barra.
    fn no(&mut self, x: f64, y: f64) {
        let (cx, cy, r) = (self.u(x), self.u(y), self.u(0.03));
        self.lista.push((EntityKind::Circle(Circle { center: Vec3::new(cx, cy, 0.0), radius: r }), None));
        self.lista.push((crate::biblioteca::disco(cx, cy, r), None));
    }

    /// As marcas de fiação na linha, centradas em `x`: os mesmos blocos e a mesma camada das tags da prancha.
    fn marcas(&mut self, x: f64, y: f64, fases: u8, neutro: bool, terra: bool) {
        use crate::fiacao::TipoCondutor as T;
        let mut tipos: Vec<T> = vec![T::Fase; usize::from(fases)];
        if neutro {
            tipos.push(T::Neutro);
        }
        if terra {
            tipos.push(T::Protecao);
        }
        let passo = 0.04;
        let inicio = x - passo * tipos.len().saturating_sub(1) as f64 / 2.0;
        for (i, tipo) in tipos.iter().enumerate() {
            let k = EntityKind::Insert(Insert {
                block: tipo.bloco().into(),
                insert: Vec3::new(self.u(inicio + passo * i as f64), self.u(y), 0.0),
                scale: Vec3::new(1.0, 1.0, 1.0),
                rotation: 0.0,
                attribs: Vec::new(),
                cols: 1,
                rows: 1,
                col_spacing: 0.0,
                row_spacing: 0.0,
            });
            self.lista.push((k, Some(crate::fiacao::CAMADA)));
        }
    }
}

/// O texto da caixa de um circuito.
fn rotulo(l: &Linha) -> String {
    if l.descricao.is_empty() { l.circuito.id.clone() } else { format!("{} - {}", l.circuito.id, l.descricao) }
}

/// Um ramal: o disjuntor, os traços, os dados e a caixa, a partir de `inicio`, que é a barra de onde ele sai.
struct Ramal<'a> {
    y: f64,
    /// `None`: reserva.
    linha: Option<&'a Linha>,
    caixa: String,
}

fn desenhar_ramal(p: &mut Pecas, ramal: &Ramal, inicio: f64, largura_caixa: f64) {
    let y = ramal.y;
    p.linha([inicio, y], [DISJUNTOR_X, y]);
    p.no(inicio, y);
    p.disjuntor(DISJUNTOR_X, y, DISJUNTOR_LARGURA);
    p.linha([DISJUNTOR_X + DISJUNTOR_LARGURA, y], [CAIXA_X, y]);
    let meio_disjuntor = DISJUNTOR_X + DISJUNTOR_LARGURA / 2.0;
    match ramal.linha {
        Some(l) => {
            let polos = l.esquema.fases();
            let corrente = l.escolha.disjuntor.map_or("-".to_string(), br);
            p.texto(&format!("{polos}P/{corrente}A"), [meio_disjuntor, y + 0.20], ALTURA * 0.8, HAlign::Center);
            p.texto(&format!("Curva {}", l.curva.codigo()), [meio_disjuntor, y - 0.14], ALTURA * 0.7, HAlign::Center);
            p.marcas(MARCAS_X, y, polos, l.esquema.neutro(), l.esquema.protecao());
            p.texto(&l.circuito.id, [MARCAS_X, y + 0.16], ALTURA * 0.8, HAlign::Center);
            p.texto(&l.escolha.secao.map_or("#-".to_string(), |s| format!("#{}", br(s))), [MARCAS_X, y - 0.16], ALTURA * 0.8, HAlign::Center);
            p.texto(&format!("POT: {} VA", br(l.circuito.potencia.round())), [DADOS_X, y + 0.11], ALTURA * 0.8, HAlign::Left);
            p.texto(&format!("FASE: {}", l.fases_texto()), [DADOS_X, y - 0.11], ALTURA * 0.8, HAlign::Left);
        }
        None => p.texto("1P", [meio_disjuntor, y + 0.20], ALTURA * 0.8, HAlign::Center),
    }
    p.retangulo(CAIXA_X, y - 0.14, CAIXA_X + largura_caixa, y + 0.14);
    p.texto(&ramal.caixa, [CAIXA_X + ALTURA / 2.0, y], ALTURA, HAlign::Left);
}

/// O conteúdo do bloco, com o canto superior esquerdo na origem, em unidades do desenho.
fn pecas(r: &ResumoQuadro, alimentado_por: &str, m: f64) -> Vec<(EntityKind, Option<&'static str>)> {
    let mut p = Pecas { lista: Vec::new(), m };
    p.texto(&format!("PAINEL: {}", r.tag), [0.0, -0.08], ALTURA * 1.5, HAlign::Left);
    p.texto(&format!("POT. INSTALADA: {} VA", br(r.demanda.instalada.round())), [0.0, -0.30], ALTURA, HAlign::Left);
    p.texto(&format!("POT. DEMANDADA: {} VA", br(r.demanda.demandada.round())), [0.0, -0.46], ALTURA, HAlign::Left);
    p.texto(&format!("{} | Alimentado por: {alimentado_por}", r.alimentacao()), [0.0, -0.62], ALTURA * 0.8, HAlign::Left);

    // Onde cada ramal fica: os grupos de DR, os circuitos sem DR e as reservas.
    let grupos = r.grupos_dr();
    let mut y = -1.15;
    let mut faixas: Vec<(usize, f64, f64)> = Vec::new();
    let mut ramais_grupo: Vec<Ramal> = Vec::new();
    for (i, g) in grupos.iter().enumerate() {
        let inicio = y;
        for l in r.linhas.iter().filter(|l| g.circuitos.iter().any(|c| c.eq_ignore_ascii_case(&l.circuito.id))) {
            ramais_grupo.push(Ramal { y, linha: Some(l), caixa: rotulo(l) });
            y -= PASSO;
        }
        faixas.push((i, inicio, y + PASSO));
        y -= FOLGA_GRUPO;
    }
    let mut ramais_barra: Vec<Ramal> = Vec::new();
    for l in r.linhas.iter().filter(|l| l.grupo_dr.is_none()) {
        ramais_barra.push(Ramal { y, linha: Some(l), caixa: rotulo(l) });
        y -= PASSO;
    }
    for id in r.ids_de_reserva() {
        ramais_barra.push(Ramal { y, linha: None, caixa: format!("{id} - RESERVA") });
        y -= PASSO;
    }
    let largura_caixa = ramais_grupo
        .iter()
        .chain(&ramais_barra)
        .map(|x| x.caixa.chars().count() as f64 * ALTURA * crate::tabela::LARGURA_CARACTERE + ALTURA)
        .fold(1.0, f64::max);

    // Os nós da barra principal: um por grupo de DR e um por ramal direto.
    let mut nos: Vec<f64> = faixas.iter().map(|(_, a, b)| (a + b) / 2.0).collect();
    nos.extend(ramais_barra.iter().map(|x| x.y));
    let topo = nos.iter().copied().fold(-1.15, f64::max);
    let fundo = nos.iter().copied().fold(-1.15, f64::min);
    let entrada = (topo + fundo) / 2.0;
    p.linha([BARRA, topo], [BARRA, fundo]);

    // A entrada: o alimentador com fase, neutro e terra, e o disjuntor geral até a barra.
    let geral_x = 1.15;
    let neutro = r.config.sistema != Some(crate::quadro::SistemaQuadro::BifasicoSemNeutro);
    p.linha([0.0, entrada], [geral_x, entrada]);
    p.marcas(0.45, entrada, r.fases_quadro.clamp(1, 3), neutro, true);
    let a = &r.alimentador;
    p.texto(&a.secao.map_or("#-".to_string(), |s| format!("#{}", br(s))), [0.45, entrada - 0.18], ALTURA * 0.8, HAlign::Center);
    p.disjuntor(geral_x, entrada, 0.26);
    p.texto(&format!("In: {} A", a.disjuntor.map_or("-".to_string(), br)), [geral_x + 0.13, entrada + 0.24], ALTURA * 0.8, HAlign::Center);
    p.linha([geral_x + 0.26, entrada], [BARRA, entrada]);
    p.no(BARRA, entrada);

    // Os grupos de DR: o módulo e a sub-barra de onde saem os circuitos dele.
    for (i, y0, y1) in &faixas {
        let Some(g) = grupos.get(*i) else { continue };
        let meio = (y0 + y1) / 2.0;
        p.no(BARRA, meio);
        p.linha([BARRA, meio], [DR_X, meio]);
        p.retangulo(DR_X, meio - 0.12, DR_X + DR_LARGURA, meio + 0.12);
        p.texto("DR", [DR_X + DR_LARGURA / 2.0, meio], ALTURA, HAlign::Center);
        p.linha([DR_X + DR_LARGURA, meio], [SUB_BARRA, meio]);
        let corrente = g.dr.map_or("-".to_string(), br);
        p.texto(&format!("IN: {corrente}A / 30mA"), [DR_X + DR_LARGURA / 2.0, meio + 0.20], ALTURA * 0.7, HAlign::Center);
        p.texto(&g.nome, [DR_X + DR_LARGURA / 2.0, meio - 0.20], ALTURA * 0.7, HAlign::Center);
        p.linha([SUB_BARRA, *y0], [SUB_BARRA, *y1]);
        p.no(SUB_BARRA, meio);
    }
    for ramal in &ramais_grupo {
        desenhar_ramal(&mut p, ramal, SUB_BARRA, largura_caixa);
    }
    for ramal in &ramais_barra {
        desenhar_ramal(&mut p, ramal, BARRA, largura_caixa);
    }
    p.lista
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
    crate::fiacao::garantir_blocos(doc, m);
    for (k, camada) in pecas(r, &quem_alimenta(r, analise), m) {
        let mut e = Entity::new(doc.new_handle(), k);
        if let Some(c) = camada {
            e.common.layer = c.into();
        }
        b.entities.push(e);
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
