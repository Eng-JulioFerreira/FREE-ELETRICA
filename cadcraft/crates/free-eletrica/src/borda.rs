// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! O eletroduto encosta na borda dos blocos: a linha ou o arco começa e termina no contorno do símbolo
//! (o círculo de uma luminária, o retângulo de uma caixa ou de um quadro), sem passar por cima dos textos
//! de dentro. O vínculo com os blocos continua no XDATA (`from_handle`, `to_handle`), e o grafo liga a
//! ponta ao bloco cujo contorno ela toca. O comprimento soma o pedaço até o centro do bloco.

use cadcraft_doc::geom::{PolyVertex, Vec2};
use cadcraft_doc::{Drawing, EntityKind, Handle};

type P = [f64; 2];
/// Um pedaço do traçado: de, até e o bulge (0 reto; + anti-horário).
type Seg = (P, P, f64);

/// A forma do contorno de um bloco, nas coordenadas dele.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Forma {
    /// Um círculo na origem: luminárias e símbolos redondos.
    Circulo(f64),
    /// O retângulo do desenho: caixas, quadros, pontos de parede.
    Caixa(P, P),
}

/// O contorno de uma inserção, no desenho.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contorno {
    origem: P,
    rotacao: f64,
    escala: P,
    forma: Forma,
}

impl Contorno {
    /// A maior distância do contorno ao ponto de inserção, no desenho.
    pub fn alcance(&self) -> f64 {
        let e = self.escala[0].abs().max(self.escala[1].abs());
        e * match self.forma {
            Forma::Circulo(r) => r,
            Forma::Caixa(min, max) => [min[0].abs().max(max[0].abs()), min[1].abs().max(max[1].abs())].iter().fold(0.0, |s: f64, v| s.hypot(*v)),
        }
    }

    /// `q` (no desenho) cai dentro do contorno, que cresce `folga` para fora.
    pub fn dentro(&self, q: P, folga: f64) -> bool {
        let (s, c) = self.rotacao.sin_cos();
        let (dx, dy) = (q[0] - self.origem[0], q[1] - self.origem[1]);
        let x = (dx * c + dy * s) / self.escala[0];
        let y = (-dx * s + dy * c) / self.escala[1];
        let f = folga / self.escala[0].abs().min(self.escala[1].abs());
        // A borda conta como dentro: um eletroduto que sai rente à face de um quadro também é cortado.
        match self.forma {
            Forma::Circulo(r) => x.hypot(y) <= r + f,
            Forma::Caixa(min, max) => x >= min[0] - f && x <= max[0] + f && y >= min[1] - f && y <= max[1] + f,
        }
    }
}

/// O desenho do bloco (sem atributos nem textos), nas coordenadas dele em relação a `base`: os pontos do
/// retângulo que o cobre, o raio do círculo na base que o cobre, e se ele tem um círculo ou arco na base.
fn medidas_do_desenho(b: &cadcraft_doc::Block, base: P) -> (Vec<P>, f64, bool) {
    let mut v: Vec<P> = Vec::new();
    let mut raio: f64 = 0.0;
    let mut redondo = false;
    let rel = |x: f64, y: f64| [x - base[0], y - base[1]];
    let mut redondo_de = |c: P, r: f64, v: &mut Vec<P>, raio: &mut f64| {
        redondo |= c[0].hypot(c[1]) < 1e-6;
        *raio = raio.max(c[0].hypot(c[1]) + r);
        v.extend([[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]]);
    };
    for e in b.entities.iter() {
        let antes = v.len();
        match &e.kind {
            EntityKind::Line(l) => v.extend([rel(l.a.x, l.a.y), rel(l.b.x, l.b.y)]),
            EntityKind::Circle(k) => redondo_de(rel(k.center.x, k.center.y), k.radius, &mut v, &mut raio),
            EntityKind::Arc(a) => redondo_de(rel(a.center.x, a.center.y), a.radius, &mut v, &mut raio),
            EntityKind::LwPolyline(p) => v.extend(p.vertices.iter().map(|x| rel(x.p.x, x.p.y))),
            EntityKind::Solid(s) => v.extend(s.corners.iter().map(|c| rel(c.x, c.y))),
            _ => {}
        }
        // Os cantos de círculos e arcos já entraram no raio pelo centro; o resto, ponto a ponto.
        if !matches!(e.kind, EntityKind::Circle(_) | EntityKind::Arc(_)) {
            raio = v.iter().skip(antes).map(|p| p[0].hypot(p[1])).fold(raio, f64::max);
        }
    }
    (v.into_iter().filter(|p| p[0].is_finite() && p[1].is_finite()).collect(), raio, redondo)
}

/// O contorno da inserção `h`. `None`: não é um bloco, ou o bloco não tem desenho.
pub fn contorno(doc: &Drawing, h: Handle) -> Option<Contorno> {
    let EntityKind::Insert(ins) = &doc.model.get(h)?.kind else { return None };
    let b = doc.blocks.get(&ins.block)?;
    let (pts, raio, redondo) = medidas_do_desenho(b, [b.base.x, b.base.y]);
    let forma = if redondo {
        // O círculo que cobre todo o desenho: o símbolo e o que estiver em volta dele.
        Forma::Circulo(raio)
    } else {
        let min = [pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min), pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min)];
        let max = [pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max), pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max)];
        Forma::Caixa(min, max)
    };
    let valida = match forma {
        Forma::Circulo(r) => r.is_finite() && r > 1e-9,
        Forma::Caixa(min, max) => (0..2).all(|i| max[i].is_finite() && min[i].is_finite() && max[i] - min[i] > 1e-9),
    };
    let escala = [ins.scale.x, ins.scale.y];
    (valida && escala.iter().all(|s| s.is_finite() && s.abs() > 1e-12) && ins.rotation.is_finite()).then_some(Contorno {
        origem: [ins.insert.x, ins.insert.y],
        rotacao: ins.rotation,
        escala,
        forma,
    })
}

/// O bloco `h` é um ponto de luz (luminária de teto ou arandela): só nele o eletroduto para na borda. Nos
/// outros (tomadas, interruptores, caixas, quadros) ele vai até o ponto de inserção, encostado na caixa.
pub fn corta_na_borda(doc: &Drawing, h: Handle) -> bool {
    crate::ponto::ler_ponto(doc, h)
        .is_some_and(|p| p.tipo == crate::ponto::TipoPonto::Iluminacao || p.categoria == Some(crate::familia::FamilyCategory::Luminaria))
}

/// O contorno em que o eletroduto para: o do bloco, se ele for um ponto de luz.
pub fn contorno_de_corte(doc: &Drawing, h: Handle) -> Option<Contorno> {
    if corta_na_borda(doc, h) { contorno(doc, h) } else { None }
}

/// Leva de volta ao ponto de inserção a ponta que um corte antigo deixou na borda de um bloco que não é
/// ponto de luz. `alvos`: (ponto de inserção, contorno) dos blocos assim. `None`: nada a mudar. No arco a
/// ponta não sai do círculo dele: fica como está.
pub fn estendido(k: &EntityKind, alvos: &[(P, Contorno)]) -> Option<EntityKind> {
    let original = segmentos(k)?;
    let mut segs = original.clone();
    let perto = |q: P| alvos.iter().find(|(o, c)| (q[0] - o[0]).hypot(q[1] - o[1]) > 1e-9 && c.dentro(q, 1e-6)).map(|(o, _)| *o);
    let reto = |s: &Seg| s.2.abs() < 1e-12;
    if let Some(s) = segs.first_mut()
        && reto(s)
        && let Some(o) = perto(s.0)
    {
        s.0 = o;
    }
    if let Some(s) = segs.last_mut()
        && reto(s)
        && let Some(o) = perto(s.1)
    {
        s.1 = o;
    }
    if segs == original || matches!(k, EntityKind::Arc(_)) {
        return None;
    }
    com_segmentos(k, &segs)
}

/// O ponto do pedaço `s` no parâmetro `t` (0 a 1). No arco, anda o ângulo.
fn ponto(s: &Seg, t: f64) -> P {
    let (a, b, bulge) = *s;
    if bulge.abs() < 1e-12 {
        return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    }
    let theta = 4.0 * bulge.atan();
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = dx.hypot(dy);
    if d < 1e-12 {
        return a;
    }
    // O centro fica à esquerda da corda, a (1 - b²)/(4b) da corda do meio dela.
    let h = d * (1.0 - bulge * bulge) / (4.0 * bulge);
    let centro = [(a[0] + b[0]) / 2.0 - dy / d * h, (a[1] + b[1]) / 2.0 + dx / d * h];
    let (s, c) = (theta * t).sin_cos();
    let (vx, vy) = (a[0] - centro[0], a[1] - centro[1]);
    [centro[0] + vx * c - vy * s, centro[1] + vx * s + vy * c]
}

/// O traçado como pedaços. `None`: não é um eletroduto que dê para cortar.
fn segmentos(k: &EntityKind) -> Option<Vec<Seg>> {
    match k {
        EntityKind::Line(l) => Some(vec![([l.a.x, l.a.y], [l.b.x, l.b.y], 0.0)]),
        EntityKind::Arc(a) => {
            let mut varrido = (a.end - a.start).rem_euclid(std::f64::consts::TAU);
            if varrido < 1e-12 {
                varrido = std::f64::consts::TAU;
            }
            let em = |ang: f64| [a.center.x + a.radius * ang.cos(), a.center.y + a.radius * ang.sin()];
            Some(vec![(em(a.start), em(a.end), (varrido / 4.0).tan())])
        }
        EntityKind::LwPolyline(p) if !p.closed && p.vertices.len() >= 2 => {
            Some(p.vertices.windows(2).map(|w| ([w[0].p.x, w[0].p.y], [w[1].p.x, w[1].p.y], w[0].bulge)).collect())
        }
        _ => None,
    }
}

/// A entidade com os pedaços novos, da mesma espécie.
fn com_segmentos(original: &EntityKind, segs: &[Seg]) -> Option<EntityKind> {
    let (Some(primeiro), Some(ultimo)) = (segs.first(), segs.last()) else { return None };
    Some(match original {
        EntityKind::Line(l) => {
            let mut n = l.clone();
            n.a.x = primeiro.0[0];
            n.a.y = primeiro.0[1];
            n.b.x = ultimo.1[0];
            n.b.y = ultimo.1[1];
            EntityKind::Line(n)
        }
        EntityKind::Arc(a) => {
            let ang = |p: P| (p[1] - a.center.y).atan2(p[0] - a.center.x);
            let mut n = a.clone();
            n.start = ang(primeiro.0);
            n.end = ang(ultimo.1);
            EntityKind::Arc(n)
        }
        EntityKind::LwPolyline(p) => {
            let mut n = p.clone();
            n.vertices = segs
                .iter()
                .map(|s| PolyVertex { p: Vec2::new(s.0[0], s.0[1]), bulge: s.2, start_width: 0.0, end_width: 0.0 })
                .chain(std::iter::once(PolyVertex { p: Vec2::new(ultimo.1[0], ultimo.1[1]), bulge: 0.0, start_width: 0.0, end_width: 0.0 }))
                .collect();
            EntityKind::LwPolyline(n)
        }
        _ => return None,
    })
}

/// Corta o começo do traçado onde ele sai de `dentro`. `None`: o começo já está fora, ou o traçado não sai.
fn cortar_inicio(segs: &[Seg], dentro: &dyn Fn(P) -> bool) -> Option<Vec<Seg>> {
    let inicio = segs.first()?.0;
    if !dentro(inicio) {
        return None;
    }
    const AMOSTRAS: usize = 32;
    for (k, s) in segs.iter().enumerate() {
        let mut antes = 0.0;
        for i in 1..=AMOSTRAS {
            let t = i as f64 / AMOSTRAS as f64;
            if dentro(ponto(s, t)) {
                antes = t;
                continue;
            }
            // Saiu entre `antes` e `t`: a borda, por bisseção.
            let (mut lo, mut hi) = (antes, t);
            for _ in 0..40 {
                let m = (lo + hi) / 2.0;
                if dentro(ponto(s, m)) { lo = m } else { hi = m }
            }
            let corte = ponto(s, hi);
            // Já na borda (de um corte anterior): nada a cortar.
            if k == 0 && (corte[0] - inicio[0]).hypot(corte[1] - inicio[1]) < 1e-9 {
                return None;
            }
            let bulge = if s.2.abs() < 1e-12 { 0.0 } else { (4.0 * s.2.atan() * (1.0 - hi) / 4.0).tan() };
            let mut v = vec![(corte, s.1, bulge)];
            v.extend(segs.iter().skip(k + 1).copied());
            return Some(v);
        }
    }
    None
}

/// O traçado ao contrário.
fn inverter(segs: &[Seg]) -> Vec<Seg> {
    segs.iter().rev().map(|s| (s.1, s.0, -s.2)).collect()
}

/// O traçado `k` cortado nos contornos dos dois blocos que ele liga, cada ponta no contorno em que ela cai (um
/// `ARC` começa pelo ângulo inicial, que pode ser o do bloco de chegada). `None`: nada a cortar.
pub fn aparado(k: &EntityKind, a: Option<&Contorno>, b: Option<&Contorno>) -> Option<EntityKind> {
    let original = segmentos(k)?;
    let (inicio, fim) = (original.first()?.0, original.last()?.1);
    let em = |q: P| [a, b].into_iter().flatten().find(|c| c.dentro(q, 0.0));
    let (de, para) = (em(inicio), em(fim));
    let mut segs = original.clone();
    if let Some(c) = de
        && let Some(v) = cortar_inicio(&segs, &|q| c.dentro(q, 0.0))
    {
        segs = v;
    }
    if let Some(c) = para
        && let Some(v) = cortar_inicio(&inverter(&segs), &|q| c.dentro(q, 0.0))
    {
        segs = inverter(&v);
    }
    if segs == original {
        return None;
    }
    // O que sobra precisa ter comprimento: dois blocos encostados não perdem o eletroduto.
    let (a, b) = (segs.first()?.0, segs.last()?.1);
    if segs.len() == 1 && (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9 {
        return None;
    }
    com_segmentos(k, &segs)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ponto_no_arco_pelo_bulge() {
        // Meia volta anti-horária de (1,0) a (-1,0): passa por (0,1)... à direita de quem anda, (0,-1).
        let s: Seg = ([1.0, 0.0], [-1.0, 0.0], 1.0);
        let m = ponto(&s, 0.5);
        assert!(m[0].abs() < 1e-9 && (m[1].abs() - 1.0).abs() < 1e-9, "{m:?}");
        assert!((ponto(&s, 1.0)[0] + 1.0).abs() < 1e-9);
    }
}
