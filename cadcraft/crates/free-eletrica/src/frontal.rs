// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! A vista frontal do quadro de distribuição: o layout de montagem dos dispositivos nos trilhos DIN, num bloco
//! `QUADRO_FRONTAL_<handle do quadro>`. O invólucro tem borda dupla (a caixa e o chassi), os barramentos de
//! neutro e de terra em pé nas laterais internas (neutro à esquerda, terra à direita), e os dispositivos lado a
//! lado nos trilhos: o geral, o DPS, cada DR
//! seguido dos circuitos dele, os circuitos sem DR e as reservas. Cada dispositivo ocupa a largura dos módulos
//! dele (1 módulo, `W`; bipolar, `2W`; tripolar, `3W`; DR tetrapolar, `4W`), e um trilho que passaria de
//! [`MODULOS_POR_TRILHO`] continua no de baixo, sem partir um dispositivo.
//!
//! É um detalhe esquemático, ampliado em relação à planta: um módulo DIN (17,5 mm) vale [`W`] no desenho.
//! Regenerar troca o conteúdo do bloco e mantém a inserção, como o unifilar.

use std::sync::Arc;

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Circle, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Space, Text, VAlign};
use serde::Serialize;

use crate::Analise;
use crate::ponto::br;
use crate::quadro::{self, ResumoQuadro};
use crate::setup::Setup;
use crate::tabela::{ALTURA_TITULO, CAMADA, LARGURA_CARACTERE, linha, texto};

/// Prefixo do bloco da vista.
pub const PREFIXO: &str = "QUADRO_FRONTAL_";
/// A largura de um módulo DIN e a altura de um dispositivo no desenho (m).
pub const W: f64 = 0.25;
const H: f64 = 0.80;
/// Sem trilho escolhido no quadro: 12 módulos num quadro pequeno; acima disso, trilhos de 18.
pub const MODULOS_POR_TRILHO_PADRAO: [u32; 2] = [12, 18];
/// O passo entre os trilhos, a folga do chassi e a borda entre a caixa e o chassi (m).
const PASSO_TRILHO: f64 = H + 0.40;
const FOLGA: f64 = 0.30;
const BORDA: f64 = 0.10;
/// A faixa de cada barramento, em cima e embaixo do chassi, ou dos lados, com eles nas laterais (m).
const FAIXA_BARRA: f64 = 0.55;
const FAIXA_LATERAL: f64 = 0.70;
/// O texto de dentro dos dispositivos (m).
const TEXTO: f64 = 0.08;
/// A distância padrão entre o quadro e a vista (m), abaixo do unifilar.
const AFASTAMENTO: [f64; 2] = [3.0, -8.0];
/// O tipo de linha das reservas.
const TRACEJADA: &str = "DASHED";
/// As cores dos barramentos (índices do CAD): azul no neutro, verde no terra.
const COR_NEUTRO: u8 = 5;
const COR_TERRA: u8 = 3;

/// O que vai num trilho.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoDispositivo {
    Geral,
    /// O disjuntor exclusivo de proteção do DPS.
    DisjuntorDps,
    Dps,
    Dr,
    Disjuntor,
    Reserva,
}

/// Um dispositivo montado no trilho.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dispositivo {
    pub tipo: TipoDispositivo,
    pub modulos: u32,
    /// O texto de cima (o circuito, `GERAL`, `DPS`, `DR 40A`) e o de baixo (a corrente).
    pub topo: String,
    pub base: String,
    /// O trilho (0 é o de cima) e o primeiro módulo dele.
    pub trilho: usize,
    pub posicao: u32,
}

/// A montagem do quadro: os dispositivos nos trilhos e os bornes dos barramentos.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Montagem {
    pub tag: String,
    pub dispositivos: Vec<Dispositivo>,
    pub trilhos: usize,
    pub modulos_por_trilho: u32,
    pub bornes_neutro: usize,
    pub bornes_terra: usize,
    /// As fases do quadro (polos do barramento das fases).
    pub fases: u32,
    pub barramento_fase: quadro::TipoBarramentoFase,
    pub posicao_nt: quadro::PosicaoBarramentoNt,
}

impl Montagem {
    /// Os módulos ocupados.
    pub fn modulos(&self) -> u32 {
        self.dispositivos.iter().map(|d| d.modulos).sum()
    }
}

/// Os dispositivos do quadro, na ordem de montagem, ainda sem trilho.
fn dispositivos(r: &ResumoQuadro) -> Vec<Dispositivo> {
    let d = |tipo, modulos: u32, topo: String, base: String| Dispositivo { tipo, modulos: modulos.max(1), topo, base, trilho: 0, posicao: 0 };
    let corrente = |v: Option<f64>| v.map_or("-".to_string(), |a| format!("{}A", br(a)));
    let fases = u32::from(r.fases_quadro.clamp(1, 3));
    let a = &r.alimentador;
    let mut v = vec![d(TipoDispositivo::Geral, u32::from(a.polos_geral.clamp(1, 3)), "GERAL".into(), corrente(a.disjuntor))];
    // Um DPS por fase (NBR 5410, 6.3.5), da classe do quadro, depois do disjuntor exclusivo dele, se houver.
    if a.dps {
        if let Some(dd) = a.disjuntor_dps {
            v.push(d(TipoDispositivo::DisjuntorDps, u32::from(a.polos_dps.clamp(1, 3)), "DPS".into(), corrente(Some(dd))));
        }
        for _ in 0..fases {
            v.push(d(TipoDispositivo::Dps, 1, "DPS".into(), format!("CLASSE {}", a.classe_dps.codigo())));
        }
    }
    let circuito =
        |l: &quadro::Linha| d(TipoDispositivo::Disjuntor, u32::from(l.esquema.fases()), l.circuito.id.clone(), corrente(l.escolha.disjuntor));
    for g in r.grupos_dr() {
        let deles: Vec<&quadro::Linha> = r.linhas.iter().filter(|l| g.circuitos.iter().any(|c| c.eq_ignore_ascii_case(&l.circuito.id))).collect();
        // Como em `ResumoQuadro::modulos`: 4 módulos com circuito multipolar, 2 nos outros.
        let modulos = if deles.iter().any(|l| l.esquema.fases() > 1) { 4 } else { 2 };
        v.push(d(TipoDispositivo::Dr, modulos, format!("DR {}", corrente(g.dr)), format!("{}mA", br(g.sensibilidade))));
        v.extend(deles.into_iter().map(circuito));
    }
    v.extend(r.linhas.iter().filter(|l| l.grupo_dr.is_none()).map(circuito));
    v.extend(r.vazios.iter().map(|x| d(TipoDispositivo::Reserva, 1, x.id.clone(), "RESERVA".into())));
    v.extend(r.ids_de_reserva().into_iter().map(|id| d(TipoDispositivo::Reserva, 1, id, "RESERVA".into())));
    v
}

/// A montagem do quadro: cada dispositivo no trilho e na posição dele.
pub fn montagem(r: &ResumoQuadro) -> Montagem {
    let mut v = dispositivos(r);
    let total: u32 = v.iter().map(|d| d.modulos).sum();
    let [pequeno, grande] = MODULOS_POR_TRILHO_PADRAO;
    let por_trilho = r.config.modulos_por_trilho.unwrap_or(if total <= pequeno { pequeno } else { grande });
    let (mut trilho, mut pos) = (0usize, 0u32);
    for d in &mut v {
        if pos > 0 && pos + d.modulos > por_trilho {
            trilho += 1;
            pos = 0;
        }
        d.trilho = trilho;
        d.posicao = pos;
        pos += d.modulos;
    }
    let com = |f: &dyn Fn(&quadro::Linha) -> bool| r.linhas.iter().filter(|l| f(l)).count();
    Montagem {
        tag: r.tag.clone(),
        trilhos: trilho + 1,
        modulos_por_trilho: por_trilho,
        // Um borne por circuito com o condutor, mais o do alimentador.
        bornes_neutro: com(&|l| l.esquema.neutro()) + 1,
        bornes_terra: com(&|l| l.esquema.protecao()) + 1,
        dispositivos: v,
        fases: u32::from(r.fases_quadro.clamp(1, 3)),
        barramento_fase: r.config.barramento_fase.unwrap_or_default(),
        // Neutro e terra sempre nas laterais internas do chassi: neutro à esquerda, terra à direita.
        posicao_nt: quadro::PosicaoBarramentoNt::Laterais,
    }
}

/// As peças do bloco, em metros até a conversão, com a cor e o tipo de linha próprios quando têm.
struct Pecas {
    lista: Vec<(EntityKind, Option<u8>, Option<&'static str>)>,
    m: f64,
}

impl Pecas {
    fn u(&self, v: f64) -> f64 {
        v / self.m
    }

    fn linha_com(&mut self, a: [f64; 2], b: [f64; 2], cor: Option<u8>, tipo: Option<&'static str>) {
        let k = linha([self.u(a[0]), self.u(a[1])], [self.u(b[0]), self.u(b[1])]);
        self.lista.push((k, cor, tipo));
    }

    fn retangulo_com(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, cor: Option<u8>, tipo: Option<&'static str>) {
        for (a, b) in [([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0])] {
            self.linha_com(a, b, cor, tipo);
        }
    }

    fn retangulo(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.retangulo_com(x0, y0, x1, y1, None, None);
    }

    fn texto_com(&mut self, t: &str, em: [f64; 2], altura: f64, alinhar: HAlign, cor: Option<u8>) {
        let k = texto(t, [self.u(em[0]), self.u(em[1])], self.u(altura), alinhar);
        self.lista.push((k, cor, None));
    }

    fn texto(&mut self, t: &str, em: [f64; 2], altura: f64) {
        self.texto_com(t, em, altura, HAlign::Center, None);
    }

    /// Um texto em pé (90°), centrado em `em`.
    fn texto_em_pe(&mut self, t: &str, em: [f64; 2], altura: f64) {
        let p = Vec3::new(self.u(em[0]), self.u(em[1]), 0.0);
        let k = EntityKind::Text(Text {
            insert: p,
            align_pt: Some(p),
            height: self.u(altura),
            value: t.to_string(),
            rotation: std::f64::consts::FRAC_PI_2,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Center,
            valign: VAlign::Middle,
        });
        self.lista.push((k, None, None));
    }

    fn circulo(&mut self, c: [f64; 2], r: f64, cor: Option<u8>) {
        let k = EntityKind::Circle(Circle { center: Vec3::new(self.u(c[0]), self.u(c[1]), 0.0), radius: self.u(r) });
        self.lista.push((k, cor, None));
    }

    /// Um barramento: a régua com os bornes, centrada em `x`, e o nome ao lado.
    fn barramento(&mut self, x: f64, y: f64, bornes: usize, nome: &str, cor: u8) {
        let passo = 0.10;
        let comprimento = passo * bornes.max(1) as f64 + passo;
        let (x0, x1) = (x - comprimento / 2.0, x + comprimento / 2.0);
        self.retangulo_com(x0, y - 0.05, x1, y + 0.05, Some(cor), None);
        for i in 0..bornes.max(1) {
            self.circulo([x0 + passo * (i as f64 + 1.0), y], 0.025, Some(cor));
        }
        self.texto_com(nome, [x, y + 0.14], TEXTO, HAlign::Center, Some(cor));
    }

    /// Um barramento vertical, nas laterais: a régua com os bornes, centrada em (`x`, `y`), e o nome em pé.
    fn barramento_vertical(&mut self, x: f64, y: f64, bornes: usize, nome: &str, cor: u8) {
        let passo = 0.10;
        let comprimento = passo * bornes.max(1) as f64 + passo;
        let (y0, y1) = (y - comprimento / 2.0, y + comprimento / 2.0);
        self.retangulo_com(x - 0.05, y0, x + 0.05, y1, Some(cor), None);
        for i in 0..bornes.max(1) {
            self.circulo([x, y1 - passo * (i as f64 + 1.0)], 0.025, Some(cor));
        }
        let p = Vec3::new(self.u(x - 0.16), self.u(y), 0.0);
        let k = EntityKind::Text(Text {
            insert: p,
            align_pt: Some(p),
            height: self.u(TEXTO),
            value: nome.to_string(),
            rotation: std::f64::consts::FRAC_PI_2,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Center,
            valign: VAlign::Middle,
        });
        self.lista.push((k, Some(cor), None));
    }

    /// O barramento das fases de um trilho, sobre os dispositivos `alimentados` (topo em `y`). Pente: uma
    /// régua por fase, com um dente para cada polo. Espinha de peixe: uma espinha só, com uma derivação para
    /// cada polo.
    fn barramento_de_fase(&mut self, mo: &Montagem, alimentados: &[&Dispositivo], inicio: f64, y: f64) {
        let polos: Vec<f64> = alimentados.iter().flat_map(|d| (0..d.modulos).map(move |k| inicio + W * (f64::from(d.posicao + k) + 0.5))).collect();
        let (Some(a), Some(b)) = (polos.iter().copied().reduce(f64::min), polos.iter().copied().reduce(f64::max)) else { return };
        let (a, b) = (a - W * 0.3, b + W * 0.3);
        match mo.barramento_fase {
            quadro::TipoBarramentoFase::Pente => {
                for f in 0..mo.fases.clamp(1, 3) {
                    let yf = y + 0.05 + 0.04 * f64::from(f);
                    self.linha_com([a, yf], [b, yf], None, None);
                }
                for (i, x) in polos.iter().enumerate() {
                    // Cada polo pega a fase seguinte: R, S, T, R...
                    let yf = y + 0.05 + 0.04 * (i as u32 % mo.fases.clamp(1, 3)) as f64;
                    self.linha_com([*x, y], [*x, yf], None, None);
                }
            }
            quadro::TipoBarramentoFase::EscamaDePeixe => {
                let espinha = y + 0.15;
                self.retangulo(a, espinha - 0.02, b, espinha + 0.02);
                for x in &polos {
                    self.linha_com([*x - 0.04, espinha - 0.02], [*x, y], None, None);
                    self.linha_com([*x + 0.04, espinha - 0.02], [*x, y], None, None);
                }
            }
        }
    }

    /// Um dispositivo no trilho, com o canto de baixo à esquerda em (`x`, `y`).
    fn dispositivo(&mut self, d: &Dispositivo, x: f64, y: f64) {
        let w = W * f64::from(d.modulos);
        let (topo, base, meio) = (y + H, y, x + w / 2.0);
        if d.tipo == TipoDispositivo::Reserva {
            // O espaço reserva: o contorno tracejado, o número em cima e RESERVA em pé.
            self.retangulo_com(x, base, x + w, topo, None, Some(TRACEJADA));
            self.texto(&d.topo, [meio, topo - 0.10], TEXTO);
            self.texto_em_pe("RESERVA", [meio, y + H / 2.0 - 0.05], TEXTO);
            return;
        }
        self.retangulo(x, base, x + w, topo);
        // As divisões entre os polos, entre os textos e a manopla, sem cortar os textos.
        let meio_y = y + H / 2.0;
        for k in 1..d.modulos {
            let xk = x + W * f64::from(k);
            self.linha_com([xk, base + 0.18], [xk, meio_y - 0.14], None, None);
            self.linha_com([xk, meio_y + 0.14], [xk, topo - 0.18], None, None);
        }
        self.texto(&d.topo, [meio, topo - 0.10], TEXTO);
        match d.tipo {
            TipoDispositivo::Dps => {
                // A classe, em pé: num módulo de 17,5 mm ela não cabe deitada.
                self.texto_em_pe(&d.base, [meio, meio_y - 0.04], 0.07);
            }
            _ => {
                self.texto(&d.base, [meio, base + 0.10], TEXTO);
                // A manopla, uma só em toda a largura num multipolar.
                let largura = (w - 0.10).min(W * 0.5 + W * 0.6 * f64::from(d.modulos - 1));
                self.retangulo(meio - largura / 2.0, meio_y - 0.10, meio + largura / 2.0, meio_y + 0.10);
                self.retangulo(meio - largura / 2.0 + 0.02, meio_y, meio + largura / 2.0 - 0.02, meio_y + 0.08);
                if d.tipo == TipoDispositivo::Dr {
                    // O botão de teste.
                    let (bx, by) = (x + w - 0.10, topo - 0.24);
                    self.retangulo(bx - 0.045, by - 0.045, bx + 0.045, by + 0.045);
                    self.texto("T", [bx, by], 0.05);
                }
            }
        }
    }
}

fn nome_do_bloco(quadro: Handle) -> String {
    format!("{PREFIXO}{}", quadro.hex())
}

/// As vistas do desenho: (inserção, quadro).
pub fn vistas(doc: &Drawing) -> Vec<(Handle, Handle)> {
    doc.model
        .iter()
        .filter_map(|e| {
            let EntityKind::Insert(i) = &e.kind else { return None };
            Some((e.handle, Handle::parse_hex(i.block.strip_prefix(PREFIXO)?)?))
        })
        .collect()
}

/// O título da vista.
pub fn titulo(tag: &str) -> String {
    format!("VISTA FRONTAL / LAYOUT DE MONTAGEM - {tag}")
}

/// O conteúdo do bloco, com o canto superior esquerdo da caixa na origem. Os barramentos de neutro e terra
/// ficam em cima e embaixo dos trilhos ou nas laterais (neutro à esquerda, terra à direita), e o das fases
/// liga os disjuntores de cada trilho: pente (uma régua por fase) ou espinha de peixe.
fn pecas(mo: &Montagem, m: f64) -> Vec<(EntityKind, Option<u8>, Option<&'static str>)> {
    let mut p = Pecas { lista: Vec::new(), m };
    let comprimento = W * f64::from(mo.modulos_por_trilho);
    let titulo = titulo(&mo.tag);
    let largura_titulo = titulo.chars().count() as f64 * ALTURA_TITULO * LARGURA_CARACTERE;
    let laterais = mo.posicao_nt == quadro::PosicaoBarramentoNt::Laterais;
    // A área dos trilhos e, em volta dela, a dos barramentos de neutro e terra.
    let trilhos_largura = if laterais {
        comprimento + 2.0 * FOLGA
    } else {
        (comprimento + 2.0 * FOLGA).max(0.10 * (mo.bornes_neutro.max(mo.bornes_terra) + 1) as f64 + 2.0 * FOLGA + 2.4)
    };
    let (margem_x, margem_y) = if laterais { (FAIXA_LATERAL, 0.15) } else { (0.0, FAIXA_BARRA) };
    let interna = trilhos_largura + 2.0 * margem_x;
    let largura = interna + 2.0 * BORDA;
    let altura_trilhos = PASSO_TRILHO * mo.trilhos as f64;
    let altura_interna = 2.0 * margem_y + altura_trilhos;
    let altura = altura_interna + 2.0 * BORDA;
    // O título, em cima da caixa.
    p.texto_com(&titulo, [largura.max(largura_titulo) / 2.0, 0.25], ALTURA_TITULO, HAlign::Center, None);
    // A caixa e o chassi (o espelho): a borda dupla.
    p.retangulo(0.0, -altura, largura, 0.0);
    p.retangulo(BORDA, -altura + BORDA, largura - BORDA, -BORDA);
    let topo = -BORDA;
    let x0 = BORDA + margem_x;
    if laterais {
        let meio_y = topo - margem_y - altura_trilhos / 2.0;
        p.barramento_vertical(BORDA + FAIXA_LATERAL / 2.0, meio_y, mo.bornes_neutro, "NEUTRO (N)", COR_NEUTRO);
        p.barramento_vertical(largura - BORDA - FAIXA_LATERAL / 2.0, meio_y, mo.bornes_terra, "TERRA (PE)", COR_TERRA);
    } else {
        let meio = largura / 2.0;
        p.barramento(meio, topo - FAIXA_BARRA / 2.0 - 0.07, mo.bornes_neutro, "BARRAMENTO DE NEUTRO (N)", COR_NEUTRO);
        p.barramento(meio, -altura + BORDA + FAIXA_BARRA / 2.0 - 0.07, mo.bornes_terra, "BARRAMENTO DE TERRA (PE)", COR_TERRA);
    }
    // Os trilhos: o perfil DIN aparece nas pontas e nos espaços sem dispositivo.
    let inicio = x0 + (trilhos_largura - comprimento) / 2.0;
    for t in 0..mo.trilhos {
        let y = topo - margem_y - PASSO_TRILHO * t as f64 - (PASSO_TRILHO - H) / 2.0 - H;
        let meio_y = y + H / 2.0;
        let ocupado: u32 = mo.dispositivos.iter().filter(|d| d.trilho == t).map(|d| d.modulos).sum();
        let fim = inicio + W * f64::from(ocupado);
        for (a, b) in [(x0 + 0.06, inicio), (fim, x0 + trilhos_largura - 0.06)] {
            if b - a > 1e-9 {
                p.retangulo(a, meio_y - 0.0875, b, meio_y + 0.0875);
            }
        }
        p.texto_com(&format!("TRILHO DIN {}", t + 1), [x0 + 0.06, y - 0.10], TEXTO, HAlign::Left, None);
        for d in mo.dispositivos.iter().filter(|d| d.trilho == t) {
            p.dispositivo(d, inicio + W * f64::from(d.posicao), y);
        }
        // O barramento das fases sobre os disjuntores e DR do trilho.
        let alimentados: Vec<&Dispositivo> =
            mo.dispositivos.iter().filter(|d| d.trilho == t && matches!(d.tipo, TipoDispositivo::Disjuntor | TipoDispositivo::Dr)).collect();
        p.barramento_de_fase(mo, &alimentados, inicio, y + H);
    }
    p.lista
}
fn gravar_bloco(doc: &mut Drawing, r: &ResumoQuadro) {
    let m = if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 };
    let nome = nome_do_bloco(r.quadro);
    let mut b = Block::new(&nome);
    crate::eletroduto::garantir_tipo_de_linha(doc, TRACEJADA);
    for (k, cor, tipo_de_linha) in pecas(&montagem(r), m) {
        let mut e = Entity::new(doc.new_handle(), k);
        if let Some(c) = cor {
            e.common.color = Color::Index(c);
        }
        if let Some(l) = tipo_de_linha {
            e.common.linetype = l.into();
            e.common.ltscale = crate::eletroduto::escala_de_linha(doc, l) * 0.5;
        }
        b.entities.push(e);
    }
    doc.blocks.insert(nome, Arc::new(b));
}

/// Gera ou atualiza a vista frontal do quadro. Com `pos`, ela vai para lá; sem ela, uma vista existente fica
/// onde está e uma nova nasce ao lado do quadro.
pub fn gerar(doc: &mut Drawing, analise: &Analise, setup: &Setup, quadro: Handle, pos: Option<[f64; 2]>) -> Result<Handle, String> {
    if pos.is_some_and(|p| !p.iter().all(|v| v.is_finite())) {
        return Err("posição inválida".into());
    }
    let r = quadro::rotear(doc, analise, setup, quadro)?;
    gravar_bloco(doc, &r);
    if let Some((h, _)) = vistas(doc).into_iter().find(|(_, q)| *q == quadro) {
        if let (Some(p), Some(mut e)) = (pos, doc.model.get(h).map(|e| (**e).clone())) {
            if let EntityKind::Insert(i) = &mut e.kind {
                i.insert = Vec3::new(p[0], p[1], 0.0);
            }
            doc.model.replace(e);
        }
        return Ok(h);
    }
    let m = if r.metros_por_unidade > 0.0 { r.metros_por_unidade } else { 1.0 };
    let junto = analise.ponto(quadro).map_or([0.0, 0.0], |q| [q.posicao[0] + AFASTAMENTO[0] / m, q.posicao[1] + AFASTAMENTO[1] / m]);
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

/// Refaz as vistas do desenho, sem mexer na inserção delas. A de um quadro que não existe mais sai. Devolve
/// quantas foram refeitas.
pub fn regenerar(doc: &mut Drawing, analise: &Analise, setup: &Setup) -> Result<usize, String> {
    let mut n = 0;
    for (h, q) in vistas(doc) {
        match quadro::rotear(doc, analise, setup, q) {
            Ok(r) => {
                gravar_bloco(doc, &r);
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
