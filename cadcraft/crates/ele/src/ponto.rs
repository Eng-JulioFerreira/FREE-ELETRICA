//! Pontos elétricos: blocos inseridos no espaço do modelo que carregam atributos elétricos
//! (padrão em `docs/padrao_blocos_eletricos.md` do CRAFT ELE).

use cadcraft_doc::{Drawing, EntityKind, Handle, Insert};
use serde::Serialize;

/// Atributos que marcam um bloco como ponto elétrico. Um bloco sem nenhum deles (porta, móvel,
/// carimbo) não entra na análise, mesmo que o nome lembre um ponto elétrico.
const TAGS_TIPO: &[&str] = &["TIPO_PONTO", "TIPO", "TYPE", "CATEGORIA"];
const TAGS_POTENCIA: &[&str] = &["POTENCIA", "POT", "POWER"];
const TAGS_TENSAO: &[&str] = &["TENSAO", "VOLTS", "VOLTAGEM"];
const TAGS_FASES: &[&str] = &["NUMERO_FASES", "FASES", "NUM_FASES", "FASE"];
const TAGS_CIRCUITO: &[&str] = &["CIRCUITO", "CIRC", "CIRCUIT"];
const TAGS_COMANDO: &[&str] = &["COMANDO", "CMD"];
const TAGS_ID: &[&str] = &["TAG", "ID", "NOME"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoPonto {
    Quadro,
    Iluminacao,
    /// Tomada de uso geral (TUG).
    Tomada,
    /// Tomada de uso específico (TUE).
    TomadaEspecifica,
    Interruptor,
    /// Caixa de passagem: não é carga.
    Caixa,
    /// Ponto com atributos elétricos mas tipo não reconhecido.
    Outro,
}

impl TipoPonto {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Quadro => "Quadro",
            Self::Iluminacao => "Iluminação",
            Self::Tomada => "TUG",
            Self::TomadaEspecifica => "TUE",
            Self::Interruptor => "Interruptor",
            Self::Caixa => "Caixa de passagem",
            Self::Outro => "Outro",
        }
    }

    /// Consome potência do circuito.
    pub fn e_carga(self) -> bool {
        matches!(self, Self::Iluminacao | Self::Tomada | Self::TomadaEspecifica | Self::Outro)
    }

    /// O tipo descrito por um texto: o valor de TIPO_PONTO ou o nome do bloco.
    pub fn classificar(texto: &str) -> Option<Self> {
        let n = normalizar(texto);
        let palavras: Vec<&str> = n.split(' ').filter(|p| !p.is_empty()).collect();
        let palavra = |p: &str| palavras.contains(&p);
        let comeca = |p: &str| palavras.iter().any(|w| w.starts_with(p));
        let contem = |p: &str| n.contains(p);

        if contem("QUADRO") || palavra("QGBT") || comeca("QD") {
            return Some(Self::Quadro);
        }
        if contem("INTERRUPTOR") || comeca("INT") || contem("PULSADOR") || contem("DIMMER") {
            return Some(Self::Interruptor);
        }
        if comeca("TUE") || contem("ESPECIFIC") || contem("CHUVEIRO") {
            return Some(Self::TomadaEspecifica);
        }
        if contem("TOMADA") || comeca("TUG") || palavra("TOM") {
            return Some(Self::Tomada);
        }
        if contem("ILUM") || palavra("LUZ") || contem("LUMINARIA") || contem("LAMPADA") || contem("ARANDELA") || contem("PLAFON") || palavra("SPOT") {
            return Some(Self::Iluminacao);
        }
        if contem("PASSAGEM") || palavra("CP") || palavra("CAIXA") {
            return Some(Self::Caixa);
        }
        None
    }
}

/// Maiúsculas, sem acentos, e tudo o que não é letra ou dígito vira espaço.
fn normalizar(s: &str) -> String {
    s.chars()
        .map(|c| match c.to_uppercase().next().unwrap_or(c) {
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'Ç' => 'C',
            c if c.is_ascii_alphanumeric() => c,
            _ => ' ',
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PontoEletrico {
    #[serde(serialize_with = "handle_hex")]
    pub handle: Handle,
    pub bloco: String,
    pub camada: String,
    pub posicao: [f64; 2],
    pub tipo: TipoPonto,
    /// O tipo veio do atributo TIPO_PONTO (e não do nome do bloco).
    pub tipo_informado: bool,
    /// Potência em VA (W é tratado como VA por enquanto).
    pub potencia: Option<f64>,
    pub tensao: Option<f64>,
    pub fases: Option<u8>,
    pub circuito: Option<String>,
    pub comando: Option<String>,
    pub tag: Option<String>,
    /// Todos os atributos lidos, na ordem do bloco.
    pub atributos: Vec<(String, String)>,
}

fn handle_hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

impl PontoEletrico {
    /// O primeiro atributo não vazio entre `tags` (sem diferenciar maiúsculas).
    fn atributo<'a>(atributos: &'a [(String, String)], tags: &[&str]) -> Option<&'a str> {
        tags.iter().find_map(|t| atributos.iter().find(|(tag, v)| tag.eq_ignore_ascii_case(t) && !v.trim().is_empty()).map(|(_, v)| v.trim()))
    }

    /// Lê um bloco inserido. `None` quando ele não tem nenhum atributo elétrico.
    pub fn de_insert(doc: &Drawing, handle: Handle, camada: &str, ins: &Insert) -> Option<Self> {
        let mut atributos: Vec<(String, String)> = ins.attribs.iter().map(|a| (a.tag.trim().to_string(), a.text.value.clone())).collect();
        // Atributos constantes ficam só na definição do bloco.
        if let Some(def) = doc.blocks.get(&ins.block) {
            for e in def.entities.iter() {
                if let EntityKind::AttDef(a) = &e.kind
                    && a.constant
                    && !atributos.iter().any(|(t, _)| t.eq_ignore_ascii_case(a.tag.trim()))
                {
                    atributos.push((a.tag.trim().to_string(), a.text.value.clone()));
                }
            }
        }
        let todas = [TAGS_TIPO, TAGS_POTENCIA, TAGS_TENSAO, TAGS_FASES, TAGS_CIRCUITO, TAGS_COMANDO];
        if !todas.iter().any(|tags| Self::atributo(&atributos, tags).is_some()) {
            return None;
        }
        let tipo_attr = Self::atributo(&atributos, TAGS_TIPO).and_then(TipoPonto::classificar);
        let tipo = tipo_attr.or_else(|| TipoPonto::classificar(&ins.block)).unwrap_or(TipoPonto::Outro);
        let texto = |tags| Self::atributo(&atributos, tags).map(str::to_string);
        Some(PontoEletrico {
            handle,
            bloco: ins.block.clone(),
            camada: camada.to_string(),
            posicao: [ins.insert.x, ins.insert.y],
            tipo,
            tipo_informado: tipo_attr.is_some(),
            potencia: Self::atributo(&atributos, TAGS_POTENCIA).and_then(potencia_va),
            tensao: Self::atributo(&atributos, TAGS_TENSAO).and_then(numero),
            fases: Self::atributo(&atributos, TAGS_FASES).and_then(fases),
            circuito: texto(TAGS_CIRCUITO),
            comando: texto(TAGS_COMANDO),
            tag: texto(TAGS_ID),
            atributos,
        })
    }

    /// Problemas de preenchimento segundo o padrão de blocos.
    pub fn avisos(&self) -> Vec<String> {
        let mut v = Vec::new();
        match self.tipo {
            TipoPonto::Interruptor => {
                if self.comando.is_none() {
                    v.push("interruptor sem COMANDO (letra da tecla)".into());
                }
                if self.potencia.is_some_and(|p| p > 0.0) {
                    v.push("interruptor deve ter POTENCIA 0W".into());
                }
                if self.circuito.is_none() {
                    v.push("interruptor sem CIRCUITO".into());
                }
            }
            TipoPonto::Quadro => {
                if self.tensao.is_none() {
                    v.push("quadro sem TENSAO".into());
                }
            }
            TipoPonto::Caixa => {}
            TipoPonto::Iluminacao | TipoPonto::Tomada | TipoPonto::TomadaEspecifica | TipoPonto::Outro => {
                if self.tipo == TipoPonto::Outro {
                    v.push("tipo não reconhecido: preencha TIPO_PONTO (TUG, TUE, ILUM, INTERRUPTOR, QUADRO)".into());
                }
                if self.potencia.is_none() {
                    v.push("ponto de carga sem POTENCIA".into());
                }
                if self.tensao.is_none() {
                    v.push("ponto sem TENSAO".into());
                }
                if self.fases.is_none() {
                    v.push("ponto sem NUMERO_FASES".into());
                }
                if self.circuito.is_none() {
                    v.push("ponto sem CIRCUITO".into());
                }
            }
        }
        v
    }

    /// Nome curto para listas: a TAG, ou o bloco e o handle.
    pub fn rotulo(&self) -> String {
        self.tag.clone().unwrap_or_else(|| format!("{} #{}", self.bloco, self.handle.hex()))
    }
}

/// Todos os pontos elétricos do espaço do modelo.
pub fn ler_pontos(doc: &Drawing) -> Vec<PontoEletrico> {
    doc.model
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Insert(ins) => PontoEletrico::de_insert(doc, e.handle, &e.common.layer, ins),
            _ => None,
        })
        .collect()
}

/// O primeiro número do texto. Aceita vírgula decimal ("1,5") e ponto de milhar ("1.500").
pub fn numero(texto: &str) -> Option<f64> {
    let inicio = texto.find(|c: char| c.is_ascii_digit())?;
    let resto = &texto[inicio..];
    let fim = resto.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).unwrap_or(resto.len());
    let bruto = resto[..fim].trim_end_matches(['.', ',']);
    let limpo = if bruto.contains(',') {
        bruto.replace('.', "").replace(',', ".")
    } else if bruto.split('.').skip(1).all(|g| g.len() == 3) && bruto.contains('.') {
        // "1.500": ponto de milhar.
        bruto.replace('.', "")
    } else {
        bruto.to_string()
    };
    limpo.parse().ok()
}

/// Potência em VA: "600VA", "100 W", "1,5kW", "5400W".
pub fn potencia_va(texto: &str) -> Option<f64> {
    let n = numero(texto)?;
    let depois = texto.trim_start_matches(|c: char| !c.is_ascii_digit()).trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ',');
    let kilo = depois.trim_start().starts_with(['k', 'K']);
    Some(if kilo { n * 1000.0 } else { n })
}

/// Número de fases: "1".."3", "MONO", "BI", "TRI".
pub fn fases(texto: &str) -> Option<u8> {
    let n = normalizar(texto);
    if n.contains("TRI") {
        return Some(3);
    }
    if n.contains("BI") {
        return Some(2);
    }
    if n.contains("MONO") {
        return Some(1);
    }
    numero(texto).map(|f| f as u8).filter(|f| (1..=3).contains(f))
}
