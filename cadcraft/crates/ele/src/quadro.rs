//! Quadro de cargas: os circuitos dimensionados e distribuídos nas fases do quadro.

use serde::Serialize;

use crate::Analise;
use crate::circuito::Circuito;
use crate::nbr5410::{self, Config, Dimensionamento};
use crate::ponto::{PontoEletrico, TipoPonto};

pub const NOMES_FASES: [&str; 3] = ["A", "B", "C"];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Linha {
    pub circuito: Circuito,
    pub dimensionamento: Dimensionamento,
    /// Fases do quadro que o circuito usa: índices em [`NOMES_FASES`].
    pub fases: Vec<usize>,
}

impl Linha {
    pub fn fases_texto(&self) -> String {
        self.fases.iter().map(|f| NOMES_FASES[*f]).collect::<Vec<_>>().join("+")
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadroDeCargas {
    /// A TAG do quadro do desenho, quando há um.
    pub quadro: Option<String>,
    /// Fases disponíveis no quadro.
    pub fases_quadro: u8,
    pub linhas: Vec<Linha>,
    /// Potência instalada (VA).
    pub potencia_total: f64,
    /// Carga por fase (VA).
    pub carga_por_fase: [f64; 3],
    /// (maior − menor) / maior entre as fases usadas, em %.
    pub desequilibrio: f64,
    pub avisos: Vec<String>,
}

/// Fases do quadro: atributo NUMERO_FASES/FASES; senão, "220/127V" indica bifásico.
fn fases_do_quadro(q: Option<&PontoEletrico>) -> u8 {
    let Some(q) = q else { return 2 };
    if let Some(f) = q.fases {
        return f;
    }
    let tensao = q.atributos.iter().find(|(t, _)| t.eq_ignore_ascii_case("TENSAO")).map(|(_, v)| v.as_str()).unwrap_or("");
    if tensao.contains('/') { 2 } else { 1 }
}

pub fn montar(analise: &Analise, cfg: &Config) -> QuadroDeCargas {
    let mut avisos = Vec::new();
    let quadros: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| p.tipo == TipoPonto::Quadro).collect();
    match quadros.len() {
        0 => avisos.push("nenhum quadro no desenho: supondo quadro bifásico".into()),
        1 => {}
        n => avisos.push(format!("{n} quadros no desenho: por enquanto todos os circuitos vão para o primeiro")),
    }
    let quadro = quadros.first().copied();
    let fases_quadro = fases_do_quadro(quadro).clamp(1, 3);

    let mut linhas: Vec<Linha> =
        analise.circuitos.iter().map(|c| Linha { circuito: c.clone(), dimensionamento: nbr5410::dimensionar(c, cfg), fases: Vec::new() }).collect();

    // Balanceamento: maiores cargas primeiro, cada uma nas fases menos carregadas.
    let mut carga = [0.0f64; 3];
    let mut ordem: Vec<usize> = (0..linhas.len()).collect();
    ordem.sort_by(|a, b| linhas[*b].circuito.potencia.total_cmp(&linhas[*a].circuito.potencia));
    for i in ordem {
        let pedidas = linhas[i].circuito.fases;
        if pedidas > fases_quadro {
            avisos.push(format!("circuito {} pede {pedidas} fases e o quadro tem {fases_quadro}", linhas[i].circuito.id));
        }
        let n = usize::from(pedidas.min(fases_quadro));
        let mut livres: Vec<usize> = (0..usize::from(fases_quadro)).collect();
        livres.sort_by(|a, b| carga[*a].total_cmp(&carga[*b]));
        let escolhidas: Vec<usize> = livres.into_iter().take(n).collect();
        for f in &escolhidas {
            carga[*f] += linhas[i].circuito.potencia / n as f64;
        }
        let mut escolhidas = escolhidas;
        escolhidas.sort_unstable();
        linhas[i].fases = escolhidas;
    }

    let usadas = &carga[..usize::from(fases_quadro)];
    let maior = usadas.iter().copied().fold(0.0, f64::max);
    let menor = usadas.iter().copied().fold(f64::INFINITY, f64::min);
    let desequilibrio = if maior > 0.0 && fases_quadro > 1 { (maior - menor) / maior * 100.0 } else { 0.0 };

    QuadroDeCargas {
        quadro: quadro.map(PontoEletrico::rotulo),
        fases_quadro,
        potencia_total: linhas.iter().map(|l| l.circuito.potencia).sum(),
        linhas,
        carga_por_fase: carga,
        desequilibrio,
        avisos,
    }
}
