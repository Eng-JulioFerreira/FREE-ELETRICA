// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Famílias de bloco, no espírito de "categoria e parâmetros de família": cada definição de bloco
//! diz a que sistema pertence, o que é, em que altura fica por padrão, que caixa física usa e quais
//! são os valores padrão dela. Fica no XDATA do `BLOCK_RECORD` (aplicativo `FREE_ELETRICA`) e viaja
//! com o DXF. As inserções herdam da família e só guardam o que foge dela.

use std::sync::Arc;

use cadcraft_doc::foreign::ForeignExtra;
use cadcraft_doc::{Drawing, RawTag};
use serde::Serialize;

use crate::ponto::TipoPonto;
use crate::xdata;

const REGISTRO: &str = "BLOCK_RECORD";

const CHAVE_DOMINIO: &str = "domain";
const CHAVE_CATEGORIA: &str = "category";
const CHAVE_REF_ALTURA: &str = "height_ref";
const CHAVE_COTA: &str = "fixed_elevation_m";
const CHAVE_CAIXA: &str = "physical_box";
const CHAVE_POTENCIA: &str = "power_va";
const CHAVE_FP: &str = "power_factor";
const CHAVE_TECLAS: &str = "keys";
const CHAVE_CLASSE: &str = "demand_class";
const CHAVE_FATOR: &str = "demand_factor";
const CHAVE_LEGENDA: &str = "legend_name";
const CHAVE_DESC_MATERIAL: &str = "material_desc";
/// Bloco de fábrica que o projetista excluiu: a preparação do desenho não o recria.
const CHAVE_EXCLUIDA: &str = "factory_block_deleted";
/// O tamanho máximo dos textos da família: um grupo do XDATA tem limite.
const TEXTO_MAXIMO: usize = 200;

/// O sistema a que um elemento pertence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemDomain {
    #[default]
    Power,
    DataTelecom,
    FireSecurity,
    Infra,
}

impl SystemDomain {
    pub const TODOS: [SystemDomain; 4] = [Self::Power, Self::DataTelecom, Self::FireSecurity, Self::Infra];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Power => "POWER",
            Self::DataTelecom => "DATA_TELECOM",
            Self::FireSecurity => "FIRE_SECURITY",
            Self::Infra => "INFRA",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Power => "Elétrica",
            Self::DataTelecom => "Dados/Telecom",
            Self::FireSecurity => "Incêndio/Especial",
            Self::Infra => "Infraestrutura",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|d| d.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// O que o bloco é.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FamilyCategory {
    /// Bloco comum de CAD: nunca é um ponto, mesmo com atributos elétricos.
    Nenhum,
    #[default]
    PontoForca,
    Luminaria,
    Interruptor,
    Dados,
    Especial,
    QuadroEletrico,
    QuadroDados,
    Caixa,
}

impl FamilyCategory {
    pub const TODAS: [FamilyCategory; 9] = [
        Self::Nenhum,
        Self::PontoForca,
        Self::Luminaria,
        Self::Interruptor,
        Self::Dados,
        Self::Especial,
        Self::QuadroEletrico,
        Self::QuadroDados,
        Self::Caixa,
    ];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Nenhum => "NONE",
            Self::PontoForca => "ELECTRICAL_FIXTURE",
            Self::Luminaria => "LIGHTING_FIXTURE",
            Self::Interruptor => "LIGHTING_DEVICE",
            Self::Dados => "DATA_DEVICE",
            Self::Especial => "FIRE_SECURITY_DEVICE",
            Self::QuadroEletrico => "ELECTRICAL_EQUIPMENT",
            Self::QuadroDados => "TELECOM_ENCLOSURE",
            Self::Caixa => "JUNCTION_BOX",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Nenhum => "Nenhum (bloco CAD comum)",
            Self::PontoForca => "Ponto de força / tomada",
            Self::Luminaria => "Luminária",
            Self::Interruptor => "Interruptor / comando",
            Self::Dados => "Ponto de dados / telecom",
            Self::Especial => "Dispositivo especial / incêndio",
            Self::QuadroEletrico => "Quadro elétrico (QDG/QDC)",
            Self::QuadroDados => "Quadro de dados / rack",
            Self::Caixa => "Caixa de passagem",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }

    /// O sistema natural da categoria.
    pub fn dominio_padrao(self) -> SystemDomain {
        match self {
            Self::Dados | Self::QuadroDados => SystemDomain::DataTelecom,
            Self::Especial => SystemDomain::FireSecurity,
            Self::Caixa | Self::Nenhum => SystemDomain::Infra,
            _ => SystemDomain::Power,
        }
    }

    /// A altura natural da categoria.
    pub fn ref_altura_padrao(self) -> RefAltura {
        match self {
            Self::Luminaria | Self::Caixa => RefAltura::Teto,
            Self::Interruptor => RefAltura::Media,
            Self::Especial => RefAltura::Alta,
            Self::QuadroEletrico | Self::QuadroDados => RefAltura::Fixa,
            _ => RefAltura::Baixa,
        }
    }

    /// O tipo de ponto da categoria. Ponto de força respeita o que o bloco diz (TUG ou TUE).
    pub fn tipo(self, informado: Option<TipoPonto>) -> Option<TipoPonto> {
        Some(match self {
            Self::Nenhum => return None,
            Self::PontoForca => match informado {
                Some(TipoPonto::TomadaEspecifica) => TipoPonto::TomadaEspecifica,
                _ => TipoPonto::Tomada,
            },
            Self::Luminaria => TipoPonto::Iluminacao,
            Self::Interruptor => TipoPonto::Interruptor,
            Self::Dados => TipoPonto::Dados,
            Self::Especial => TipoPonto::Especial,
            Self::QuadroEletrico => TipoPonto::Quadro,
            Self::QuadroDados => TipoPonto::Rack,
            Self::Caixa => TipoPonto::Caixa,
        })
    }
}

/// Em que altura do Setup o bloco fica por padrão.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RefAltura {
    #[default]
    Baixa,
    Media,
    Alta,
    Teto,
    Piso,
    /// A cota fixa da família.
    Fixa,
}

impl RefAltura {
    pub const TODAS: [RefAltura; 6] = [Self::Baixa, Self::Media, Self::Alta, Self::Teto, Self::Piso, Self::Fixa];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Baixa => "LOW",
            Self::Media => "MEDIUM",
            Self::Alta => "HIGH",
            Self::Teto => "CEILING",
            Self::Piso => "FLOOR",
            Self::Fixa => "FIXED",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Baixa => "Baixa",
            Self::Media => "Média",
            Self::Alta => "Alta",
            Self::Teto => "Teto",
            Self::Piso => "Piso",
            Self::Fixa => "Cota fixa",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|r| r.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// A caixa física do ponto, para a lista de materiais.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaixaFisica {
    #[default]
    Nenhuma,
    C4x2,
    C4x4,
    Octogonal,
    Passagem15,
    Passagem20,
    Passagem30,
}

impl CaixaFisica {
    pub const TODAS: [CaixaFisica; 7] =
        [Self::Nenhuma, Self::C4x2, Self::C4x4, Self::Octogonal, Self::Passagem15, Self::Passagem20, Self::Passagem30];

    pub fn codigo(self) -> &'static str {
        match self {
            Self::Nenhuma => "NONE",
            Self::C4x2 => "4X2",
            Self::C4x4 => "4X4",
            Self::Octogonal => "OCTAGONAL",
            Self::Passagem15 => "PULL_15X15",
            Self::Passagem20 => "PULL_20X20",
            Self::Passagem30 => "PULL_30X30",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Nenhuma => "Nenhuma",
            Self::C4x2 => "4x2\"",
            Self::C4x4 => "4x4\"",
            Self::Octogonal => "Octogonal",
            Self::Passagem15 => "Passagem 15x15",
            Self::Passagem20 => "Passagem 20x20",
            Self::Passagem30 => "Passagem 30x30",
        }
    }

    pub fn de_codigo(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|c| c.codigo().eq_ignore_ascii_case(t.trim()))
    }
}

/// As propriedades de uma família.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Familia {
    pub dominio: SystemDomain,
    pub categoria: FamilyCategory,
    pub ref_altura: RefAltura,
    /// A cota (m) quando a referência é [`RefAltura::Fixa`].
    pub cota_fixa: f64,
    pub caixa: CaixaFisica,
    /// Potência padrão (VA). `None`: a do atributo do bloco.
    pub potencia: Option<f64>,
    /// Fator de potência padrão. `None`: o do atributo do bloco.
    pub fp: Option<f64>,
    /// Número de teclas ou pontos.
    pub teclas: u8,
    /// Classe de demanda. `None`: a do atributo TIPO_CARGA ou a natural do tipo de ponto.
    pub classe_demanda: Option<crate::demanda::TipoCarga>,
    /// Fator de demanda, de 0 a 1. `None`: automático, pela tabela da concessionária.
    pub fator_demanda: Option<f64>,
    /// O nome na Legenda de Símbolos da prancha.
    pub legenda: String,
    /// A descrição comercial na Lista de Materiais.
    pub desc_material: String,
}

impl Default for Familia {
    fn default() -> Self {
        Familia::da_categoria(FamilyCategory::default())
    }
}

impl Familia {
    /// Uma família com os valores naturais da categoria.
    pub fn da_categoria(categoria: FamilyCategory) -> Self {
        Familia {
            dominio: categoria.dominio_padrao(),
            categoria,
            ref_altura: categoria.ref_altura_padrao(),
            cota_fixa: 1.5,
            caixa: match categoria {
                FamilyCategory::PontoForca | FamilyCategory::Interruptor | FamilyCategory::Dados => CaixaFisica::C4x2,
                FamilyCategory::Luminaria => CaixaFisica::Octogonal,
                FamilyCategory::Caixa => CaixaFisica::Passagem15,
                _ => CaixaFisica::Nenhuma,
            },
            potencia: None,
            fp: None,
            teclas: 1,
            classe_demanda: None,
            fator_demanda: None,
            legenda: String::new(),
            desc_material: String::new(),
        }
    }

    /// Recusa valores fora do que faz sentido: entrada hostil não chega ao desenho.
    pub fn validar(&self) -> Result<(), String> {
        if !(self.cota_fixa.is_finite() && (0.0..=50.0).contains(&self.cota_fixa)) {
            return Err(format!("cota fixa {} inválida: use de 0 a 50 m", self.cota_fixa));
        }
        if let Some(p) = self.potencia
            && !(p.is_finite() && p >= 0.0)
        {
            return Err(format!("potência {p} inválida"));
        }
        if let Some(f) = self.fp
            && !(f.is_finite() && f > 0.0 && f <= 1.0)
        {
            return Err(format!("fator de potência {f} inválido: use um valor acima de 0 e até 1"));
        }
        if let Some(f) = self.fator_demanda
            && !(f.is_finite() && (0.0..=1.0).contains(&f))
        {
            return Err(format!("fator de demanda {f} inválido: use de 0,00 a 1,00"));
        }
        if !(1..=3).contains(&self.teclas) {
            return Err(format!("{} teclas: use de 1 a 3", self.teclas));
        }
        for (nome, t) in [("o nome na legenda", &self.legenda), ("a descrição na lista de materiais", &self.desc_material)] {
            if t.chars().count() > TEXTO_MAXIMO {
                return Err(format!("{nome} passa de {TEXTO_MAXIMO} caracteres"));
            }
        }
        Ok(())
    }
}

fn chave(bloco: &str) -> (String, String) {
    (REGISTRO.to_string(), bloco.trim().to_ascii_uppercase())
}

/// A família gravada num bloco. `None`: o bloco não tem família.
pub fn ler(doc: &Drawing, bloco: &str) -> Option<Familia> {
    let extra = doc.foreign.records.get(&chave(bloco))?;
    de_xdata(&extra.xdata)
}

fn de_xdata(x: &[RawTag]) -> Option<Familia> {
    let pares = xdata::ler(x);
    let valor = |k: &str| pares.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
    let categoria = FamilyCategory::de_codigo(valor(CHAVE_CATEGORIA)?)?;
    let num = |k: &str| valor(k).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite());
    let base = Familia::da_categoria(categoria);
    let f = Familia {
        dominio: valor(CHAVE_DOMINIO).and_then(SystemDomain::de_codigo).unwrap_or(base.dominio),
        categoria,
        ref_altura: valor(CHAVE_REF_ALTURA).and_then(RefAltura::de_codigo).unwrap_or(base.ref_altura),
        cota_fixa: num(CHAVE_COTA).unwrap_or(base.cota_fixa),
        caixa: valor(CHAVE_CAIXA).and_then(CaixaFisica::de_codigo).unwrap_or(base.caixa),
        potencia: num(CHAVE_POTENCIA),
        fp: num(CHAVE_FP),
        teclas: num(CHAVE_TECLAS).map_or(base.teclas, |t| t as u8),
        classe_demanda: valor(CHAVE_CLASSE).and_then(crate::demanda::TipoCarga::de_codigo),
        fator_demanda: num(CHAVE_FATOR),
        legenda: valor(CHAVE_LEGENDA).unwrap_or_default().to_string(),
        desc_material: valor(CHAVE_DESC_MATERIAL).unwrap_or_default().to_string(),
    };
    // Um arquivo editado à mão não derruba a leitura: volta ao natural da categoria.
    Some(if f.validar().is_ok() { f } else { base })
}

/// Grava a família de um bloco (`None` tira). Outros dados do registro ficam como estão.
pub fn gravar(doc: &mut Drawing, bloco: &str, familia: Option<&Familia>) -> Result<(), String> {
    if bloco.trim().is_empty() {
        return Err("informe o bloco".into());
    }
    if let Some(f) = familia {
        f.validar()?;
    }
    let foreign = Arc::make_mut(&mut doc.foreign);
    let registro: &mut ForeignExtra = foreign.records.entry(chave(bloco)).or_default();
    let texto = |v: f64| v.to_string();
    let pares: [(&str, Option<String>); 13] = [
        (crate::rota::CHAVE_ESQUEMA, familia.map(|_| crate::rota::VERSAO_ESQUEMA.to_string())),
        (CHAVE_DOMINIO, familia.map(|f| f.dominio.codigo().to_string())),
        (CHAVE_CATEGORIA, familia.map(|f| f.categoria.codigo().to_string())),
        (CHAVE_REF_ALTURA, familia.map(|f| f.ref_altura.codigo().to_string())),
        (CHAVE_COTA, familia.map(|f| texto(f.cota_fixa))),
        (CHAVE_CAIXA, familia.map(|f| f.caixa.codigo().to_string())),
        (CHAVE_POTENCIA, familia.and_then(|f| f.potencia).map(texto)),
        (CHAVE_FP, familia.and_then(|f| f.fp).map(texto)),
        (CHAVE_TECLAS, familia.map(|f| f.teclas.to_string())),
        (CHAVE_CLASSE, familia.and_then(|f| f.classe_demanda).map(|c| c.codigo().to_string())),
        (CHAVE_FATOR, familia.and_then(|f| f.fator_demanda).map(texto)),
        (CHAVE_LEGENDA, familia.map(|f| f.legenda.trim().to_string()).filter(|t| !t.is_empty())),
        (CHAVE_DESC_MATERIAL, familia.map(|f| f.desc_material.trim().to_string()).filter(|t| !t.is_empty())),
    ];
    for (k, v) in &pares {
        xdata::gravar(&mut registro.xdata, k, v.as_deref());
    }
    if familia.is_some() {
        xdata::gravar(&mut registro.xdata, CHAVE_EXCLUIDA, None);
    }
    Ok(())
}

/// O bloco de fábrica foi excluído pelo projetista neste desenho.
pub fn excluida(doc: &Drawing, bloco: &str) -> bool {
    doc.foreign.records.get(&chave(bloco)).is_some_and(|r| xdata::ler(&r.xdata).iter().any(|(k, _)| k.eq_ignore_ascii_case(CHAVE_EXCLUIDA)))
}

/// Marca (ou desmarca) um bloco de fábrica como excluído.
pub fn marcar_excluida(doc: &mut Drawing, bloco: &str, sim: bool) {
    let foreign = Arc::make_mut(&mut doc.foreign);
    xdata::gravar(&mut foreign.records.entry(chave(bloco)).or_default().xdata, CHAVE_EXCLUIDA, sim.then_some("1"));
}

/// A família natural de um bloco base, com os textos da legenda e da lista de materiais.
pub fn da_base(nome: &str) -> Option<Familia> {
    use FamilyCategory as C;
    let com = |categoria: C, legenda: String, material: String, ajustar: &dyn Fn(&mut Familia)| {
        let mut f = Familia::da_categoria(categoria);
        f.legenda = legenda;
        f.desc_material = material;
        ajustar(&mut f);
        Some(f)
    };
    let txt = |a: &str, b: &str| (a.to_string(), b.to_string());
    if let Some((duplas, altura)) = crate::biblioteca::tomada_base(nome) {
        let (ref_altura, onde, h) = match altura {
            crate::biblioteca::Nivel::Baixo => (RefAltura::Baixa, "baixa", "0,30"),
            crate::biblioteca::Nivel::Medio => (RefAltura::Media, "média", "1,20"),
            crate::biblioteca::Nivel::Alto => (RefAltura::Alta, "alta", "2,10"),
        };
        let (quantas, conjunto) = if duplas { ("dupla ", "Conjunto 2 Tomadas 2P+T 10A") } else { ("", "Conjunto 1 Tomada 2P+T 10A") };
        return com(C::PontoForca, format!("Tomada {onde} {quantas}2P+T 10A - h={h}m"), format!("{conjunto} + Placa 4x2 + Caixa 4x2"), &|f| {
            f.ref_altura = ref_altura;
            f.teclas = if duplas { 2 } else { 1 };
        });
    }
    if let Some(i) = crate::biblioteca::interruptor_base(nome) {
        let (tipo, comercial) = match i.funcao {
            crate::biblioteca::FuncaoTecla::Simples => ("simples", "simples"),
            crate::biblioteca::FuncaoTecla::Paralelo => ("paralelo", "paralela (three-way)"),
            crate::biblioteca::FuncaoTecla::Intermediario => ("intermediário", "intermediária (four-way)"),
        };
        let (ponto, h, ref_altura) = if i.alto { ("alto", "2,10", RefAltura::Alta) } else { ("médio", "1,20", RefAltura::Media) };
        let secoes = i.secoes;
        let tecla = if secoes == 1 { "tecla" } else { "teclas" };
        let secao = if secoes == 1 { "seção" } else { "seções" };
        return com(
            C::Interruptor,
            format!("Interruptor {tipo} {secoes} {secao} - ponto {ponto}, h={h}m"),
            format!("Conjunto {secoes} {tecla} {comercial} + Placa 4x2 + Caixa 4x2"),
            &|f| {
                f.teclas = secoes;
                f.ref_altura = ref_altura;
            },
        );
    }
    let (categoria, (legenda, material)) = match nome {
        "TOM-TUE" => (C::PontoForca, txt("Tomada de uso específico (TUE)", "Conjunto 1 Tomada 2P+T 20A + Placa 4x2 + Caixa 4x2")),
        "ILU-OCT" => (C::Luminaria, txt("Ponto de luz no teto", "Ponto de luz + Caixa octogonal")),
        "ILU-PAR" => (C::Luminaria, txt("Ponto de luz na parede (arandela) - h=2,10m", "Ponto de luz + Caixa 4x2")),
        "QDG" => (C::QuadroEletrico, txt("Quadro de distribuição geral", "Quadro de distribuição geral")),
        "QDC" => (C::QuadroEletrico, txt("Quadro de distribuição de circuitos", "Quadro de distribuição de circuitos")),
        "RJ45" => (C::Dados, txt("Ponto de dados/telecom", "Conjunto 1 Tomada RJ45 + Placa 4x2 + Caixa 4x2")),
        "RACK" => (C::QuadroDados, txt("Rack de dados", "Rack de dados")),
        "CX-PASSAGEM" => (C::Caixa, txt("Caixa de passagem", "Caixa de passagem")),
        "CX-OCT" => (C::Caixa, txt("Caixa de passagem octogonal (teto)", "Caixa octogonal 4x4\" de teto")),
        _ => return None,
    };
    com(categoria, legenda, material, &|f| {
        if nome == "TOM-TUE" {
            f.ref_altura = RefAltura::Media;
        }
        if nome == "ILU-PAR" {
            f.ref_altura = RefAltura::Alta;
        }
        if nome == "CX-OCT" {
            f.ref_altura = RefAltura::Teto;
            f.caixa = CaixaFisica::Octogonal;
            f.potencia = None;
        }
    })
}
