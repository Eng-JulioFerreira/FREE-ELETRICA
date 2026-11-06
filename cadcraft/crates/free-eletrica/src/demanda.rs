// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Cálculo de demanda do quadro pelas tabelas das concessionárias.
//!
//! Cada ponto de carga é um aparelho com uma classe de demanda (na família do bloco ou no atributo
//! TIPO_CARGA da inserção): iluminação e TUG, aquecimento, TUE geral, ar condicionado, motores,
//! bomba d'água, fator manual, outras, reserva. A demanda de cada classe sai da tabela da
//! concessionária escolhida nas Configurações:
//! - iluminação e TUG: fator por faixa de potência, acumulada ou sobre o total, conforme a edificação;
//! - aquecimento e TUE geral: fator pelo número de aparelhos, em duas colunas (potência individual
//!   até o limite ou acima dele), cada coluna com a sua contagem, ou numa coluna só de aquecimento;
//! - ar condicionado, motores e bombas: fator pelo número de aparelhos;
//! - outras cargas: 100 % (a norma deixa o fator ao projetista);
//! - reserva: fora da demanda.
//!
//! Um ponto ou uma família com fator de demanda próprio usa esse fator e sai da contagem da tabela.
//!
//! As potências do Free Elétrica já estão em VA (aparente): a demanda sai em VA sem dividir pelo
//! fator de potência, que nas normas converte potências dadas em W.
//!
//! As tabelas são transcritas das normas públicas das concessionárias e citadas em `fonte`.
//! Confira sempre a revisão em vigor na sua área de concessão.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// O tipo de carga de um circuito no cálculo de demanda.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoCarga {
    #[default]
    IluminacaoTug,
    /// Chuveiro, torneira, aquecedor, forno, fogão elétrico.
    Aquecimento,
    /// TUE geral: geladeira, televisão, som, lava-roupas e semelhantes.
    Eletrodomesticos,
    ArCondicionado,
    Motores,
    /// Motores de bombas d'água.
    Bomba,
    Outros,
    /// O fator informado na família ou no ponto.
    FatorManual,
    /// Circuito reserva: não entra na demanda.
    Reserva,
}

impl TipoCarga {
    pub const TODOS: [TipoCarga; 9] = [
        Self::IluminacaoTug,
        Self::Aquecimento,
        Self::Eletrodomesticos,
        Self::ArCondicionado,
        Self::Motores,
        Self::Bomba,
        Self::Outros,
        Self::FatorManual,
        Self::Reserva,
    ];

    /// As classes oferecidas na família e no inspetor de atributos.
    pub const CLASSES: [TipoCarga; 6] =
        [Self::IluminacaoTug, Self::Aquecimento, Self::ArCondicionado, Self::Motores, Self::Eletrodomesticos, Self::FatorManual];

    /// O nome completo da classe, nos seletores.
    pub fn nome_classe(self) -> &'static str {
        match self {
            Self::IluminacaoTug => "Iluminação + TUG",
            Self::Aquecimento => "Aquecimento / Eletrotérmico (Chuveiro/Torneira/Aquecedor)",
            Self::ArCondicionado => "Ar-Condicionado / Climatização",
            Self::Motores => "Motores",
            Self::Eletrodomesticos => "TUE Geral",
            Self::FatorManual => "Fator Manual Fixo",
            outro => outro.nome(),
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::IluminacaoTug => "Iluminação e TUG",
            Self::Aquecimento => "Aquecimento",
            Self::Eletrodomesticos => "TUE geral",
            Self::ArCondicionado => "Ar condicionado",
            Self::Motores => "Motores",
            Self::Bomba => "Bomba d'água",
            Self::Outros => "Outras cargas",
            Self::FatorManual => "Fator manual",
            Self::Reserva => "Reserva",
        }
    }

    /// Código do atributo TIPO_CARGA dos blocos.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::IluminacaoTug => "ILUM_TUG",
            Self::Aquecimento => "AQUECIMENTO",
            Self::Eletrodomesticos => "ELETRODOMESTICOS",
            Self::ArCondicionado => "AR",
            Self::Motores => "MOTORES",
            Self::Bomba => "BOMBA",
            Self::Outros => "OUTROS",
            Self::FatorManual => "FATOR_MANUAL",
            Self::Reserva => "RESERVA",
        }
    }

    pub fn de_codigo(s: &str) -> Option<TipoCarga> {
        let s = s.trim();
        Self::TODOS.into_iter().find(|t| t.codigo().eq_ignore_ascii_case(s))
    }

    /// O valor do atributo TIPO_CARGA: os códigos e os nomes comuns ("chuveiro", "split",
    /// "geladeira"...), sem diferenciar maiúsculas e acentos.
    pub fn de_texto(s: &str) -> Option<TipoCarga> {
        if let Some(t) = Self::de_codigo(s) {
            return Some(t);
        }
        let n = crate::ponto::normalizar(s);
        let palavras: Vec<&str> = n.split(' ').filter(|p| !p.is_empty()).collect();
        let contem = |p: &str| n.contains(p);
        let palavra = |p: &str| palavras.contains(&p);
        Some(if contem("RESERVA") {
            Self::Reserva
        } else if contem("MANUAL") {
            Self::FatorManual
        } else if contem("AQUEC") || contem("CHUVEIRO") || contem("TORNEIRA") || contem("FORNO") || contem("FOGAO") || contem("BOILER") {
            Self::Aquecimento
        } else if palavra("TUE") || contem("ELETRODOM") || contem("GELADEIRA") || contem("REFRIGERADOR") || contem("LAVA") || contem("MICRO") {
            Self::Eletrodomesticos
        } else if palavra("AR") || contem("CONDICIONADO") || contem("SPLIT") {
            Self::ArCondicionado
        } else if contem("MOTOR") {
            Self::Motores
        } else if contem("BOMBA") {
            Self::Bomba
        } else if contem("OUTR") {
            Self::Outros
        } else if contem("ILUM") || palavra("TUG") || palavra("ILUM TUG") {
            Self::IluminacaoTug
        } else {
            return None;
        })
    }

    /// Fator de potência sugerido: 1,00 em aquecimento resistivo, 0,92 no resto.
    pub fn fp_padrao(self) -> f64 {
        if self == Self::Aquecimento { 1.0 } else { 0.92 }
    }
}

/// Como as faixas de potência de uma edificação valem.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModoFaixas {
    /// Cada faixa aplica o seu fator só à parte da potência dentro dela (Equatorial).
    #[default]
    Acumulada,
    /// O fator da faixa em que o total cai vale para toda a potência (como nas tabelas de residências de algumas concessionárias).
    TotalNaFaixa,
}

/// Uma faixa de potência: o fator vale até `ate_kw` (desde zero); `None` é o que excede a última faixa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Faixa {
    pub ate_kw: Option<f64>,
    pub fator: f64,
}

const fn faixa(ate_kw: f64, fator: f64) -> Faixa {
    Faixa { ate_kw: Some(ate_kw), fator }
}

const fn resto(fator: f64) -> Faixa {
    Faixa { ate_kw: None, fator }
}

/// Iluminação e tomadas de uso geral de um tipo de edificação.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Edificacao {
    pub nome: String,
    pub modo: ModoFaixas,
    pub faixas: Vec<Faixa>,
}

impl Edificacao {
    /// Potência demandada (VA) de iluminação e tomadas.
    pub fn demanda(&self, instalada_va: f64) -> f64 {
        match self.modo {
            ModoFaixas::Acumulada => por_faixas(&self.faixas, instalada_va),
            ModoFaixas::TotalNaFaixa => por_faixa_do_total(&self.faixas, instalada_va),
        }
    }
}

/// Fator pelo número de aparelhos: vale até `ate` aparelhos (o último vale para mais).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PorQuantidade {
    pub ate: u32,
    pub fator: f64,
}

/// Fator pelo número de aparelhos em duas colunas: potência individual até o limite, e acima.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PorQuantidadeDupla {
    pub ate: u32,
    pub ate_limite: f64,
    pub acima_limite: f64,
}

/// As tabelas de demanda de uma concessionária. Uma lista vazia quer dizer 100 %.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TabelaDemanda {
    pub id: String,
    pub nome: String,
    /// A norma, a revisão e as tabelas usadas.
    pub fonte: String,
    pub edificacoes: Vec<Edificacao>,
    /// TUE geral (eletrodomésticos) e, sem a lista `aquecimento`, também o aquecimento.
    pub aparelhos: Vec<PorQuantidadeDupla>,
    /// Limite da potência individual entre as duas colunas de `aparelhos` (kW).
    pub limite_aparelho_kw: f64,
    /// Aquecimento e eletrotérmicos, quando a norma dá uma coluna própria.
    pub aquecimento: Vec<PorQuantidade>,
    pub ar_condicionado: Vec<PorQuantidade>,
    pub bombas: Vec<PorQuantidade>,
    pub motores: Vec<PorQuantidade>,
    /// Criada pelo projetista: pode ser editada e excluída.
    pub propria: bool,
}

fn edificacao(nome: &str, faixas: &[Faixa]) -> Edificacao {
    Edificacao { nome: nome.into(), modo: ModoFaixas::Acumulada, faixas: faixas.to_vec() }
}

fn quantidades(linhas: &[(u32, f64)]) -> Vec<PorQuantidade> {
    linhas.iter().map(|(ate, fator)| PorQuantidade { ate: *ate, fator: *fator }).collect()
}

fn duplas(linhas: &[(u32, f64, f64)]) -> Vec<PorQuantidadeDupla> {
    linhas
        .iter()
        .map(|(ate, ate_limite, acima_limite)| PorQuantidadeDupla { ate: *ate, ate_limite: *ate_limite, acima_limite: *acima_limite })
        .collect()
}

/// Equatorial Energia, NT.00001.EQTL revisão 07 (homologada em 29/03/2023), item 7.1 e
/// tabelas 4, 5 e 10.
fn equatorial_nt001_r07() -> TabelaDemanda {
    TabelaDemanda {
        id: "equatorial-nt001-r07".into(),
        nome: "Equatorial (NT.00001.EQTL rev. 07)".into(),
        fonte: "Equatorial Energia, NT.00001.EQTL revisão 07, 29/03/2023: item 7.1, tabelas 4 (aquecimento e eletrodomésticos), 5 (iluminação e tomadas) e 10 (condicionadores de ar); bombas: k = 1 para uma, 0,5 para mais de uma.".into(),
        edificacoes: vec![
            edificacao("Residências e edifícios de apartamentos", &[faixa(10.0, 1.00), faixa(120.0, 0.35), resto(0.25)]),
            edificacao("Auditórios, salões para exposição", &[resto(1.00)]),
            edificacao("Bancos, lojas", &[resto(1.00)]),
            edificacao("Barbearias, salões de beleza", &[resto(1.00)]),
            edificacao("Clubes", &[resto(1.00)]),
            edificacao("Escolas", &[faixa(12.0, 1.00), resto(0.50)]),
            edificacao("Escritórios", &[faixa(20.0, 1.00), resto(0.70)]),
            edificacao("Garagens comerciais, corredores, depósitos", &[resto(1.00)]),
            edificacao("Hospitais", &[faixa(50.0, 0.40), resto(0.20)]),
            edificacao("Hotéis", &[faixa(20.0, 0.50), faixa(100.0, 0.40), resto(0.30)]),
            edificacao("Igrejas", &[resto(1.00)]),
            edificacao("Restaurantes", &[resto(1.00)]),
        ],
        aparelhos: duplas(&[
            (1, 0.80, 0.80),
            (2, 0.75, 0.65),
            (3, 0.70, 0.55),
            (4, 0.66, 0.50),
            (5, 0.62, 0.45),
            (6, 0.59, 0.43),
            (7, 0.56, 0.40),
            (8, 0.53, 0.36),
            (9, 0.51, 0.35),
            (10, 0.49, 0.34),
            (11, 0.47, 0.32),
            (12, 0.45, 0.32),
            (13, 0.43, 0.32),
            (14, 0.41, 0.32),
            (15, 0.40, 0.32),
            (16, 0.39, 0.28),
            (17, 0.38, 0.28),
            (18, 0.37, 0.28),
            (19, 0.36, 0.28),
            (20, 0.35, 0.28),
            (21, 0.34, 0.26),
            (22, 0.33, 0.26),
            (23, 0.31, 0.26),
            (24, 0.30, 0.26),
            (25, 0.30, 0.26),
            (30, 0.30, 0.24),
            (40, 0.30, 0.22),
            (50, 0.30, 0.20),
            (60, 0.30, 0.18),
            (u32::MAX, 0.30, 0.16),
        ]),
        limite_aparelho_kw: 3.5,
        aquecimento: Vec::new(),
        // Tabela 10: 1 e 2 aparelhos com 1,00 (célula única na norma); acima de 10, 0,70.
        ar_condicionado: quantidades(&[(2, 1.00), (3, 0.88), (4, 0.82), (5, 0.78), (6, 0.76), (7, 0.74), (8, 0.72), (9, 0.71), (u32::MAX, 0.70)]),
        bombas: quantidades(&[(1, 1.0), (u32::MAX, 0.5)]),
        motores: Vec::new(),
        propria: false,
    }
}

/// De fábrica, só a Equatorial: as outras concessionárias o projetista cadastra, com a norma e a revisão que\n/// valem na área dele.
static TABELAS: LazyLock<Vec<TabelaDemanda>> = LazyLock::new(|| vec![equatorial_nt001_r07()]);
static SEM_EDIFICACAO: LazyLock<Edificacao> =
    LazyLock::new(|| Edificacao { nome: String::new(), modo: ModoFaixas::Acumulada, faixas: vec![resto(1.0)] });
static SEM_TABELA: LazyLock<TabelaDemanda> = LazyLock::new(TabelaDemanda::default);

/// As tabelas de demanda de fábrica, com números conferidos nas normas públicas.
pub fn tabelas() -> &'static [TabelaDemanda] {
    &TABELAS
}

/// A tabela de fábrica pelo id; sem ela, a primeira.
pub fn tabela(id: &str) -> &'static TabelaDemanda {
    let todas: &'static [TabelaDemanda] = &TABELAS;
    todas.iter().find(|t| t.id.eq_ignore_ascii_case(id.trim())).or_else(|| todas.first()).unwrap_or(&SEM_TABELA)
}

impl TabelaDemanda {
    /// O tipo de edificação pelo nome; sem ele, o primeiro (residências).
    pub fn edificacao(&self, nome: &str) -> &Edificacao {
        self.edificacoes.iter().find(|e| e.nome.eq_ignore_ascii_case(nome.trim())).or_else(|| self.edificacoes.first()).unwrap_or(&SEM_EDIFICACAO)
    }

    /// Recusa o que não faz sentido: fatores fora de 0 a 1, faixas e quantidades fora de ordem.
    /// A entrada vem da interface, de scripts e do arquivo de preferências.
    pub fn validar(&self) -> Result<(), String> {
        if self.nome.trim().is_empty() {
            return Err("informe o nome da tabela".into());
        }
        if self.edificacoes.is_empty() {
            return Err("cadastre ao menos um tipo de edificação".into());
        }
        let bom = |f: f64| f.is_finite() && (0.0..=1.0).contains(&f);
        for e in &self.edificacoes {
            if e.nome.trim().is_empty() {
                return Err("toda edificação precisa de um nome".into());
            }
            if e.faixas.is_empty() {
                return Err(format!("{}: cadastre ao menos uma faixa de potência", e.nome));
            }
            let mut anterior = 0.0;
            for (i, f) in e.faixas.iter().enumerate() {
                if !bom(f.fator) {
                    return Err(format!("{}: fator {} fora de 0 a 1", e.nome, f.fator));
                }
                match f.ate_kw {
                    Some(a) if !a.is_finite() || a <= anterior => return Err(format!("{}: as faixas precisam crescer", e.nome)),
                    Some(a) => anterior = a,
                    None if i + 1 != e.faixas.len() => return Err(format!("{}: só a última faixa pode ficar sem limite", e.nome)),
                    None => {}
                }
            }
        }
        if !self.limite_aparelho_kw.is_finite() || self.limite_aparelho_kw < 0.0 {
            return Err("limite de potência do aparelho inválido".into());
        }
        let mut anterior = 0;
        for q in &self.aparelhos {
            if q.ate <= anterior || !bom(q.ate_limite) || !bom(q.acima_limite) {
                return Err("TUE geral: quantidades crescentes e fatores de 0 a 1".into());
            }
            anterior = q.ate;
        }
        for (nome, lista) in
            [("aquecimento", &self.aquecimento), ("ar condicionado", &self.ar_condicionado), ("bombas", &self.bombas), ("motores", &self.motores)]
        {
            let mut anterior = 0;
            for q in lista {
                if q.ate <= anterior || !bom(q.fator) {
                    return Err(format!("{nome}: quantidades crescentes e fatores de 0 a 1"));
                }
                anterior = q.ate;
            }
        }
        Ok(())
    }

    /// Um ponto de partida para uma tabela nova: um tipo de edificação com 100 %.
    pub fn em_branco(nome: &str) -> Self {
        TabelaDemanda {
            nome: nome.to_string(),
            fonte: "Cadastrada pelo projetista: informe a norma e a revisão.".into(),
            edificacoes: vec![Edificacao { nome: "Residências".into(), modo: ModoFaixas::Acumulada, faixas: vec![resto(1.0)] }],
            limite_aparelho_kw: 3.5,
            propria: true,
            ..TabelaDemanda::default()
        }
    }
}

/// Potência demandada (VA) pelas faixas acumuladas: cada faixa aplica o seu fator só à parte da
/// potência dentro dela.
pub fn por_faixas(faixas: &[Faixa], instalada_va: f64) -> f64 {
    let total_kw = instalada_va.max(0.0) / 1000.0;
    let mut inicio = 0.0;
    let mut demanda = 0.0;
    for f in faixas {
        let fim = f.ate_kw.unwrap_or(f64::INFINITY);
        if total_kw > inicio {
            demanda += (total_kw.min(fim) - inicio) * f.fator;
        }
        inicio = fim;
        if total_kw <= fim {
            break;
        }
    }
    demanda * 1000.0
}

/// Potência demandada (VA) quando o fator da faixa em que o total cai vale para toda a potência.
pub fn por_faixa_do_total(faixas: &[Faixa], instalada_va: f64) -> f64 {
    let total = instalada_va.max(0.0);
    let kw = total / 1000.0;
    let achada = faixas.iter().find(|f| f.ate_kw.is_none_or(|a| kw <= a)).or(faixas.last());
    achada.map_or(total, |f| total * f.fator)
}

/// Fator para `n` aparelhos.
pub fn fator_quantidade(tabela: &[PorQuantidade], n: usize) -> f64 {
    let n = u32::try_from(n).unwrap_or(u32::MAX);
    tabela.iter().find(|q| n <= q.ate).or(tabela.last()).map_or(1.0, |q| q.fator)
}

/// Fatores (até o limite, acima do limite) para `n` aparelhos de cada coluna.
pub fn fator_aparelhos(tabela: &[PorQuantidadeDupla], n: usize, acima: bool) -> f64 {
    let n = u32::try_from(n).unwrap_or(u32::MAX);
    tabela.iter().find(|q| n <= q.ate).or(tabela.last()).map_or(1.0, |q| if acima { q.acima_limite } else { q.ate_limite })
}

/// As cargas de um tipo: a potência (VA) de cada aparelho (ponto de carga). Com `fator`, o da família
/// ou do ponto vale no lugar da tabela da concessionária.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cargas {
    pub tipo: TipoCarga,
    pub aparelhos: Vec<f64>,
    pub fator: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaDemanda {
    pub tipo: TipoCarga,
    pub aparelhos: usize,
    /// Potência instalada (VA).
    pub instalada: f64,
    /// Fator de demanda resultante (demandada / instalada).
    pub fator: f64,
    /// Potência demandada (VA).
    pub demandada: f64,
    /// A regra aplicada, para o projetista conferir.
    pub regra: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Demanda {
    pub tabela: String,
    pub fonte: String,
    pub edificacao: String,
    pub linhas: Vec<LinhaDemanda>,
    /// Potência instalada (VA), reservas fora.
    pub instalada: f64,
    /// Potência demandada (VA).
    pub demandada: f64,
}

/// Aplica a tabela da concessionária aos aparelhos de um tipo: (demandada, regra).
fn pela_tabela(tabela: &TabelaDemanda, ed: &Edificacao, tipo: TipoCarga, aparelhos: &[f64]) -> (f64, String) {
    let instalada: f64 = aparelhos.iter().sum();
    let n = aparelhos.len();
    let duas_colunas = || {
        let limite = tabela.limite_aparelho_kw * 1000.0;
        let (ate, acima): (Vec<f64>, Vec<f64>) = aparelhos.iter().partition(|p| **p <= limite);
        let d = ate.iter().sum::<f64>() * fator_aparelhos(&tabela.aparelhos, ate.len(), false)
            + acima.iter().sum::<f64>() * fator_aparelhos(&tabela.aparelhos, acima.len(), true);
        (d, format!("{} aparelho(s) até {} kW, {} acima", ate.len(), crate::ponto::br(tabela.limite_aparelho_kw), acima.len()))
    };
    let lista = |t: &[PorQuantidade], nome: &str| -> (f64, String) {
        if t.is_empty() {
            (instalada, format!("{n} {nome}: a tabela não dá fator, 100 %"))
        } else {
            (instalada * fator_quantidade(t, n), format!("{n} {nome}"))
        }
    };
    match tipo {
        TipoCarga::IluminacaoTug => (ed.demanda(instalada), format!("faixas de potência: {}", ed.nome)),
        TipoCarga::Aquecimento if !tabela.aquecimento.is_empty() => lista(&tabela.aquecimento, "aparelho(s)"),
        TipoCarga::Aquecimento | TipoCarga::Eletrodomesticos if tabela.aparelhos.is_empty() => {
            (instalada, format!("{n} aparelho(s): a tabela não dá fator, 100 %"))
        }
        TipoCarga::Aquecimento | TipoCarga::Eletrodomesticos => duas_colunas(),
        TipoCarga::ArCondicionado => lista(&tabela.ar_condicionado, "aparelho(s)"),
        TipoCarga::Bomba => lista(&tabela.bombas, "bomba(s)"),
        TipoCarga::Motores => lista(&tabela.motores, "motor(es)"),
        TipoCarga::FatorManual => (instalada, "fator manual não informado: 100 %".to_string()),
        TipoCarga::Outros => (instalada, "100 % (fator a critério do projetista)".to_string()),
        TipoCarga::Reserva => (0.0, "reserva: fora da demanda".to_string()),
    }
}

/// Calcula a demanda das cargas pela tabela e pelo tipo de edificação. Um aparelho com fator próprio
/// fica fora da contagem da tabela.
pub fn calcular(tabela: &TabelaDemanda, edificacao: &str, cargas: &[Cargas]) -> Demanda {
    let ed = tabela.edificacao(edificacao);
    let mut linhas = Vec::new();
    for tipo in TipoCarga::TODOS {
        let validas = |p: &&f64| p.is_finite() && **p > 0.0;
        let da_tabela: Vec<f64> =
            cargas.iter().filter(|c| c.tipo == tipo && c.fator.is_none()).flat_map(|c| c.aparelhos.iter().filter(validas).copied()).collect();
        let proprios: Vec<(f64, f64)> = cargas
            .iter()
            .filter(|c| c.tipo == tipo)
            .filter_map(|c| c.fator.filter(|f| f.is_finite()).map(|f| (c, f.clamp(0.0, 1.0))))
            .flat_map(|(c, f)| c.aparelhos.iter().filter(validas).map(move |p| (*p, f)))
            .collect();
        if da_tabela.is_empty() && proprios.is_empty() {
            continue;
        }
        let instalada: f64 = da_tabela.iter().sum::<f64>() + proprios.iter().map(|(p, _)| *p).sum::<f64>();
        let (mut demandada, mut regra) = if da_tabela.is_empty() { (0.0, String::new()) } else { pela_tabela(tabela, ed, tipo, &da_tabela) };
        if tipo == TipoCarga::Reserva {
            regra = "reserva: fora da demanda".to_string();
        } else if !proprios.is_empty() {
            demandada += proprios.iter().map(|(p, f)| p * f).sum::<f64>();
            let texto = format!("{} ponto(s) com fator próprio", proprios.len());
            regra = if regra.is_empty() { texto } else { format!("{regra}; {texto}") };
        }
        linhas.push(LinhaDemanda {
            tipo,
            aparelhos: da_tabela.len() + proprios.len(),
            instalada,
            fator: if instalada > 0.0 { demandada / instalada } else { 0.0 },
            demandada,
            regra,
        });
    }
    Demanda {
        tabela: tabela.nome.clone(),
        fonte: tabela.fonte.clone(),
        edificacao: ed.nome.clone(),
        instalada: linhas.iter().filter(|l| l.tipo != TipoCarga::Reserva).map(|l| l.instalada).sum(),
        demandada: linhas.iter().map(|l| l.demandada).sum(),
        linhas,
    }
}
