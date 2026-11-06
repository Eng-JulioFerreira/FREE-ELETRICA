// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Configurações gerais do projeto (a aba de setup do Free Elétrica). Delas saem os parâmetros do
//! dimensionamento e do roteamento: nada disso é digitado como fator solto.

use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::nbr5410::{Config, Isolacao, TipoCabo};

/// As configurações em uso no processo. Ficam no motor, e não na interface: os comandos
/// (linha de comando, scripts, canal de controle, MCP) e a paleta leem a mesma fonte. A
/// interface só carrega e grava nas preferências do CADCraft.
static ATUAL: LazyLock<RwLock<Setup>> = LazyLock::new(|| RwLock::new(Setup::default()));

/// As configurações em uso.
pub fn atual() -> Setup {
    ATUAL.read().map_or_else(|e| e.into_inner().clone(), |s| s.clone())
}

/// Troca as configurações em uso (normalizadas).
pub fn definir(mut s: Setup) {
    s.normalizar();
    match ATUAL.write() {
        Ok(mut g) => *g = s,
        Err(e) => *e.into_inner() = s,
    }
}

/// Lê (`params` vazio) ou grava campos das configurações, como JSON: `{sistema,
/// temperaturaAmbiente, peDireito...}`. Devolve as configurações em uso.
pub fn json(params: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut v = serde_json::to_value(atual()).map_err(|e| e.to_string())?;
    if let (Some(campos), Some(novos)) = (v.as_object_mut(), params.as_object())
        && !novos.is_empty()
    {
        for (k, valor) in novos {
            campos.insert(k.clone(), valor.clone());
        }
        definir(serde_json::from_value(v).map_err(|e| e.to_string())?);
        v = serde_json::to_value(atual()).map_err(|e| e.to_string())?;
    }
    Ok(v)
}

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
    /// Diâmetro externo (mm), para a ocupação dos eletrodutos.
    pub diametro: f64,
    pub observacao: String,
    /// Faz parte do catálogo de fábrica: não pode ser removido.
    pub padrao: bool,
}

impl Default for CaboTelecom {
    fn default() -> Self {
        CaboTelecom {
            nome: String::new(),
            categoria: CategoriaTelecom::Outro,
            quantidade: 1,
            diametro: 0.0,
            observacao: String::new(),
            padrao: false,
        }
    }
}

/// O catálogo de fábrica. Diâmetros externos típicos de catálogo: confira com o fabricante do
/// cabo usado na obra.
pub fn cabos_padrao() -> Vec<CaboTelecom> {
    use CategoriaTelecom::*;
    let c = |nome: &str, categoria, quantidade, diametro| CaboTelecom {
        nome: nome.into(),
        categoria,
        quantidade,
        diametro,
        observacao: String::new(),
        padrao: true,
    };
    vec![
        c("UTP Cat 5", Utp, 4, 5.0),
        c("UTP Cat 6", Utp, 4, 6.0),
        c("Fibra óptica N2", Fibra, 2, 3.0),
        c("Fibra óptica N4", Fibra, 4, 5.0),
        c("Fibra óptica N12", Fibra, 12, 6.5),
        c("CCI 1P", Cci, 1, 3.0),
        c("CCI 2P", Cci, 2, 3.5),
        c("CCI 3P", Cci, 3, 4.0),
        c("CCI 4P", Cci, 4, 4.5),
        c("CI 10P", Ci, 10, 7.5),
        c("CI 20P", Ci, 20, 9.5),
        c("Coaxial RG 59", Coaxial, 1, 6.1),
    ]
}

/// Diâmetro externo de um condutor isolado (mm), por seção e isolação.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiametroCondutor {
    /// Seção (mm²).
    pub secao: f64,
    /// Cabo de PVC 450/750 V.
    pub pvc: f64,
    /// Cabo de EPR ou XLPE 0,6/1 kV.
    pub epr_xlpe: f64,
}

impl Default for DiametroCondutor {
    fn default() -> Self {
        DiametroCondutor { secao: 0.0, pvc: 0.0, epr_xlpe: 0.0 }
    }
}

/// Diâmetros externos típicos de catálogo (condutor unipolar de cobre): confira com o fabricante.
pub fn diametros_padrao() -> Vec<DiametroCondutor> {
    [
        (1.5, 3.0, 5.6),
        (2.5, 3.6, 6.0),
        (4.0, 4.2, 6.6),
        (6.0, 4.8, 7.2),
        (10.0, 6.1, 8.2),
        (16.0, 7.2, 9.3),
        (25.0, 8.9, 11.0),
        (35.0, 10.2, 12.2),
        (50.0, 12.0, 14.0),
        (70.0, 13.8, 15.9),
        (95.0, 15.9, 18.1),
        (120.0, 17.6, 19.9),
    ]
    .into_iter()
    .map(|(secao, pvc, epr_xlpe)| DiametroCondutor { secao, pvc, epr_xlpe })
    .collect()
}

/// Um tamanho de eletroduto do tipo usado no projeto.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TamanhoEletroduto {
    /// Diâmetro nominal (mm), o que vai escrito no desenho.
    pub nominal: f64,
    /// Referência em polegadas ("1/2\"", "3/4\""...).
    pub polegadas: String,
    /// Diâmetro interno (mm), para a ocupação.
    pub interno: f64,
}

impl Default for TamanhoEletroduto {
    fn default() -> Self {
        TamanhoEletroduto { nominal: 0.0, polegadas: String::new(), interno: 0.0 }
    }
}

/// Eletroduto de PVC rígido roscável, linha média: diâmetros internos típicos de catálogo.
/// Confira com o fabricante.
pub fn eletrodutos_padrao() -> Vec<TamanhoEletroduto> {
    [
        (16.0, "3/8\"", 12.6),
        (20.0, "1/2\"", 16.4),
        (25.0, "3/4\"", 21.3),
        (32.0, "1\"", 27.5),
        (40.0, "1 1/4\"", 36.1),
        (50.0, "1 1/2\"", 41.4),
        (60.0, "2\"", 52.8),
        (75.0, "2 1/2\"", 67.1),
        (85.0, "3\"", 79.6),
    ]
    .into_iter()
    .map(|(nominal, pol, interno)| TamanhoEletroduto { nominal, polegadas: pol.into(), interno })
    .collect()
}

/// Um cabo elétrico do catálogo: o nome que o projetista usa e a classe que vale no cálculo.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CaboEletrico {
    pub nome: String,
    /// Isolação e tensão: entram no dimensionamento (NBR 5410) e no diâmetro externo.
    pub classe: TipoCabo,
    pub observacao: String,
}

/// O catálogo de fábrica: um cabo de cada classe.
pub fn cabos_eletricos_padrao() -> Vec<CaboEletrico> {
    TipoCabo::TODOS.into_iter().map(|classe| CaboEletrico { nome: classe.nome().to_string(), classe, observacao: String::new() }).collect()
}

/// O tipo de linha de um perfil de eletroduto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoLinha {
    #[default]
    Continua,
    Tracejada,
    TracoPonto,
    Pontilhada,
}

impl TipoLinha {
    pub const TODOS: [TipoLinha; 4] = [Self::Continua, Self::Tracejada, Self::TracoPonto, Self::Pontilhada];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Continua => "Contínua (Continuous)",
            Self::Tracejada => "Tracejada (Dashed)",
            Self::TracoPonto => "Traço e ponto (DashDot)",
            Self::Pontilhada => "Pontilhada (Dotted)",
        }
    }

    /// O padrão do caminhamento: contínua no teto, tracejada na parede, traço e ponto no piso.
    pub fn do_caminhamento(i: crate::rota::Instalacao) -> Self {
        use crate::rota::Instalacao as I;
        match i {
            I::Teto => Self::Continua,
            I::Parede => Self::Tracejada,
            I::Piso => Self::TracoPonto,
        }
    }

    /// Os tipos de linha de um caminhamento: é o tipo de linha que diz por onde o eletroduto corre.
    pub fn do_caminhamento_opcoes(i: crate::rota::Instalacao) -> &'static [TipoLinha] {
        use crate::rota::Instalacao as I;
        match i {
            I::Teto => &[Self::Continua],
            I::Parede => &[Self::Tracejada, Self::Pontilhada],
            I::Piso => &[Self::TracoPonto],
        }
    }

    /// O nome do tipo de linha no desenho.
    pub fn no_desenho(self) -> &'static str {
        match self {
            Self::Continua => "Continuous",
            Self::Tracejada => "ELE-TRACEJADA",
            Self::TracoPonto => "ELE-TRACO-PONTO",
            Self::Pontilhada => "ELE-PONTILHADA",
        }
    }
}

/// As cores oferecidas para os perfis: (índice de cor do CAD, nome, RGB para a tela).
pub const CORES_DE_LINHA: [(u8, &str, [u8; 3]); 8] = [
    (1, "Vermelho", [255, 0, 0]),
    (2, "Amarelo", [255, 255, 0]),
    (3, "Verde", [0, 255, 0]),
    (4, "Ciano", [0, 255, 255]),
    (5, "Azul", [0, 0, 255]),
    (6, "Magenta", [255, 0, 255]),
    (7, "Branco", [255, 255, 255]),
    (8, "Cinza", [128, 128, 128]),
];

/// Um perfil de eletroduto: o que a mini-barra escolhe e o que cada trecho desenhado aplica.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PerfilEletroduto {
    pub nome: String,
    /// O material: entra na lista de materiais.
    pub tipo: crate::rota::TipoEletroduto,
    pub tipo_linha: TipoLinha,
    /// Índice de cor do CAD.
    pub cor: u8,
    /// Onde o trecho corre em 3D.
    pub caminhamento: crate::rota::Instalacao,
    /// A tabela de diâmetros comerciais: nominal e interno real (mm).
    pub diametros: Vec<TamanhoEletroduto>,
}

impl PerfilEletroduto {
    /// O tipo de linha que vale: o do perfil, se o caminhamento aceita; senão, o padrão do caminhamento.
    pub fn linha_efetiva(&self) -> TipoLinha {
        if TipoLinha::do_caminhamento_opcoes(self.caminhamento).contains(&self.tipo_linha) {
            self.tipo_linha
        } else {
            TipoLinha::do_caminhamento(self.caminhamento)
        }
    }
}

/// Os perfis de fábrica. Os diâmetros internos são típicos de catálogo: confira com o fabricante.
pub fn perfis_padrao() -> Vec<PerfilEletroduto> {
    use crate::rota::{Instalacao as I, TipoEletroduto as E};
    let s = Setup { perfis_eletroduto: Vec::new(), ..Setup::default_sem_perfis() };
    [
        ("Eletroduto PVC Flexível - Teto", E::CorrugadoReforcado, I::Teto),
        ("Eletroduto PVC Flexível - Parede", E::CorrugadoLeve, I::Parede),
        ("Eletroduto PEAD - Piso", E::PeadCorrugado, I::Piso),
        ("Eletroduto Aço Galvanizado", E::AcoEletrolitico, I::Teto),
    ]
    .into_iter()
    .map(|(nome, tipo, caminhamento)| PerfilEletroduto {
        nome: nome.into(),
        tipo,
        tipo_linha: TipoLinha::do_caminhamento(caminhamento),
        cor: tipo.cor_padrao(),
        caminhamento,
        diametros: s.tamanhos_de(tipo),
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Setup {
    pub sistema: Sistema,
    /// Temperatura do projeto: ambiente dos condutores (°C), de 10 a 60.
    pub temperatura_ambiente: f64,
    /// Pé-direito padrão do pavimento (m): os eletrodutos correm no teto.
    pub pe_direito: f64,
    /// Altura baixa padrão do projeto (m).
    pub altura_baixa: f64,
    /// Altura média padrão do projeto (m).
    pub altura_media: f64,
    /// Altura alta padrão do projeto (m).
    pub altura_alta: f64,
    /// Queda de tensão admissível nos circuitos terminais de força (%).
    pub queda_admissivel: f64,
    /// Queda de tensão admissível nos circuitos de iluminação (%).
    pub queda_iluminacao: f64,
    /// Queda de tensão admissível no alimentador (%).
    pub queda_alimentador: f64,
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
    /// Os cabos de telecom de fábrica que o projetista excluiu: não voltam sozinhos.
    pub cabos_telecom_excluidos: Vec<String>,
    /// O catálogo de cabos elétricos.
    pub cabos_eletricos: Vec<CaboEletrico>,
    /// Os perfis de eletroduto.
    pub perfis_eletroduto: Vec<PerfilEletroduto>,
    /// O cabo de um circuito novo; cada circuito pode ter o seu.
    pub cabo_padrao: TipoCabo,
    /// Circuitos no mesmo eletroduto podem dividir um condutor de proteção, da maior seção entre eles.
    pub compartilhar_pe: bool,
    /// FCA fixo para todos os circuitos. `None`: o real de cada trecho (NBR 5410, tabela 42).
    pub fca_manual: Option<f64>,
    /// Como escolher o disjuntor entre Ib e Iz.
    pub criterio_disjuntor: crate::nbr5410::CriterioDisjuntor,
    /// O método de instalação dos quadros que não escolheram o seu (NBR 5410, tabela 33).
    pub metodo_instalacao: crate::nbr5410::MetodoInstalacao,
    /// Diâmetros externos dos condutores, por seção.
    pub diametros_condutor: Vec<DiametroCondutor>,
    /// Nome do tipo de eletroduto do projeto.
    pub tipo_eletroduto: String,
    /// Tamanhos do tipo de eletroduto, do menor ao maior.
    pub eletrodutos: Vec<TamanhoEletroduto>,
    /// Menor diâmetro nominal usado (mm).
    pub eletroduto_minimo: f64,
    /// Tabela de demanda da concessionária (id em `demanda::tabelas`).
    pub concessionaria: String,
    /// Tipo de edificação da tabela de iluminação e tomadas.
    pub edificacao: String,
    /// As tabelas de demanda cadastradas pelo projetista.
    pub tabelas_proprias: Vec<crate::demanda::TabelaDemanda>,
    /// As tabelas de fábrica excluídas: saem da lista, e um preset não volta sozinho.
    pub tabelas_excluidas: Vec<String>,
}

impl Default for Setup {
    fn default() -> Self {
        Setup { perfis_eletroduto: perfis_padrao(), ..Setup::default_sem_perfis() }
    }
}

impl Setup {
    fn default_sem_perfis() -> Self {
        Setup {
            sistema: Sistema::default(),
            temperatura_ambiente: 30.0,
            pe_direito: 2.8,
            altura_baixa: 0.30,
            altura_media: 1.20,
            altura_alta: 2.10,
            queda_admissivel: 4.0,
            queda_iluminacao: 4.0,
            queda_alimentador: 1.0,
            fator_materiais: 1.10,
            secoes_omitidas: Vec::new(),
            cores_fase: [CorCabo::Preto, CorCabo::Vermelho, CorCabo::Branco],
            unidade: Unidade::default(),
            camadas_eletroduto: "*ELETRODUTO*".into(),
            camadas_piso: "*PISO*".into(),
            raio_captura: 0.30,
            cabos_telecom: cabos_padrao(),
            cabos_telecom_excluidos: Vec::new(),
            cabos_eletricos: cabos_eletricos_padrao(),
            perfis_eletroduto: Vec::new(),
            cabo_padrao: TipoCabo::default(),
            compartilhar_pe: false,
            fca_manual: None,
            criterio_disjuntor: crate::nbr5410::CriterioDisjuntor::default(),
            metodo_instalacao: crate::nbr5410::MetodoInstalacao::default(),
            diametros_condutor: diametros_padrao(),
            tipo_eletroduto: "PVC rígido roscável, linha média".into(),
            eletrodutos: eletrodutos_padrao(),
            eletroduto_minimo: 20.0,
            concessionaria: "equatorial-nt001-r07".into(),
            edificacao: "Residências e edifícios de apartamentos".into(),
            tabelas_proprias: Vec::new(),
            tabelas_excluidas: Vec::new(),
        }
    }
}

/// Ocupação máxima da seção interna do eletroduto (NBR 5410, 6.2.11.1.6): 53 % com um condutor
/// ou cabo, 31 % com dois, 40 % com três ou mais.
pub fn ocupacao_maxima(condutores: usize) -> f64 {
    match condutores {
        0 | 1 => 0.53,
        2 => 0.31,
        _ => 0.40,
    }
}

impl Setup {
    /// A altura (m) de uma referência da legenda do projeto. `cota` é a cota fixa da família.
    pub fn altura_de(&self, referencia: crate::familia::RefAltura, cota: f64) -> f64 {
        use crate::familia::RefAltura::*;
        match referencia {
            Baixa => self.altura_baixa,
            Media => self.altura_media,
            Alta => self.altura_alta,
            Teto => self.pe_direito,
            Piso => 0.0,
            Fixa => cota,
        }
    }

    /// Fator de correção de temperatura (tabela 40) da isolação padrão.
    pub fn fct(&self) -> f64 {
        self.cabo_padrao.isolacao().fct(self.temperatura_ambiente)
    }

    /// Os parâmetros do dimensionamento com a isolação padrão. O FCA de cada circuito vem do
    /// roteamento.
    pub fn config(&self) -> Config {
        self.config_com(self.cabo_padrao.isolacao())
    }

    /// Os parâmetros do dimensionamento com outra isolação (a de um quadro).
    pub fn config_com(&self, isolacao: Isolacao) -> Config {
        Config {
            tensao_fn: self.sistema.tensao_fn(),
            tensao_ff: self.sistema.tensao_ff().unwrap_or(self.sistema.tensao_fn()),
            fct: isolacao.fct(self.temperatura_ambiente),
            fca: 1.0,
            fases_padrao: self.sistema.fases(),
            queda_admissivel: self.queda_admissivel,
            isolacao,
            carregados: None,
            metodo: self.metodo_instalacao,
            criterio: self.criterio_disjuntor,
        }
    }

    /// Diâmetro externo (mm) de um condutor da seção e isolação dadas. `None`: seção fora da
    /// tabela das configurações.
    pub fn diametro_condutor(&self, secao: f64, isolacao: Isolacao) -> Option<f64> {
        let d = self.diametros_condutor.iter().find(|d| (d.secao - secao).abs() < 1e-6)?;
        let v = if isolacao == Isolacao::Pvc { d.pvc } else { d.epr_xlpe };
        (v > 0.0).then_some(v)
    }

    /// Diâmetro externo (mm) de um condutor pela seção e pelo tipo de cabo: 450/750 V ou 0,6/1 kV.
    pub fn diametro_do_cabo(&self, secao: f64, cabo: TipoCabo) -> Option<f64> {
        let d = self.diametros_condutor.iter().find(|d| (d.secao - secao).abs() < 1e-6)?;
        let v = if cabo.de_06_1kv() { d.epr_xlpe } else { d.pvc };
        (v > 0.0).then_some(v)
    }

    /// Os tamanhos de um tipo de eletroduto. O PVC rígido usa a tabela editável das configurações.
    pub fn tamanhos_de(&self, tipo: crate::rota::TipoEletroduto) -> Vec<TamanhoEletroduto> {
        match tipo.tamanhos_tipicos() {
            Some(lista) => lista
                .iter()
                .map(|(nominal, pol, interno)| TamanhoEletroduto { nominal: *nominal, polegadas: (*pol).into(), interno: *interno })
                .collect(),
            None => self.eletrodutos.clone(),
        }
    }

    /// Os tamanhos de um trecho: os do perfil dele ou, sem perfil, os do tipo.
    pub fn tamanhos_do_trecho(&self, perfil: Option<&str>, tipo: crate::rota::TipoEletroduto) -> Vec<TamanhoEletroduto> {
        match perfil.and_then(|n| self.perfil(n)).filter(|p| !p.diametros.is_empty()) {
            Some(p) => p.diametros.clone(),
            None => self.tamanhos_de(tipo),
        }
    }

    /// Um perfil de eletroduto pelo nome.
    pub fn perfil(&self, nome: &str) -> Option<&PerfilEletroduto> {
        self.perfis_eletroduto.iter().find(|p| p.nome.eq_ignore_ascii_case(nome.trim()))
    }

    /// O menor eletroduto do tipo (a partir do mínimo) que comporta condutores com estes diâmetros
    /// externos (mm), com a ocupação máxima da NBR 5410. Devolve o tamanho e a ocupação (0-1).
    pub fn eletroduto_do_tipo(&self, diametros: &[f64], tipo: crate::rota::TipoEletroduto) -> Option<(TamanhoEletroduto, f64)> {
        self.eletroduto_do_trecho(diametros, None, tipo)
    }

    /// O menor eletroduto do trecho (a partir do mínimo) que comporta condutores com estes diâmetros
    /// externos (mm), pela tabela do perfil ou do tipo.
    pub fn eletroduto_do_trecho(
        &self,
        diametros: &[f64],
        perfil: Option<&str>,
        tipo: crate::rota::TipoEletroduto,
    ) -> Option<(TamanhoEletroduto, f64)> {
        let area = |d: f64| std::f64::consts::PI * d * d / 4.0;
        let ocupada: f64 = diametros.iter().map(|d| area(*d)).sum();
        let limite = ocupacao_maxima(diametros.len());
        self.tamanhos_do_trecho(perfil, tipo)
            .into_iter()
            .filter(|e| e.nominal + 1e-9 >= self.eletroduto_minimo && e.interno > 0.0)
            .map(|e| {
                let oc = ocupada / area(e.interno);
                (e, oc)
            })
            .find(|(_, oc)| *oc <= limite + 1e-9)
    }

    /// O menor eletroduto (a partir do mínimo) que comporta condutores com estes diâmetros
    /// externos (mm), com a ocupação máxima da NBR 5410. Devolve o tamanho e a ocupação (0-1).
    pub fn eletroduto_para(&self, diametros: &[f64]) -> Option<(TamanhoEletroduto, f64)> {
        let area = |d: f64| std::f64::consts::PI * d * d / 4.0;
        let ocupada: f64 = diametros.iter().map(|d| area(*d)).sum();
        let limite = ocupacao_maxima(diametros.len());
        self.eletrodutos
            .iter()
            .filter(|e| e.nominal + 1e-9 >= self.eletroduto_minimo && e.interno > 0.0)
            .map(|e| (e, ocupada / area(e.interno)))
            .find(|(_, oc)| *oc <= limite + 1e-9)
            .map(|(e, oc)| (e.clone(), oc))
    }

    /// A ocupação (0-1) de condutores com estes diâmetros externos num eletroduto dado.
    pub fn ocupacao_em(&self, diametros: &[f64], e: &TamanhoEletroduto) -> Option<f64> {
        let area = |d: f64| std::f64::consts::PI * d * d / 4.0;
        (e.interno > 0.0).then(|| diametros.iter().map(|d| area(*d)).sum::<f64>() / area(e.interno))
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
        self.altura_baixa = self.altura_baixa.clamp(0.0, 10.0);
        self.altura_media = self.altura_media.clamp(0.0, 10.0);
        self.altura_alta = self.altura_alta.clamp(0.0, 10.0);
        self.queda_admissivel = self.queda_admissivel.clamp(0.5, 10.0);
        self.queda_iluminacao = self.queda_iluminacao.clamp(0.5, 10.0);
        self.queda_alimentador = self.queda_alimentador.clamp(0.1, 10.0);
        self.fca_manual = self.fca_manual.filter(|f| f.is_finite()).map(|f| f.clamp(0.1, 1.0));
        self.fator_materiais = self.fator_materiais.clamp(1.0, 2.0);
        self.raio_captura = self.raio_captura.clamp(0.01, 5.0);
        self.secoes_omitidas.sort_by(f64::total_cmp);
        self.secoes_omitidas.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        for c in cabos_padrao().into_iter().rev() {
            match self.cabos_telecom.iter_mut().find(|x| x.nome.eq_ignore_ascii_case(&c.nome)) {
                // Preferências antigas não tinham diâmetro.
                Some(x) if x.diametro <= 0.0 => x.diametro = c.diametro,
                Some(_) => {}
                None if self.cabos_telecom_excluidos.iter().any(|n| n.eq_ignore_ascii_case(&c.nome)) => {}
                None => self.cabos_telecom.insert(0, c),
            }
        }
        if self.cabos_eletricos.is_empty() {
            self.cabos_eletricos = cabos_eletricos_padrao();
        }
        // Perfis com nome único, cor válida e diâmetros em ordem; nunca sem nenhum.
        let mut vistos: Vec<String> = Vec::new();
        self.perfis_eletroduto.retain(|p| {
            let chave = p.nome.trim().to_lowercase();
            let bom = !chave.is_empty() && !vistos.contains(&chave);
            if bom {
                vistos.push(chave);
            }
            bom
        });
        for p in &mut self.perfis_eletroduto {
            p.nome = p.nome.trim().to_string();
            p.cor = p.cor.max(1);
            p.diametros.retain(|d| d.nominal.is_finite() && d.nominal > 0.0 && d.interno.is_finite() && d.interno > 0.0);
            p.diametros.sort_by(|a, b| a.nominal.total_cmp(&b.nominal));
        }
        if self.perfis_eletroduto.is_empty() {
            self.perfis_eletroduto = perfis_padrao();
        }
        self.padronizar_perfis();
        if self.diametros_condutor.is_empty() {
            self.diametros_condutor = diametros_padrao();
        }
        self.diametros_condutor.sort_by(|a, b| a.secao.total_cmp(&b.secao));
        if self.eletrodutos.is_empty() {
            self.eletrodutos = eletrodutos_padrao();
        }
        self.eletrodutos.sort_by(|a, b| a.nominal.total_cmp(&b.nominal));
        if self.tipo_eletroduto.trim().is_empty() {
            self.tipo_eletroduto = Setup::default().tipo_eletroduto;
        }
        // Concessionária e edificação sempre válidas (as tabelas mudam entre versões).
        // Uma tabela própria inválida (arquivo editado à mão) não entra no cálculo.
        self.tabelas_proprias.retain(|t| {
            !t.id.trim().is_empty() && t.validar().is_ok() && crate::demanda::tabelas().iter().all(|p| !p.id.eq_ignore_ascii_case(&t.id))
        });
        for t in &mut self.tabelas_proprias {
            t.propria = true;
        }
        // Nunca sem nenhuma tabela de demanda.
        if self.tabelas_demanda().is_empty() {
            self.tabelas_excluidas.clear();
        }
        let tabela = self.tabela_demanda();
        let (id, edificacao) = (tabela.id.clone(), tabela.edificacao(&self.edificacao).nome.clone());
        self.concessionaria = id;
        self.edificacao = edificacao;
    }

    /// Eletrodutos do mesmo material têm a mesma cor, a do primeiro perfil dele, e o tipo de linha segue
    /// o caminhamento.
    pub fn padronizar_perfis(&mut self) {
        let mut cores: Vec<(crate::rota::TipoEletroduto, u8)> = Vec::new();
        for p in &mut self.perfis_eletroduto {
            p.tipo_linha = p.linha_efetiva();
            match cores.iter().find(|(t, _)| t.mesma_familia(p.tipo)) {
                Some((_, c)) => p.cor = *c,
                None => cores.push((p.tipo, p.cor.max(1))),
            }
        }
    }

    /// Muda a cor de um material em todos os perfis dele.
    pub fn definir_cor_do_material(&mut self, tipo: crate::rota::TipoEletroduto, cor: u8) {
        for p in self.perfis_eletroduto.iter_mut().filter(|p| p.tipo.mesma_familia(tipo)) {
            p.cor = cor.max(1);
        }
    }

    /// A tabela de demanda escolhida. A que não existe mais dá lugar à primeira da lista.
    pub fn tabela_demanda(&self) -> &crate::demanda::TabelaDemanda {
        let id = self.concessionaria.trim();
        let lista = self.tabelas_demanda();
        lista.iter().find(|t| t.id.eq_ignore_ascii_case(id)).or_else(|| lista.first()).copied().unwrap_or_else(|| crate::demanda::tabela(id))
    }

    /// Todas as tabelas de demanda: as de fábrica que não foram excluídas e as próprias.
    pub fn tabelas_demanda(&self) -> Vec<&crate::demanda::TabelaDemanda> {
        let excluida = |id: &str| self.tabelas_excluidas.iter().any(|e| e.eq_ignore_ascii_case(id));
        crate::demanda::tabelas().iter().filter(|t| !excluida(&t.id)).chain(self.tabelas_proprias.iter()).collect()
    }

    /// Cadastra ou atualiza uma tabela própria. Sem id, ele nasce do nome. As de fábrica não mudam:
    /// copie uma para outro nome. Devolve o id.
    pub fn salvar_tabela(&mut self, mut t: crate::demanda::TabelaDemanda) -> Result<String, String> {
        t.validar()?;
        t.propria = true;
        t.nome = t.nome.trim().to_string();
        if t.id.trim().is_empty() {
            let base: String = crate::ponto::normalizar(&t.nome).to_lowercase().split(' ').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
            t.id = format!("propria-{base}");
        }
        if crate::demanda::tabelas().iter().any(|p| p.id.eq_ignore_ascii_case(&t.id)) {
            return Err("as tabelas de fábrica não podem ser alteradas: salve com outro nome".into());
        }
        if self.tabelas_proprias.iter().any(|o| !o.id.eq_ignore_ascii_case(&t.id) && o.nome.eq_ignore_ascii_case(&t.nome)) {
            return Err(format!("já existe uma tabela \"{}\"", t.nome));
        }
        let id = t.id.clone();
        match self.tabelas_proprias.iter_mut().find(|o| o.id.eq_ignore_ascii_case(&t.id)) {
            Some(o) => *o = t,
            None => self.tabelas_proprias.push(t),
        }
        Ok(id)
    }

    /// Exclui uma tabela, própria ou de fábrica. Se ela estava em uso, o projeto vai para a primeira da
    /// lista. A última que sobrar não sai: sem nenhuma, voltam as de fábrica.
    pub fn remover_tabela(&mut self, id: &str) -> bool {
        let id = id.trim();
        let existe = self.tabelas_demanda().iter().any(|t| t.id.eq_ignore_ascii_case(id));
        if !existe {
            return false;
        }
        let antes = self.tabelas_proprias.len();
        self.tabelas_proprias.retain(|t| !t.id.eq_ignore_ascii_case(id));
        if self.tabelas_proprias.len() == antes {
            self.tabelas_excluidas.push(id.to_string());
        }
        self.normalizar();
        true
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
        if !(cabo.diametro.is_finite() && cabo.diametro > 0.0) {
            return Err("informe o diâmetro externo do cabo (mm): ele entra no cálculo de ocupação dos eletrodutos".into());
        }
        cabo.padrao = false;
        cabo.quantidade = cabo.quantidade.max(1);
        self.cabos_telecom.push(cabo);
        Ok(())
    }

    /// Exclui um cabo de telecom. Um de fábrica fica anotado, para não voltar ao abrir o programa.
    pub fn remover_cabo(&mut self, nome: &str) -> bool {
        let antes = self.cabos_telecom.len();
        let padrao = self.cabos_telecom.iter().any(|c| c.padrao && c.nome.eq_ignore_ascii_case(nome));
        self.cabos_telecom.retain(|c| !c.nome.eq_ignore_ascii_case(nome));
        if padrao {
            self.cabos_telecom_excluidos.push(nome.trim().to_string());
        }
        self.cabos_telecom.len() != antes
    }

    /// Altera um cabo de telecom. Mudar o nome de um de fábrica equivale a excluí-lo e cadastrar outro.
    pub fn atualizar_cabo(&mut self, nome: &str, mut novo: CaboTelecom) -> Result<(), String> {
        let Some(i) = self.cabos_telecom.iter().position(|c| c.nome.eq_ignore_ascii_case(nome)) else {
            return Err(format!("não há cabo \"{nome}\" no catálogo"));
        };
        novo.nome = novo.nome.trim().to_string();
        if novo.nome.is_empty() {
            return Err("informe o nome do cabo".into());
        }
        if self.cabos_telecom.iter().enumerate().any(|(k, c)| k != i && c.nome.eq_ignore_ascii_case(&novo.nome)) {
            return Err(format!("já existe um cabo \"{}\" no catálogo", novo.nome));
        }
        if !(novo.diametro.is_finite() && novo.diametro > 0.0) {
            return Err("informe o diâmetro externo do cabo (mm): ele entra no cálculo de ocupação dos eletrodutos".into());
        }
        let antigo = &self.cabos_telecom[i];
        let renomeado = !antigo.nome.eq_ignore_ascii_case(&novo.nome);
        novo.padrao = antigo.padrao && !renomeado;
        if antigo.padrao && renomeado {
            self.cabos_telecom_excluidos.push(antigo.nome.clone());
        }
        novo.quantidade = novo.quantidade.max(1);
        self.cabos_telecom[i] = novo;
        Ok(())
    }

    /// Cadastra um cabo elétrico. O nome é obrigatório e único.
    pub fn cadastrar_cabo_eletrico(&mut self, mut cabo: CaboEletrico) -> Result<(), String> {
        cabo.nome = cabo.nome.trim().to_string();
        if cabo.nome.is_empty() {
            return Err("informe o nome do cabo".into());
        }
        if self.cabos_eletricos.iter().any(|c| c.nome.eq_ignore_ascii_case(&cabo.nome)) {
            return Err(format!("já existe um cabo \"{}\" no catálogo", cabo.nome));
        }
        self.cabos_eletricos.push(cabo);
        Ok(())
    }

    /// Altera um cabo elétrico. Os circuitos que já usam o nome antigo continuam com ele.
    pub fn atualizar_cabo_eletrico(&mut self, nome: &str, mut novo: CaboEletrico) -> Result<(), String> {
        let Some(i) = self.cabos_eletricos.iter().position(|c| c.nome.eq_ignore_ascii_case(nome)) else {
            return Err(format!("não há cabo \"{nome}\" no catálogo"));
        };
        novo.nome = novo.nome.trim().to_string();
        if novo.nome.is_empty() {
            return Err("informe o nome do cabo".into());
        }
        if self.cabos_eletricos.iter().enumerate().any(|(k, c)| k != i && c.nome.eq_ignore_ascii_case(&novo.nome)) {
            return Err(format!("já existe um cabo \"{}\" no catálogo", novo.nome));
        }
        self.cabos_eletricos[i] = novo;
        Ok(())
    }

    /// Exclui um cabo elétrico do catálogo. O último fica: sem nenhum, não há o que escolher.
    pub fn remover_cabo_eletrico(&mut self, nome: &str) -> bool {
        if self.cabos_eletricos.len() <= 1 {
            return false;
        }
        let antes = self.cabos_eletricos.len();
        self.cabos_eletricos.retain(|c| !c.nome.eq_ignore_ascii_case(nome));
        self.cabos_eletricos.len() != antes
    }

    /// O nome de uma classe de cabo no catálogo.
    pub fn nome_do_cabo(&self, classe: TipoCabo) -> String {
        self.cabos_eletricos.iter().find(|c| c.classe == classe).map_or_else(|| classe.nome().to_string(), |c| c.nome.clone())
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
