// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Inserir eletroduto: uma entidade só, que liga exatamente dois blocos elétricos, gravados em
//! `from_handle` e `to_handle`: `LINE` no reto, `ARC` circular no arco e `LWPOLYLINE` no 45° (nunca
//! `SPLINE`). Inserir um trecho não mexe em nenhum outro. Clicar no meio de um eletroduto
//! existente divide o trecho e insere uma caixa de passagem: é assim que se deriva no grafo
//! estrito. Um bloco no caminho do eletroduto já é uma caixa (4x2, octogonal): [`encaixar`] corta o
//! trecho nele, em `A -> bloco` e `bloco -> B`.

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::{PolyVertex, Polyline, Spline, Vec2, Vec3};
use cadcraft_doc::{Arc as Arco, Common, Drawing, Entity, EntityKind, Handle, Line, Lineweight, Space};

use crate::biblioteca;
use crate::ponto::ler_ponto;
use crate::rota::{self, Instalacao, TipoEletroduto};
use crate::setup::{PerfilEletroduto, Setup, TipoLinha};
use crate::xdata;

/// Camadas do eletroduto.
pub const CAMADA_TETO: &str = "ELE-ELETRODUTO-TETO";
pub const CAMADA_PISO: &str = "ELE-ELETRODUTO-PISO";
pub const CAMADA_PAREDE: &str = "ELE-ELETRODUTO-PAREDE";
/// Flecha padrão do arco: a distância do meio da corda ao arco, como fração da corda.
pub const FLECHA_PADRAO: f64 = 0.15;
/// A flecha vai até meia corda (meia circunferência).
pub const FLECHA_MAXIMA: f64 = 0.5;
/// Até esta distância (m) do traçado, um bloco está em linha com o eletroduto sem que ninguém tenha
/// clicado nele. É bem menor que o raio de captura: um eletroduto que só passa perto de um interruptor
/// não vira um trecho dele.
pub const TOLERANCIA_EM_LINHA: f64 = 0.05;

type P = [f64; 2];

/// A camada de uma instalação.
pub fn camada(i: Instalacao) -> &'static str {
    match i {
        Instalacao::Teto => CAMADA_TETO,
        Instalacao::Piso => CAMADA_PISO,
        Instalacao::Parede => CAMADA_PAREDE,
    }
}

/// A cor das camadas de eletroduto: a do PVC flexível. Cada trecho grava a cor do material dele.
const COR_DA_CAMADA: u8 = 3;
/// O comprimento (m) de um traço no desenho: a escala da linha faz o padrão aparecer em qualquer unidade.
const TRACO: f64 = 0.20;

/// Cria a camada com a cor padrão, se ela ainda não existe. As camadas não diferem pela cor: só pelo
/// tipo de linha do caminhamento. Uma camada com a cor padrão das versões anteriores é corrigida.
pub fn garantir_camada(doc: &mut Drawing, i: Instalacao) {
    let nome = camada(i);
    let linha = TipoLinha::do_caminhamento(i).no_desenho();
    let (cor_antiga, descricao) = match i {
        Instalacao::Teto => (4, "Eletroduto embutido no teto/laje"),
        Instalacao::Piso => (6, "Eletroduto embutido no piso"),
        Instalacao::Parede => (3, "Eletroduto embutido na parede"),
    };
    let nova = doc.ensure_layer(nome);
    garantir_tipo_de_linha(doc, linha);
    if let Some(l) = doc.layer_mut(nome)
        && (nova || l.color == Color::Index(cor_antiga))
    {
        l.color = Color::Index(COR_DA_CAMADA);
        l.linetype = linha.into();
        l.lineweight = Lineweight::Mm100(30);
        l.description = descricao.into();
    }
    // Os tipos de linha das versões anteriores (os padrões do CAD, com traço longo) viram os nossos.
    if let Some(l) = doc.layer_mut(nome)
        && ["DASHED", "DASHDOT", "DOT"].iter().any(|v| l.linetype.eq_ignore_ascii_case(v))
    {
        l.linetype = linha.into();
    }
}

/// Os tipos de linha dos eletrodutos, em metros: (nome, descrição, traços; negativo é espaço, zero é ponto).
/// Curtos e densos para a planta em 1:50: traço de 8 cm e espaço de 4 cm; no traço e ponto, traço de 8 cm,
/// espaços de 3 cm e um traço curto de 1,5 cm.
pub const TIPOS_DE_LINHA: [(&str, &str, &[f64]); 3] = [
    ("ELE-TRACEJADA", "Eletroduto na parede __ __ __", &[0.08, -0.04]),
    ("ELE-TRACO-PONTO", "Eletroduto no piso __ - __ -", &[0.08, -0.03, 0.015, -0.03]),
    ("ELE-PONTILHADA", "Eletroduto na parede . . . .", &[0.0, -0.04]),
];

/// A escala do tipo de linha no desenho: nos tipos dos eletrodutos, que já estão em metros, a da unidade;
/// nos outros, a que dá um traço de [`TRACO`].
pub(crate) fn escala_de_linha(doc: &Drawing, nome: &str) -> f64 {
    let (metros, _) = crate::setup::atual().unidade.metros(doc.header.i64("INSUNITS", 0));
    let metros = if metros > 0.0 { metros } else { 1.0 };
    if TIPOS_DE_LINHA.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(nome)) {
        return 1.0 / metros;
    }
    let traco = doc.linetype(nome).and_then(|l| l.pattern.iter().map(|p| p.length.abs()).find(|c| *c > 0.0));
    traco.map_or(1.0, |c| TRACO / metros / c)
}

/// Grava no eletroduto a cor e o tipo de linha.
fn pintar(doc: &Drawing, c: &mut Common, cor: u8, linha: TipoLinha) {
    let nome = linha.no_desenho();
    c.color = Color::Index(cor.max(1));
    c.linetype = nome.into();
    c.ltscale = escala_de_linha(doc, nome);
}

/// Pontos de um arco de circunferência de `a` a `b` (ambos incluídos). `flecha` é fração da
/// corda: positiva curva para a esquerda de quem vai de `a` para `b`, negativa para a direita,
/// zero é reta. Limitada a meia corda.
pub fn pontos_do_arco(a: P, b: P, flecha: f64, n: usize) -> Vec<P> {
    let n = n.max(2);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let corda = dx.hypot(dy);
    let reta = |t: f64| [a[0] + dx * t, a[1] + dy * t];
    let flecha = if flecha.is_finite() { flecha.clamp(-FLECHA_MAXIMA, FLECHA_MAXIMA) } else { 0.0 };
    let h = flecha * corda;
    if corda <= 0.0 || h.abs() < 1e-9 * corda.max(1.0) {
        return (0..n).map(|i| reta(i as f64 / (n - 1) as f64)).collect();
    }
    let u = [dx / corda, dy / corda];
    let lado = h.signum();
    // Do meio da corda para o lado do arco.
    let topo = [-u[1] * lado, u[0] * lado];
    let r = (corda * corda / 4.0 + h * h) / (2.0 * h.abs());
    let meio = reta(0.5);
    let centro = [meio[0] - topo[0] * (r - h.abs()), meio[1] - topo[1] * (r - h.abs())];
    let phi = (corda / (2.0 * r)).clamp(-1.0, 1.0).asin();
    (0..n)
        .map(|i| {
            let alfa = -phi + 2.0 * phi * i as f64 / (n - 1) as f64;
            let (s, c) = alfa.sin_cos();
            [centro[0] + r * (topo[0] * c + u[0] * s), centro[1] + r * (topo[1] * c + u[1] * s)]
        })
        .collect()
}

fn xdata_do_eletroduto(de: Handle, para: Handle, instalacao: Instalacao) -> Vec<cadcraft_doc::RawTag> {
    let mut x = Vec::new();
    xdata::gravar(&mut x, rota::CHAVE_ESQUEMA, Some(rota::VERSAO_ESQUEMA));
    xdata::gravar(&mut x, rota::CHAVE_DE, Some(&de.hex()));
    xdata::gravar(&mut x, rota::CHAVE_PARA, Some(&para.hex()));
    xdata::gravar(&mut x, rota::CHAVE_INSTALACAO, Some(instalacao.codigo()));
    x
}

/// Cria um eletroduto de PVC rígido em arco entre dois blocos elétricos.
pub fn inserir(doc: &mut Drawing, de: Handle, para: Handle, instalacao: Instalacao, flecha: f64) -> Result<Handle, String> {
    inserir_com_tipo(doc, de, para, instalacao, flecha, TipoEletroduto::PvcRigido)
}

/// Cria o eletroduto em arco entre dois blocos elétricos.
pub fn inserir_com_tipo(
    doc: &mut Drawing,
    de: Handle,
    para: Handle,
    instalacao: Instalacao,
    flecha: f64,
    tipo: TipoEletroduto,
) -> Result<Handle, String> {
    let tracado = if flecha.abs() < 1e-9 { Tracado::Reto } else { Tracado::Arco };
    inserir_percurso(doc, de, para, instalacao, &Percurso { tracado, flecha, ..Percurso::default() }, tipo)
}

/// Cria o eletroduto entre dois blocos elétricos, pelo percurso dado.
pub fn inserir_percurso(
    doc: &mut Drawing,
    de: Handle,
    para: Handle,
    instalacao: Instalacao,
    percurso: &Percurso,
    tipo: TipoEletroduto,
) -> Result<Handle, String> {
    let posicao = |h: Handle| ler_ponto(doc, h).map(|p| p.posicao).ok_or_else(|| format!("o objeto {} não é um bloco elétrico", h.hex()));
    let (a, b) = (posicao(de)?, posicao(para)?);
    if de == para || (a[0] - b[0]).hypot(a[1] - b[1]) <= 1e-9 {
        return Err("os dois blocos estão no mesmo lugar".into());
    }
    if !percurso.desvios.iter().flatten().all(|v| v.is_finite()) || !percurso.flecha.is_finite() {
        return Err("percurso inválido".into());
    }
    garantir_camada(doc, instalacao);
    let mut xdata_novo = xdata_do_eletroduto(de, para, instalacao);
    xdata::gravar(&mut xdata_novo, rota::CHAVE_TIPO, Some(tipo.codigo()));
    let mut common = Common { layer: camada(instalacao).into(), xdata: xdata_novo, ..Common::default() };
    pintar(doc, &mut common, tipo.cor_padrao(), TipoLinha::do_caminhamento(instalacao));
    // O traçado vai de centro a centro e para na borda dos pontos de luz.
    let mut kind = percurso.entidade(a, b);
    let corte = |h| crate::borda::contorno_de_corte(doc, h);
    if let Some(k) = crate::borda::aparado(&kind, corte(de).as_ref(), corte(para).as_ref()) {
        kind = k;
    }
    doc.add(&Space::Model, common, kind).map_err(|e| e.to_string())
}

/// Os eletrodutos gravados no bloco `antigo` passam a apontar para `novo`. Devolve quantos mudaram.
pub fn trocar_ponta(doc: &mut Drawing, antigo: Handle, novo: Handle) -> usize {
    let alvos: Vec<Entity> = doc
        .model
        .iter()
        .filter(|e| xdata::ler(&e.common.xdata).iter().any(|(k, v)| e_ponta_gravada(k) && Handle::parse_hex(v) == Some(antigo)))
        .map(|e| (**e).clone())
        .collect();
    let n = alvos.len();
    for mut e in alvos {
        for chave in [rota::CHAVE_DE, rota::CHAVE_PARA] {
            let atual = xdata::ler(&e.common.xdata).into_iter().find(|(k, _)| k.eq_ignore_ascii_case(chave)).and_then(|(_, v)| Handle::parse_hex(&v));
            if atual == Some(antigo) {
                xdata::gravar(&mut e.common.xdata, chave, Some(&novo.hex()));
            }
        }
        doc.model.replace(e);
    }
    n
}

fn e_ponta_gravada(k: &str) -> bool {
    k.eq_ignore_ascii_case(rota::CHAVE_DE) || k.eq_ignore_ascii_case(rota::CHAVE_PARA)
}

/// Corta as pontas dos eletrodutos na borda dos blocos que eles ligam (`alvos`: só esses). Devolve quantos
/// mudaram.
pub fn aparar(doc: &mut Drawing, setup: &Setup, metros: f64, alvos: Option<&[Handle]>) -> usize {
    let analise = crate::analisar(doc);
    let rede = rota::Rede::montar(doc, setup, &analise.pontos, metros);
    let mut n = 0;
    for aresta in &rede.arestas {
        if alvos.is_some_and(|l| !l.contains(&aresta.eletroduto)) {
            continue;
        }
        let (Some(de), Some(para)) = (rede.nos.get(aresta.de), rede.nos.get(aresta.para)) else { continue };
        let Some(mut e) = doc.model.get(aresta.eletroduto).map(|e| (**e).clone()) else { continue };
        let (cd, cp) = (crate::borda::contorno_de_corte(doc, de.handle), crate::borda::contorno_de_corte(doc, para.handle));
        // Nos outros blocos, a ponta que uma versão anterior cortou na borda volta ao ponto de inserção.
        let voltar: Vec<_> = [(de, &cd), (para, &cp)]
            .into_iter()
            .filter(|(_, c)| c.is_none())
            .filter_map(|(no, _)| crate::borda::contorno(doc, no.handle).map(|c| (no.pos, c)))
            .collect();
        let mut mudou = false;
        if let Some(k) = crate::borda::estendido(&e.kind, &voltar) {
            e.kind = k;
            mudou = true;
        }
        // A ponta do começo do traçado é a de `de` (o grafo lê as pontas nessa ordem).
        if let Some(k) = crate::borda::aparado(&e.kind, cd.as_ref(), cp.as_ref()) {
            e.kind = k;
            mudou = true;
        }
        if mudou {
            doc.model.replace(e);
            n += 1;
        }
    }
    n
}

/// Garante o tipo de linha no desenho, com a definição da biblioteca padrão.
pub(crate) fn garantir_tipo_de_linha(doc: &mut Drawing, nome: &str) {
    // Os dos eletrodutos são nossos: a definição vale sobre a que estiver no desenho.
    if let Some((n, descricao, tracos)) = TIPOS_DE_LINHA.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(nome)) {
        let novo = cadcraft_doc::Linetype::simple(n, descricao, tracos);
        match doc.linetypes.iter_mut().find(|l| l.name.eq_ignore_ascii_case(n)) {
            Some(l) => *l = novo,
            None => doc.linetypes.push(novo),
        }
        return;
    }
    if doc.linetype(nome).is_some() {
        return;
    }
    if let Some(l) = cadcraft_doc::library::standard_linetypes_for(doc).into_iter().find(|l| l.name.eq_ignore_ascii_case(nome)) {
        doc.linetypes.push(l);
    }
}

/// Aplica um perfil a um eletroduto: o tipo, o caminhamento, a camada, o tipo de linha e a cor.
pub fn aplicar_perfil(doc: &mut Drawing, h: Handle, perfil: &PerfilEletroduto) -> Result<(), String> {
    garantir_camada(doc, perfil.caminhamento);
    garantir_tipo_de_linha(doc, perfil.linha_efetiva().no_desenho());
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_PERFIL, Some(&perfil.nome));
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_TIPO, Some(perfil.tipo.codigo()));
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_INSTALACAO, Some(perfil.caminhamento.codigo()));
    e.common.layer = camada(perfil.caminhamento).into();
    pintar(doc, &mut e.common, perfil.cor, perfil.linha_efetiva());
    doc.model.replace(e);
    Ok(())
}

/// Cria o eletroduto entre dois blocos elétricos, com tudo o que o perfil define.
pub fn inserir_com_perfil(doc: &mut Drawing, de: Handle, para: Handle, percurso: &Percurso, perfil: &PerfilEletroduto) -> Result<Handle, String> {
    let h = inserir_percurso(doc, de, para, perfil.caminhamento, percurso, perfil.tipo)?;
    aplicar_perfil(doc, h, perfil)?;
    Ok(h)
}

/// O desenho do eletroduto entre os blocos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tracado {
    /// Em linha reta entre os pontos.
    Reto,
    /// Um trecho reto e um trecho a 45°.
    Angulo45,
    /// Em arco ou curva suave.
    #[default]
    Arco,
}

impl Tracado {
    pub const TODOS: [Tracado; 3] = [Self::Reto, Self::Angulo45, Self::Arco];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Reto => "Reto",
            Self::Angulo45 => "Ângulo 45°",
            Self::Arco => "Arco",
        }
    }

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Reto => "STRAIGHT",
            Self::Angulo45 => "ANGLE45",
            Self::Arco => "ARC",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        let t = t.trim();
        // Scripts da versão anterior ainda pedem ORTHOGONAL, que o 45° substituiu.
        if t.eq_ignore_ascii_case("ORTHOGONAL") || t.eq_ignore_ascii_case("ORTOGONAL") || t == "45" {
            return Some(Self::Angulo45);
        }
        Self::TODOS.into_iter().find(|i| i.codigo().eq_ignore_ascii_case(t) || i.nome().eq_ignore_ascii_case(t))
    }
}

/// Como desenhar um eletroduto entre dois pontos.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Percurso {
    pub tracado: Tracado,
    /// A fração da corda de um arco sem desvios: + à esquerda, - à direita, 0 reto.
    pub flecha: f64,
    /// Pontos de passagem entre os blocos, para contornar pilares e paredes.
    pub desvios: Vec<P>,
    /// No 45°, faz a diagonal primeiro e depois o trecho reto; sem isso, o contrário.
    pub diagonal_primeiro: bool,
}

impl Percurso {
    /// Os pontos do percurso, de `a` a `b`. Num arco sem desvios, os da curva.
    pub fn pontos(&self, a: P, b: P, n_arco: usize) -> Vec<P> {
        let passagem: Vec<P> = std::iter::once(a).chain(self.desvios.iter().copied()).chain(std::iter::once(b)).collect();
        match self.tracado {
            Tracado::Arco if self.desvios.is_empty() && self.flecha.abs() >= 1e-9 => pontos_do_arco(a, b, self.flecha, n_arco),
            Tracado::Arco | Tracado::Reto => passagem,
            Tracado::Angulo45 => {
                let mut v: Vec<P> = Vec::new();
                for par in passagem.windows(2) {
                    let (p, q) = (par[0], par[1]);
                    v.push(p);
                    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
                    // A diagonal cobre o menor dos dois lados; o resto é o trecho reto.
                    let diagonal = dx.abs().min(dy.abs());
                    let (sx, sy) = (dx.signum(), dy.signum());
                    let canto = if self.diagonal_primeiro {
                        [p[0] + sx * diagonal, p[1] + sy * diagonal]
                    } else {
                        [q[0] - sx * diagonal, q[1] - sy * diagonal]
                    };
                    if (canto[0] - p[0]).hypot(canto[1] - p[1]) > 1e-9 && (canto[0] - q[0]).hypot(canto[1] - q[1]) > 1e-9 {
                        v.push(canto);
                    }
                }
                v.push(b);
                v
            }
        }
    }

    /// A entidade do percurso: `LINE` reta, `ARC` circular ou `LWPOLYLINE` (o 45°, ou arcos por
    /// desvios). Nunca `SPLINE`.
    pub fn entidade(&self, a: P, b: P) -> EntityKind {
        let flecha = if self.flecha.is_finite() { self.flecha.clamp(-FLECHA_MAXIMA, FLECHA_MAXIMA) } else { 0.0 };
        if self.tracado == Tracado::Arco && flecha.abs() >= 1e-9 {
            if self.desvios.is_empty() {
                if let Some(arco) = arco_circular(a, b, flecha) {
                    return EntityKind::Arc(arco);
                }
            } else {
                // Com desvios, um arco por pedaço, todos com a mesma flecha: o bulge é 2 × flecha, com o
                // sinal contrário (o bulge positivo curva para a direita de quem anda).
                let passagem: Vec<P> = std::iter::once(a).chain(self.desvios.iter().copied()).chain(std::iter::once(b)).collect();
                let vertices = passagem
                    .iter()
                    .enumerate()
                    .map(|(i, p)| PolyVertex {
                        p: Vec2::new(p[0], p[1]),
                        bulge: if i + 1 < passagem.len() { -2.0 * flecha } else { 0.0 },
                        start_width: 0.0,
                        end_width: 0.0,
                    })
                    .collect();
                return EntityKind::LwPolyline(cadcraft_doc::LwPolyline {
                    vertices,
                    closed: false,
                    const_width: 0.0,
                    elevation: 0.0,
                    plinegen: false,
                });
            }
        }
        let pontos = self.pontos(a, b, 5);
        let v: Vec<Vec2> = pontos.iter().map(|p| Vec2::new(p[0], p[1])).collect();
        if v.len() <= 2 {
            return EntityKind::Line(Line { a: Vec3::new(a[0], a[1], 0.0), b: Vec3::new(b[0], b[1], 0.0) });
        }
        EntityKind::LwPolyline(cadcraft_doc::LwPolyline {
            vertices: Polyline::from_points(&v, false).vertices,
            closed: false,
            const_width: 0.0,
            elevation: 0.0,
            plinegen: false,
        })
    }
}

/// O arco circular de `a` a `b` com a flecha dada (fração da corda: + à esquerda de quem vai de `a` para
/// `b`). `None`: a corda é nula.
pub fn arco_circular(a: P, b: P, flecha: f64) -> Option<Arco> {
    let pts = pontos_do_arco(a, b, flecha, 3);
    let m = *pts.get(1)?;
    // O centro: o encontro das mediatrizes de a-m e m-b.
    let (ax, ay, bx, by, cx, cy) = (a[0], a[1], m[0], m[1], b[0], b[1]);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (ax * ax + ay * ay, bx * bx + by * by, cx * cx + cy * cy);
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    let r = (ax - ux).hypot(ay - uy);
    if !(ux.is_finite() && uy.is_finite() && r.is_finite() && r > 0.0) {
        return None;
    }
    let ang = |p: P| (p[1] - uy).atan2(p[0] - ux);
    let tau = std::f64::consts::TAU;
    let (ta, tm, tb) = (ang(a), ang(m), ang(b));
    // O ARC corre no sentido anti-horário: começa no ponto de onde se chega ao outro passando pelo meio.
    let (inicio, fim) = if (tm - ta).rem_euclid(tau) < (tb - ta).rem_euclid(tau) { (ta, tb) } else { (tb, ta) };
    Some(Arco { center: Vec3::new(ux, uy, 0.0), radius: r, start: inicio, end: fim })
}

/// O que o próximo trecho de ELEELETRODUTO usa: a barra de cima muda enquanto o comando corre.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeloEletroduto {
    pub instalacao: Instalacao,
    pub flecha: f64,
    pub tipo: TipoEletroduto,
    pub tracado: Tracado,
    /// O perfil escolhido na mini-barra. `None`: o tipo e a instalação acima, sem perfil.
    pub perfil: Option<String>,
}

impl ModeloEletroduto {
    /// O modelo com um perfil: o tipo e a instalação passam a ser os dele.
    pub fn com_perfil(mut self, perfil: &PerfilEletroduto) -> Self {
        self.instalacao = perfil.caminhamento;
        self.tipo = perfil.tipo;
        self.perfil = Some(perfil.nome.clone());
        self
    }
}

impl Default for ModeloEletroduto {
    fn default() -> Self {
        ModeloEletroduto {
            instalacao: Instalacao::Teto,
            flecha: FLECHA_PADRAO,
            tipo: TipoEletroduto::CorrugadoReforcado,
            tracado: Tracado::Arco,
            perfil: None,
        }
    }
}

/// O modelo de cada desenho aberto (`uid`): outro desenho no mesmo processo não usa o deste.
static MODELO: std::sync::Mutex<std::collections::BTreeMap<u64, ModeloEletroduto>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Escolhe a instalação e a flecha dos próximos eletrodutos do desenho `desenho`.
pub fn definir_modelo(desenho: u64, m: ModeloEletroduto) {
    MODELO.lock().unwrap_or_else(|e| e.into_inner()).insert(desenho, m);
}

/// A instalação e a flecha em uso no desenho `desenho`.
pub fn modelo(desenho: u64) -> ModeloEletroduto {
    MODELO.lock().unwrap_or_else(|e| e.into_inner()).get(&desenho).cloned().unwrap_or_default()
}

/// O desenho já tem um modelo escolhido, pela mini-barra ou por comando.
pub fn modelo_definido(desenho: u64) -> bool {
    MODELO.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&desenho)
}

/// O bloco elétrico mais perto de `q`, dentro do raio de captura.
pub fn bloco_em(doc: &Drawing, setup: &Setup, q: P, metros: f64) -> Option<Handle> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let raio = setup.raio_captura / metros;
    let analise = crate::analisar(doc);
    // Um ponto empilhado leva à caixa base: é nela que os eletrodutos chegam.
    let base = |h: Handle| analise.pontos.iter().find(|p| p.handle == h).and_then(|p| p.host).filter(|b| analise.ponto(*b).is_some()).unwrap_or(h);
    analise
        .pontos
        .iter()
        .map(|p| (p.handle, (p.posicao[0] - q[0]).hypot(p.posicao[1] - q[1])))
        .filter(|(_, d)| *d <= raio)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| base(h))
}

/// O eletroduto mais perto de `q`, dentro do raio de captura: (handle, ponto nele).
pub fn eletroduto_em(doc: &Drawing, setup: &Setup, q: P, metros: f64) -> Option<(Handle, P)> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let raio = setup.raio_captura / metros;
    doc.model
        .iter()
        .filter(|e| setup.e_camada_eletroduto(&e.common.layer))
        .filter_map(|e| {
            let t = rota::tracado(&e.kind, 0.005 / metros)?;
            let (p, d, _) = rota::perto_do_tracado(&t, q)?;
            (d <= raio).then_some((e.handle, p, d))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(h, p, _)| (h, p))
}

/// Uma parte do traçado cortado, com a mesma espécie de entidade quando dá.
fn parte(original: &EntityKind, pontos: &[P]) -> EntityKind {
    let v: Vec<Vec2> = pontos.iter().map(|p| Vec2::new(p[0], p[1])).collect();
    match original {
        EntityKind::Line(_) => match (pontos.first(), pontos.last()) {
            (Some(a), Some(b)) => EntityKind::Line(Line { a: Vec3::new(a[0], a[1], 0.0), b: Vec3::new(b[0], b[1], 0.0) }),
            _ => original.clone(),
        },
        EntityKind::LwPolyline(pl) => {
            let mut nova = pl.clone();
            nova.vertices = Polyline::from_points(&v, false).vertices;
            EntityKind::LwPolyline(nova)
        }
        // Um arco cortado continua arco: o mesmo centro e o mesmo raio, do primeiro ao último ponto da parte.
        EntityKind::Arc(a) => match (pontos.first(), pontos.last()) {
            (Some(p), Some(q)) => {
                let ang = |x: &P| (x[1] - a.center.y).atan2(x[0] - a.center.x);
                EntityKind::Arc(Arco { start: ang(p), end: ang(q), ..a.clone() })
            }
            _ => original.clone(),
        },
        // Arco e spline viram uma spline por até 7 pontos da parte.
        _ => {
            let passo = (v.len().saturating_sub(1) / 6).max(1);
            let mut fit: Vec<Vec2> = v.iter().step_by(passo).copied().collect();
            if let (Some(u), Some(f)) = (v.last(), fit.last())
                && u != f
            {
                fit.push(*u);
            }
            EntityKind::Spline(Spline::from_fit_points(&fit))
        }
    }
}

/// Divide o eletroduto `h` em `q` e insere uma caixa de passagem ali, ligando as duas partes a
/// ela. Devolve a caixa. Os blocos das pontas continuam os mesmos.
pub fn dividir(doc: &mut Drawing, setup: &Setup, h: Handle, q: P, metros: f64) -> Result<Handle, String> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let original = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    if !setup.e_camada_eletroduto(&original.common.layer) {
        return Err("o objeto não está numa camada de eletroduto".into());
    }
    let t = rota::tracado(&original.kind, 0.005 / metros).ok_or("o objeto não é um eletroduto aberto")?;
    let (p, _, i) = rota::perto_do_tracado(&t, q).ok_or("não achei o ponto no eletroduto")?;
    let (Some(ini), Some(fim)) = (t.first().copied(), t.last().copied()) else { return Err("eletroduto vazio".into()) };
    let folga = 0.01 / metros;
    let longe = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) > folga;
    if !longe(p, ini) || !longe(p, fim) {
        return Err("o ponto cai na ponta do eletroduto: não há o que dividir".into());
    }
    // Quem está nas pontas, pelo grafo atual (a gravação pode estar faltando).
    let analise = crate::analisar(doc);
    let rede = rota::Rede::montar(doc, setup, &analise.pontos, metros);
    let aresta = rede.arestas.iter().find(|a| a.eletroduto == h);
    let pontas = aresta.and_then(|a| Some((rede.nos.get(a.de)?.handle, rede.nos.get(a.para)?.handle)));
    let xd = xdata::ler(&original.common.xdata);
    let instalacao = rota::instalacao_de(&xd, &original.common.layer, setup);

    let z = match instalacao {
        Instalacao::Teto => setup.pe_direito,
        Instalacao::Piso => 0.0,
        Instalacao::Parede => aresta.and_then(|a| rede.nos.get(a.de)).map_or(setup.pe_direito, |n| n.z),
    };
    // A caixa é necessária aqui, mesmo que o projetista a tenha tirado da lista de blocos.
    crate::familia::marcar_excluida(doc, biblioteca::BLOCO_CAIXA, false);
    biblioteca::garantir_base(doc, setup, metros);
    let valores = vec![("ALTURA".to_string(), crate::ponto::br(z))];
    let caixa = biblioteca::inserir_ponto(doc, biblioteca::BLOCO_CAIXA, p, 0.0, &valores, setup)?;
    let novo = partir(doc, &original, &t, i, p, caixa, pontas, instalacao)?;
    aparar(doc, setup, metros, Some(&[h, novo]));
    Ok(caixa)
}

/// Corta o eletroduto `original` em `p`, no segmento `i` do traçado `t`: `de -> no` fica com o handle
/// dele e `no -> para` é uma entidade nova, que é devolvida.
#[allow(clippy::too_many_arguments)]
fn partir(
    doc: &mut Drawing,
    original: &Entity,
    t: &[P],
    i: usize,
    p: P,
    no: Handle,
    pontas: Option<(Handle, Handle)>,
    instalacao: Instalacao,
) -> Result<Handle, String> {
    let mut primeira: Vec<P> = t.iter().take(i + 1).copied().collect();
    primeira.push(p);
    let mut segunda: Vec<P> = vec![p];
    segunda.extend(t.iter().skip(i + 1).copied());

    let mut e1 = original.clone();
    e1.kind = parte(&original.kind, &primeira);
    xdata::gravar(&mut e1.common.xdata, rota::CHAVE_PARA, Some(&no.hex()));
    if let Some((de, _)) = pontas {
        xdata::gravar(&mut e1.common.xdata, rota::CHAVE_DE, Some(&de.hex()));
    }
    xdata::gravar(&mut e1.common.xdata, rota::CHAVE_INSTALACAO, Some(instalacao.codigo()));
    doc.model.replace(e1);

    let mut x2 = original.common.xdata.clone();
    xdata::gravar(&mut x2, rota::CHAVE_ESQUEMA, Some(rota::VERSAO_ESQUEMA));
    xdata::gravar(&mut x2, rota::CHAVE_DE, Some(&no.hex()));
    if let Some((_, para)) = pontas {
        xdata::gravar(&mut x2, rota::CHAVE_PARA, Some(&para.hex()));
    }
    xdata::gravar(&mut x2, rota::CHAVE_INSTALACAO, Some(instalacao.codigo()));
    xdata::gravar(&mut x2, rota::CHAVE_TAG, None);
    let c2 = Common { xdata: x2, ..original.common.clone() };
    doc.add(&Space::Model, c2, parte(&original.kind, &segunda)).map_err(|e| e.to_string())
}

/// O próximo corte: (eletroduto, bloco, posição do bloco). Um bloco das pontas do eletroduto não conta.
fn proximo_encaixe(
    doc: &Drawing,
    setup: &Setup,
    metros: f64,
    tolerancia: f64,
    blocos: Option<&[Handle]>,
    eletrodutos: Option<&[Handle]>,
) -> Option<(Handle, Handle, P)> {
    let analise = crate::analisar(doc);
    let rede = rota::Rede::montar(doc, setup, &analise.pontos, metros);
    let margem = TOLERANCIA_EM_LINHA / metros;
    let longe = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) > margem;
    for e in doc.model.iter().filter(|e| setup.e_camada_eletroduto(&e.common.layer)) {
        if eletrodutos.is_some_and(|l| !l.contains(&e.handle)) {
            continue;
        }
        let Some(t) = rota::tracado(&e.kind, 0.005 / metros) else { continue };
        let (Some(ini), Some(fim)) = (t.first().copied(), t.last().copied()) else { continue };
        let aresta = rede.arestas.iter().find(|a| a.eletroduto == e.handle);
        let e_ponta = |h: Handle| aresta.is_some_and(|a| [a.de, a.para].iter().any(|n| rede.nos.get(*n).is_some_and(|no| no.handle == h)));
        for p in analise.pontos.iter().filter(|p| p.host.is_none() && blocos.is_none_or(|l| l.contains(&p.handle)) && !e_ponta(p.handle)) {
            if let Some((q, d, _)) = rota::perto_do_tracado(&t, p.posicao)
                && d * metros <= tolerancia
                && longe(q, ini)
                && longe(q, fim)
            {
                return Some((e.handle, p.handle, p.posicao));
            }
        }
    }
    None
}

/// Liga os blocos que estão no caminho de eletrodutos: cada um corta o trecho em `A -> bloco` e
/// `bloco -> B`, pois o próprio bloco já é uma caixa. `tolerancia` em metros: [`TOLERANCIA_EM_LINHA`]
/// sem clique, o raio de captura num bloco clicado. `blocos` e `eletrodutos` restringem a busca. Devolve
/// quantos cortes fez.
pub fn encaixar(
    doc: &mut Drawing,
    setup: &Setup,
    metros: f64,
    tolerancia: f64,
    blocos: Option<&[Handle]>,
    eletrodutos: Option<&[Handle]>,
) -> Result<usize, String> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let mut lista = eletrodutos.map(<[Handle]>::to_vec);
    let mut n = 0;
    // O limite é só uma trava: cada corte encurta um trecho, e isso sempre termina.
    while n < 10_000 {
        let Some((h, bloco, pos)) = proximo_encaixe(doc, setup, metros, tolerancia, blocos, lista.as_deref()) else { break };
        let original = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
        let t = rota::tracado(&original.kind, 0.005 / metros).ok_or("o objeto não é um eletroduto aberto")?;
        let (_, _, i) = rota::perto_do_tracado(&t, pos).ok_or("não achei o ponto no eletroduto")?;
        let analise = crate::analisar(doc);
        let rede = rota::Rede::montar(doc, setup, &analise.pontos, metros);
        let pontas = rede.arestas.iter().find(|a| a.eletroduto == h).and_then(|a| Some((rede.nos.get(a.de)?.handle, rede.nos.get(a.para)?.handle)));
        let instalacao = rota::instalacao_de(&xdata::ler(&original.common.xdata), &original.common.layer, setup);
        let novo = partir(doc, &original, &t, i, pos, bloco, pontas, instalacao)?;
        if let Some(l) = &mut lista {
            l.push(novo);
        }
        n += 1;
    }
    // As partes chegam ao centro do bloco do corte: param na borda dele.
    if n > 0 {
        aparar(doc, setup, metros, None);
    }
    Ok(n)
}

/// Trava (`Some`) ou destrava (`None`) o diâmetro de um eletroduto.
pub fn travar_diametro(doc: &mut Drawing, h: Handle, diametro: Option<f64>) -> Result<(), String> {
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    match diametro {
        Some(d) => {
            if !(d.is_finite() && d > 0.0) {
                return Err("diâmetro inválido".into());
            }
            xdata::gravar(&mut e.common.xdata, rota::CHAVE_DIAMETRO, Some(&d.to_string()));
            xdata::gravar(&mut e.common.xdata, rota::CHAVE_TRAVA_DIAMETRO, Some("1"));
        }
        None => {
            xdata::gravar(&mut e.common.xdata, rota::CHAVE_TRAVA_DIAMETRO, None);
        }
    }
    doc.model.replace(e);
    Ok(())
}

/// Muda o tipo de um eletroduto.
pub fn definir_tipo(doc: &mut Drawing, h: Handle, tipo: TipoEletroduto) -> Result<(), String> {
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_TIPO, Some(tipo.codigo()));
    // O tipo escolhido à mão sai do perfil: a tabela de diâmetros passa a ser a do tipo.
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_PERFIL, None);
    e.common.color = Color::Index(tipo.cor_padrao());
    doc.model.replace(e);
    Ok(())
}

/// Muda a instalação de um eletroduto (e a camada dele).
pub fn definir_instalacao(doc: &mut Drawing, h: Handle, instalacao: Instalacao) -> Result<(), String> {
    garantir_camada(doc, instalacao);
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_INSTALACAO, Some(instalacao.codigo()));
    e.common.layer = camada(instalacao).into();
    let linha = TipoLinha::do_caminhamento(instalacao);
    let cor = match e.common.color {
        Color::Index(c) => c,
        _ => COR_DA_CAMADA,
    };
    pintar(doc, &mut e.common, cor, linha);
    doc.model.replace(e);
    Ok(())
}

/// Aplica o padrão aos eletrodutos que já estão no desenho: a cor do material (a do perfil, se houver) e
/// o tipo de linha do caminhamento. Devolve quantos mudaram.
pub fn padronizar(doc: &mut Drawing, setup: &Setup) -> usize {
    for i in [Instalacao::Teto, Instalacao::Parede, Instalacao::Piso] {
        if doc.layer(camada(i)).is_some() {
            garantir_camada(doc, i);
        }
    }
    let hs: Vec<Handle> = doc.model.iter().filter(|e| setup.e_camada_eletroduto(&e.common.layer)).map(|e| e.handle).collect();
    let mut n = 0;
    for h in hs {
        let Some(mut e) = doc.model.get(h).map(|e| (**e).clone()) else { continue };
        let xd = xdata::ler(&e.common.xdata);
        let valor = |k: &str| xd.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
        let tipo = valor(rota::CHAVE_TIPO).and_then(rota::TipoEletroduto::de_codigo).unwrap_or_default();
        let perfil = valor(rota::CHAVE_PERFIL).and_then(|p| setup.perfil(p)).filter(|p| p.tipo.mesma_familia(tipo));
        let instalacao = rota::instalacao_de(&xd, &e.common.layer, setup);
        let (cor, linha) = match perfil {
            Some(p) if p.caminhamento == instalacao => (p.cor, p.linha_efetiva()),
            Some(p) => (p.cor, TipoLinha::do_caminhamento(instalacao)),
            None => (tipo.cor_padrao(), TipoLinha::do_caminhamento(instalacao)),
        };
        garantir_tipo_de_linha(doc, linha.no_desenho());
        let antes = (e.common.color, e.common.linetype.clone());
        pintar(doc, &mut e.common, cor, linha);
        if antes != (e.common.color, e.common.linetype.clone()) {
            doc.model.replace(e);
            n += 1;
        }
    }
    n
}

/// Exclui eletrodutos, com as tags de fiação deles. Recusa, sem excluir nada, um objeto que não esteja
/// numa camada de eletroduto. Devolve quantos saíram.
pub fn excluir(doc: &mut Drawing, setup: &Setup, eletrodutos: &[Handle]) -> Result<usize, String> {
    for h in eletrodutos {
        if !doc.model.get(*h).is_some_and(|e| setup.e_camada_eletroduto(&e.common.layer)) {
            return Err(format!("o objeto {} não é um eletroduto", h.hex()));
        }
    }
    crate::fiacao::limpar(doc, Some(eletrodutos));
    Ok(eletrodutos.iter().filter(|h| doc.model.remove(**h).is_some()).count())
}
