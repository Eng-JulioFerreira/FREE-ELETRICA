//! Configurações gerais do projeto (a aba de setup do CRAFT ELE). Delas saem os parâmetros do
//! dimensionamento e do roteamento: nada disso é digitado como fator solto.

use serde::{Deserialize, Serialize};

use crate::nbr5410::Config;

/// Sistema de distribuição da instalação.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sistema {
    /// F+N, 127 V.
    Monofasico127,
    /// 2F+N, 220/127 V.
    Bifasico220127,
    /// 3F+N, 220/127 V.
    Trifasico220127,
    /// 3F+N, 380/220 V: o padrão regional do projeto.
    #[default]
    Trifasico380220,
}

impl Sistema {
    pub const TODOS: [Sistema; 4] = [Self::Monofasico127, Self::Bifasico220127, Self::Trifasico220127, Self::Trifasico380220];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Monofasico127 => "Monofásico 127 V (F+N)",
            Self::Bifasico220127 => "Bifásico 220/127 V (2F+N)",
            Self::Trifasico220127 => "Trifásico 220/127 V (3F+N)",
            Self::Trifasico380220 => "Trifásico 380/220 V (3F+N)",
        }
    }

    /// Tensão fase-neutro (V).
    pub fn tensao_fn(self) -> f64 {
        match self {
            Self::Trifasico380220 => 220.0,
            _ => 127.0,
        }
    }

    /// Tensão fase-fase (V). Monofásico não tem.
    pub fn tensao_ff(self) -> Option<f64> {
        match self {
            Self::Monofasico127 => None,
            Self::Bifasico220127 | Self::Trifasico220127 => Some(220.0),
            Self::Trifasico380220 => Some(380.0),
        }
    }

    pub fn fases(self) -> u8 {
        match self {
            Self::Monofasico127 => 1,
            Self::Bifasico220127 => 2,
            Self::Trifasico220127 | Self::Trifasico380220 => 3,
        }
    }
}

/// Unidade das coordenadas do desenho.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unidade {
    /// A do cabeçalho do desenho (INSUNITS).
    #[default]
    Automatica,
    Metro,
    Centimetro,
    Milimetro,
}

impl Unidade {
    pub const TODAS: [Unidade; 4] = [Self::Automatica, Self::Metro, Self::Centimetro, Self::Milimetro];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Automatica => "Automática (INSUNITS do desenho)",
            Self::Metro => "Metro",
            Self::Centimetro => "Centímetro",
            Self::Milimetro => "Milímetro",
        }
    }

    /// Metros por unidade do desenho, e um aviso quando o desenho não diz a unidade.
    pub fn metros(self, insunits: i64) -> (f64, Option<String>) {
        match self {
            Self::Metro => (1.0, None),
            Self::Centimetro => (0.01, None),
            Self::Milimetro => (0.001, None),
            Self::Automatica => match insunits {
                6 => (1.0, None),
                5 => (0.01, None),
                4 => (0.001, None),
                14 => (0.1, None),
                1 => (0.0254, None),
                2 => (0.3048, None),
                _ => (1.0, Some("o desenho não define a unidade (INSUNITS): considerando metros; ajuste em Configurações > Geral".into())),
            },
        }
    }
}

/// Cores de isolação para as fases. Azul-claro (neutro) e verde/verde-amarelo (proteção) são
/// reservados pela NBR 5410 e não entram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CorCabo {
    Preto,
    Branco,
    Vermelho,
    Marrom,
    Cinza,
    Amarelo,
    Laranja,
    Violeta,
    Rosa,
}

impl CorCabo {
    pub const TODAS: [CorCabo; 9] =
        [Self::Preto, Self::Branco, Self::Vermelho, Self::Marrom, Self::Cinza, Self::Amarelo, Self::Laranja, Self::Violeta, Self::Rosa];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Preto => "Preto",
            Self::Branco => "Branco",
            Self::Vermelho => "Vermelho",
            Self::Marrom => "Marrom",
            Self::Cinza => "Cinza",
            Self::Amarelo => "Amarelo",
            Self::Laranja => "Laranja",
            Self::Violeta => "Violeta",
            Self::Rosa => "Rosa",
        }
    }

    /// Cor para mostrar na tela.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Self::Preto => [20, 20, 20],
            Self::Branco => [235, 235, 235],
            Self::Vermelho => [220, 60, 60],
            Self::Marrom => [150, 95, 55],
            Self::Cinza => [160, 160, 160],
            Self::Amarelo => [230, 200, 40],
            Self::Laranja => [235, 140, 40],
            Self::Violeta => [150, 90, 200],
            Self::Rosa => [235, 130, 180],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoriaTelecom {
    Utp,
    Fibra,
    Cci,
    Ci,
    Coaxial,
    Outro,
}

impl CategoriaTelecom {
    pub const TODAS: [CategoriaTelecom; 6] = [Self::Utp, Self::Fibra, Self::Cci, Self::Ci, Self::Coaxial, Self::Outro];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Utp => "UTP",
            Self::Fibra => "Fibra óptica",
            Self::Cci => "CCI",
            Self::Ci => "CI",
            Self::Coaxial => "Coaxial",
            Self::Outro => "Outro",
        }
    }

    /// O que a quantidade conta.
    pub fn unidade(self) -> &'static str {
        match self {
            Self::Utp | Self::Cci | Self::Ci => "pares",
            Self::Fibra => "fibras",
            Self::Coaxial | Self::Outro => "condutores",
        }
    }
}

/// Um cabo do catálogo de telecomunicações.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CaboTelecom {
    pub nome: String,
    pub categoria: CategoriaTelecom,
    /// Pares, fibras ou condutores (ver [`CategoriaTelecom::unidade`]).
    pub quantidade: u32,
    pub observacao: String,
    /// Faz parte do catálogo de fábrica: não pode ser removido.
    pub padrao: bool,
}

impl Default for CaboTelecom {
    fn default() -> Self {
        CaboTelecom { nome: String::new(), categoria: CategoriaTelecom::Outro, quantidade: 1, observacao: String::new(), padrao: false }
    }
}

/// O catálogo de fábrica.
pub fn cabos_padrao() -> Vec<CaboTelecom> {
    use CategoriaTelecom::*;
    let c = |nome: &str, categoria, quantidade| CaboTelecom { nome: nome.into(), categoria, quantidade, observacao: String::new(), padrao: true };
    vec![
        c("UTP Cat 5", Utp, 4),
        c("UTP Cat 6", Utp, 4),
        c("Fibra óptica N2", Fibra, 2),
        c("Fibra óptica N4", Fibra, 4),
        c("Fibra óptica N12", Fibra, 12),
        c("CCI 1P", Cci, 1),
        c("CCI 2P", Cci, 2),
        c("CCI 3P", Cci, 3),
        c("CCI 4P", Cci, 4),
        c("CI 10P", Ci, 10),
        c("CI 20P", Ci, 20),
        c("Coaxial RG 59", Coaxial, 1),
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Setup {
    pub sistema: Sistema,
    /// Temperatura do projeto: ambiente dos condutores (°C), de 10 a 60.
    pub temperatura_ambiente: f64,
    /// Pé-direito padrão do pavimento (m): os eletrodutos correm no teto.
    pub pe_direito: f64,
    /// Queda de tensão admissível nos circuitos terminais (%).
    pub queda_admissivel: f64,
    /// Multiplica os comprimentos de cabo no levantamento de materiais (perdas, emendas, sobras).
    pub fator_materiais: f64,
    /// Seções (mm²) que não são escritas nas indicações de fiação e no resumo.
    pub secoes_omitidas: Vec<f64>,
    /// Cores das fases A, B e C.
    pub cores_fase: [CorCabo; 3],
    pub unidade: Unidade,
    /// Camadas dos eletrodutos, separadas por vírgula, com curingas `*` e `?`.
    pub camadas_eletroduto: String,
    /// Entre as camadas de eletroduto, as que correm no piso (as outras correm no teto).
    pub camadas_piso: String,
    /// Distância máxima (m) entre um ponto e o eletroduto que chega nele.
    pub raio_captura: f64,
    pub cabos_telecom: Vec<CaboTelecom>,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            sistema: Sistema::default(),
            temperatura_ambiente: 30.0,
            pe_direito: 2.8,
            queda_admissivel: 4.0,
            fator_materiais: 1.10,
            secoes_omitidas: Vec::new(),
            cores_fase: [CorCabo::Preto, CorCabo::Vermelho, CorCabo::Branco],
            unidade: Unidade::default(),
            camadas_eletroduto: "*ELETRODUTO*".into(),
            camadas_piso: "*PISO*".into(),
            raio_captura: 0.30,
            cabos_telecom: cabos_padrao(),
        }
    }
}

/// Tabela 40, isolação PVC, temperatura ambiente (não no solo): (°C, fator).
const FCT_PVC: [(f64, f64); 11] = [
    (10.0, 1.22),
    (15.0, 1.17),
    (20.0, 1.12),
    (25.0, 1.06),
    (30.0, 1.00),
    (35.0, 0.94),
    (40.0, 0.87),
    (45.0, 0.79),
    (50.0, 0.71),
    (55.0, 0.61),
    (60.0, 0.50),
];

impl Setup {
    /// Fator de correção de temperatura (tabela 40, PVC). Entre duas linhas da tabela vale a
    /// temperatura de cima, que é o lado seguro.
    pub fn fct(&self) -> f64 {
        let t = self.temperatura_ambiente;
        FCT_PVC.iter().find(|(limite, _)| t <= *limite + 1e-9).map_or(FCT_PVC[FCT_PVC.len() - 1].1, |(_, f)| *f)
    }

    /// Os parâmetros do dimensionamento. O FCA de cada circuito vem do roteamento.
    pub fn config(&self) -> Config {
        Config {
            tensao_fn: self.sistema.tensao_fn(),
            tensao_ff: self.sistema.tensao_ff().unwrap_or(self.sistema.tensao_fn()),
            fct: self.fct(),
            fca: 1.0,
            fases_padrao: self.sistema.fases(),
            queda_admissivel: self.queda_admissivel,
        }
    }

    /// A seção não é escrita nas indicações.
    pub fn omitida(&self, secao: f64) -> bool {
        self.secoes_omitidas.iter().any(|s| (s - secao).abs() < 1e-6)
    }

    /// Valores dentro dos limites, seções em ordem e o catálogo de fábrica completo (um
    /// arquivo de preferências antigo pode não ter cabos novos).
    pub fn normalizar(&mut self) {
        self.temperatura_ambiente = self.temperatura_ambiente.clamp(10.0, 60.0);
        self.pe_direito = self.pe_direito.clamp(2.0, 10.0);
        self.queda_admissivel = self.queda_admissivel.clamp(0.5, 10.0);
        self.fator_materiais = self.fator_materiais.clamp(1.0, 2.0);
        self.raio_captura = self.raio_captura.clamp(0.01, 5.0);
        self.secoes_omitidas.sort_by(f64::total_cmp);
        self.secoes_omitidas.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        for c in cabos_padrao().into_iter().rev() {
            if !self.cabos_telecom.iter().any(|x| x.nome.eq_ignore_ascii_case(&c.nome)) {
                self.cabos_telecom.insert(0, c);
            }
        }
    }

    /// Cadastra um cabo de telecom. O nome é obrigatório e único.
    pub fn cadastrar_cabo(&mut self, mut cabo: CaboTelecom) -> Result<(), String> {
        cabo.nome = cabo.nome.trim().to_string();
        if cabo.nome.is_empty() {
            return Err("informe o nome do cabo".into());
        }
        if self.cabos_telecom.iter().any(|c| c.nome.eq_ignore_ascii_case(&cabo.nome)) {
            return Err(format!("já existe um cabo \"{}\" no catálogo", cabo.nome));
        }
        cabo.padrao = false;
        cabo.quantidade = cabo.quantidade.max(1);
        self.cabos_telecom.push(cabo);
        Ok(())
    }

    /// Remove um cabo cadastrado pelo usuário. Os de fábrica ficam.
    pub fn remover_cabo(&mut self, nome: &str) -> bool {
        let antes = self.cabos_telecom.len();
        self.cabos_telecom.retain(|c| c.padrao || !c.nome.eq_ignore_ascii_case(nome));
        self.cabos_telecom.len() != antes
    }

    /// A camada é de eletroduto.
    pub fn e_camada_eletroduto(&self, camada: &str) -> bool {
        casa(&self.camadas_eletroduto, camada)
    }

    /// A camada de eletroduto corre no piso.
    pub fn e_camada_piso(&self, camada: &str) -> bool {
        casa(&self.camadas_piso, camada)
    }
}

/// A camada casa com algum dos padrões separados por vírgula.
fn casa(padroes: &str, camada: &str) -> bool {
    padroes.split([',', ';']).map(str::trim).filter(|p| !p.is_empty()).any(|p| curinga(&p.to_uppercase(), &camada.to_uppercase()))
}

/// `*` vale qualquer sequência, `?` um caractere.
fn curinga(padrao: &str, texto: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (padrao.chars().collect(), texto.chars().collect());
    let (mut i, mut j, mut estrela, mut volta) = (0, 0, None, 0);
    while j < t.len() {
        if i < p.len() && (p[i] == '?' || p[i] == t[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            estrela = Some(i);
            volta = j;
            i += 1;
        } else if let Some(e) = estrela {
            i = e + 1;
            volta += 1;
            j = volta;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|c| *c == '*')
}
