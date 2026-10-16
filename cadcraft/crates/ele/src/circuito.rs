//! Circuitos: os pontos agrupados pelo atributo CIRCUITO.

use cadcraft_doc::Handle;
use serde::Serialize;

use crate::ponto::{PontoEletrico, TipoPonto};

/// O que o circuito alimenta, pelos tipos dos seus pontos de carga.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Natureza {
    Iluminacao,
    Tug,
    Tue,
    /// Iluminação e tomadas no mesmo circuito.
    Misto,
    /// Só interruptores ou caixas: nenhuma carga.
    SemCarga,
}

impl Natureza {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Iluminacao => "Iluminação",
            Self::Tug => "TUG",
            Self::Tue => "TUE",
            Self::Misto => "Misto",
            Self::SemCarga => "Sem carga",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Circuito {
    pub id: String,
    #[serde(skip)]
    pub pontos: Vec<Handle>,
    pub natureza: Natureza,
    /// Soma das potências dos pontos de carga, em VA.
    pub potencia: f64,
    /// Tensão dos pontos (a maior, se divergirem). `None`: nenhum ponto informou.
    pub tensao: Option<f64>,
    /// Fases do circuito: a maior entre os pontos, 1 se nenhum informou.
    pub fases: u8,
    pub cargas: usize,
    pub interruptores: usize,
    pub avisos: Vec<String>,
}

/// Agrupa os pontos por circuito, em ordem natural ("2" antes de "10").
pub fn agrupar(pontos: &[PontoEletrico]) -> Vec<Circuito> {
    let mut ids: Vec<&str> = pontos.iter().filter(|p| p.tipo != TipoPonto::Quadro).filter_map(|p| p.circuito.as_deref()).collect();
    ids.sort_by(|a, b| ordem_natural(a, b));
    ids.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    ids.into_iter()
        .map(|id| {
            let membros: Vec<&PontoEletrico> =
                pontos.iter().filter(|p| p.tipo != TipoPonto::Quadro && p.circuito.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(id))).collect();
            montar(id, &membros)
        })
        .collect()
}

fn montar(id: &str, membros: &[&PontoEletrico]) -> Circuito {
    let cargas: Vec<&&PontoEletrico> = membros.iter().filter(|p| p.tipo.e_carga()).collect();
    let tem = |t: TipoPonto| cargas.iter().any(|p| p.tipo == t);
    let (ilum, tug, tue) = (tem(TipoPonto::Iluminacao), tem(TipoPonto::Tomada), tem(TipoPonto::TomadaEspecifica));
    let natureza = match (ilum, tug || tem(TipoPonto::Outro), tue) {
        (false, false, false) => Natureza::SemCarga,
        (true, false, false) => Natureza::Iluminacao,
        (false, true, false) => Natureza::Tug,
        (false, false, true) => Natureza::Tue,
        _ => Natureza::Misto,
    };
    let mut avisos = Vec::new();
    let tensoes: Vec<f64> = cargas.iter().filter_map(|p| p.tensao).collect();
    let tensao = tensoes.iter().copied().reduce(f64::max);
    if tensoes.iter().any(|t| Some(*t) != tensao) {
        avisos.push("pontos com tensões diferentes no mesmo circuito".into());
    }
    if natureza == Natureza::Misto {
        avisos.push("iluminação e tomadas no mesmo circuito: a NBR 5410 (9.5.3.2) pede circuitos separados, salvo as exceções da norma".into());
    }
    if natureza == Natureza::Tue && cargas.len() > 1 {
        avisos.push("TUE com mais de um ponto: equipamento acima de 10 A deve ter circuito próprio (NBR 5410, 9.5.3.3)".into());
    }
    let sem_potencia = cargas.iter().filter(|p| p.potencia.is_none()).count();
    if sem_potencia > 0 {
        avisos.push(format!("{sem_potencia} ponto(s) de carga sem POTENCIA, contados como 0 VA"));
    }
    Circuito {
        id: id.to_string(),
        pontos: membros.iter().map(|p| p.handle).collect(),
        natureza,
        potencia: cargas.iter().filter_map(|p| p.potencia).sum(),
        tensao,
        fases: membros.iter().filter_map(|p| p.fases).max().unwrap_or(1),
        cargas: cargas.len(),
        interruptores: membros.iter().filter(|p| p.tipo == TipoPonto::Interruptor).count(),
        avisos,
    }
}

/// Números pelo valor, depois o resto em ordem alfabética.
fn ordem_natural(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        _ => a.to_uppercase().cmp(&b.to_uppercase()),
    }
}
