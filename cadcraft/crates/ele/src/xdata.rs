//! Dados do CRAFT ELE gravados no XDATA das entidades (aplicativo `CRAFTELE`), como pares
//! `CHAVE=valor` em grupos 1000. Viajam dentro do DWG/DXF e são copiados junto com o objeto.

use cadcraft_doc::RawTag;

pub const APP: &str = "CRAFTELE";

/// O trecho `[início, fim)` do XDATA do CRAFT ELE: do grupo 1001 até o próximo 1001.
fn trecho(xdata: &[RawTag]) -> Option<(usize, usize)> {
    let inicio = xdata.iter().position(|t| t.code == 1001 && t.value.trim().eq_ignore_ascii_case(APP))?;
    let fim = xdata.iter().skip(inicio + 1).position(|t| t.code == 1001).map_or(xdata.len(), |i| inicio + 1 + i);
    Some((inicio, fim))
}

/// Os pares gravados, na ordem.
pub fn ler(xdata: &[RawTag]) -> Vec<(String, String)> {
    let Some((inicio, fim)) = trecho(xdata) else { return Vec::new() };
    xdata[inicio + 1..fim]
        .iter()
        .filter(|t| t.code == 1000)
        .filter_map(|t| t.value.split_once('=').map(|(k, v)| (k.trim().to_string(), v.to_string())))
        .collect()
}

/// Grava (`Some`) ou apaga (`None`) uma chave. Sem nenhuma chave, o trecho do CRAFT ELE some.
pub fn gravar(xdata: &mut Vec<RawTag>, chave: &str, valor: Option<&str>) {
    let mut pares = ler(xdata);
    match (pares.iter().position(|(k, _)| k.eq_ignore_ascii_case(chave)), valor) {
        (Some(i), Some(v)) => pares[i].1 = v.to_string(),
        (Some(i), None) => {
            pares.remove(i);
        }
        (None, Some(v)) => pares.push((chave.to_string(), v.to_string())),
        (None, None) => return,
    }
    let novo: Vec<RawTag> = if pares.is_empty() {
        Vec::new()
    } else {
        std::iter::once(RawTag { code: 1001, value: APP.into() })
            .chain(pares.into_iter().map(|(k, v)| RawTag { code: 1000, value: format!("{k}={v}") }))
            .collect()
    };
    match trecho(xdata) {
        Some((inicio, fim)) => {
            xdata.splice(inicio..fim, novo);
        }
        None => xdata.extend(novo),
    }
}
