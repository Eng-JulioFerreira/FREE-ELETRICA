// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Inserir eletroduto: uma spline em arco (ou uma linha, com flecha zero) que liga exatamente dois
//! blocos elétricos, gravados em `from_handle` e `to_handle`. Clicar no meio de um eletroduto
//! existente divide o trecho e insere uma caixa de passagem: é assim que se deriva no grafo
//! estrito. Um bloco no caminho do eletroduto já é uma caixa (4x2, octogonal): [`encaixar`] corta o
//! trecho nele, em `A -> bloco` e `bloco -> B`.

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::{Polyline, Spline, Vec2, Vec3};
use cadcraft_doc::{Common, Drawing, Entity, EntityKind, Handle, Line, Lineweight, Space};

use crate::biblioteca;
use crate::ponto::ler_ponto;
use crate::rota::{self, Instalacao, TipoEletroduto};
use crate::setup::{PerfilEletroduto, Setup};
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

/// Cria a camada com a cor padrão, se ela ainda não existe.
pub fn garantir_camada(doc: &mut Drawing, i: Instalacao) {
    let nome = camada(i);
    if doc.ensure_layer(nome)
        && let Some(l) = doc.layer_mut(nome)
    {
        let (cor, descricao) = match i {
            Instalacao::Teto => (4, "Eletroduto embutido no teto/laje"),
            Instalacao::Piso => (6, "Eletroduto embutido no piso"),
            Instalacao::Parede => (3, "Eletroduto embutido na parede"),
        };
        l.color = Color::Index(cor);
        l.lineweight = Lineweight::Mm100(30);
        l.description = descricao.into();
    }
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
    let common = Common { layer: camada(instalacao).into(), xdata: xdata_novo, ..Common::default() };
    doc.add(&Space::Model, common, percurso.entidade(a, b)).map_err(|e| e.to_string())
}

/// Garante o tipo de linha no desenho, com a definição da biblioteca padrão.
fn garantir_tipo_de_linha(doc: &mut Drawing, nome: &str) {
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
    garantir_tipo_de_linha(doc, perfil.tipo_linha.no_desenho());
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_PERFIL, Some(&perfil.nome));
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_TIPO, Some(perfil.tipo.codigo()));
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_INSTALACAO, Some(perfil.caminhamento.codigo()));
    e.common.layer = camada(perfil.caminhamento).into();
    e.common.linetype = perfil.tipo_linha.no_desenho().into();
    e.common.color = Color::Index(perfil.cor.max(1));
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

    /// A entidade do percurso.
    pub fn entidade(&self, a: P, b: P) -> EntityKind {
        let pontos = self.pontos(a, b, 5);
        let v: Vec<Vec2> = pontos.iter().map(|p| Vec2::new(p[0], p[1])).collect();
        if self.tracado == Tracado::Arco && (!self.desvios.is_empty() || self.flecha.abs() >= 1e-9) {
            return EntityKind::Spline(Spline::from_fit_points(&v));
        }
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
    crate::analisar(doc)
        .pontos
        .iter()
        .map(|p| (p.handle, (p.posicao[0] - q[0]).hypot(p.posicao[1] - q[1])))
        .filter(|(_, d)| *d <= raio)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
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
    biblioteca::garantir_base(doc, setup, metros);
    let valores = vec![("ALTURA".to_string(), crate::ponto::br(z))];
    let caixa = biblioteca::inserir_ponto(doc, biblioteca::BLOCO_CAIXA, p, 0.0, &valores, setup)?;
    partir(doc, &original, &t, i, p, caixa, pontas, instalacao)?;
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
        for p in analise.pontos.iter().filter(|p| blocos.is_none_or(|l| l.contains(&p.handle)) && !e_ponta(p.handle)) {
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
    doc.model.replace(e);
    Ok(())
}

/// Muda a instalação de um eletroduto (e a camada dele).
pub fn definir_instalacao(doc: &mut Drawing, h: Handle, instalacao: Instalacao) -> Result<(), String> {
    garantir_camada(doc, instalacao);
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("eletroduto não encontrado")?;
    xdata::gravar(&mut e.common.xdata, rota::CHAVE_INSTALACAO, Some(instalacao.codigo()));
    e.common.layer = camada(instalacao).into();
    doc.model.replace(e);
    Ok(())
}
