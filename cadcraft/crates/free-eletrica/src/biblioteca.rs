// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Biblioteca de pontos: os blocos do desenho que são pontos elétricos (têm atributos elétricos
//! na definição), a inserção de um ponto com os valores escolhidos e a exclusão de pontos.

use std::sync::Arc;

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::{PolyVertex, Vec2, Vec3};
use cadcraft_doc::{
    Attrib, Block, Circle, Common, Drawing, Entity, EntityKind, HAlign, Handle, Hatch, HatchLoop, Insert, Line, Solid, Space, Text, VAlign,
};
use serde::Serialize;

use crate::familia::{self, Familia, FamilyCategory};
use crate::ponto::{TAGS_ALTURA, TAGS_CIRCUITO, TAGS_COMANDO, TAGS_FASES, TAGS_POTENCIA, TAGS_QUADRO, TAGS_TENSAO, TAGS_TIPO, TipoPonto};

/// Camadas dos pontos inseridos (as do template).
pub const CAMADA_PONTOS: &str = "ELE-PONTOS";
pub const CAMADA_QUADROS: &str = "ELE-QUADROS";
pub const CAMADA_DADOS: &str = "ELE-DADOS";

/// Um atributo da definição do bloco.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtributoBloco {
    pub tag: String,
    pub padrao: String,
    pub prompt: String,
    /// Constante: o valor é da definição e não muda por inserção.
    pub constante: bool,
}

/// Um bloco de ponto elétrico disponível no desenho.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocoPonto {
    pub nome: String,
    pub tipo: TipoPonto,
    /// Quantas vezes está inserido no espaço do modelo.
    pub inseridos: usize,
    pub atributos: Vec<AtributoBloco>,
    /// A família do bloco. `None`: classificado só pelos atributos e pelo nome.
    pub familia: Option<Familia>,
}

impl BlocoPonto {
    /// A altura (m) que uma inserção deste bloco tem pela legenda do projeto.
    pub fn altura_padrao(&self, setup: &crate::setup::Setup) -> f64 {
        if let Some(f) = &self.familia {
            return setup.altura_de(f.ref_altura, f.cota_fixa);
        }
        self.atributos
            .iter()
            .find(|a| TAGS_ALTURA.iter().any(|t| a.tag.eq_ignore_ascii_case(t)))
            .and_then(|a| crate::ponto::altura_m(&a.padrao))
            .unwrap_or_else(|| crate::ponto::altura_padrao(self.tipo, Some(setup.pe_direito)))
    }
}

fn definicoes(b: &Block) -> Vec<&Attrib> {
    b.entities.iter().filter_map(|e| if let EntityKind::AttDef(a) = &e.kind { Some(a) } else { None }).collect()
}

/// Os blocos de ponto elétrico do desenho: os que têm algum atributo elétrico na definição
/// (o mesmo critério da análise) ou uma família. O tipo vem da família, do TIPO_PONTO da definição
/// ou do nome do bloco. Bloco classificado como comum fica de fora.
/// Em ordem de tipo e nome.
pub fn blocos(doc: &Drawing) -> Vec<BlocoPonto> {
    let eletricas = [TAGS_TIPO, TAGS_POTENCIA, TAGS_TENSAO, TAGS_FASES, TAGS_CIRCUITO, TAGS_COMANDO, TAGS_ALTURA, TAGS_QUADRO];
    let mut v: Vec<BlocoPonto> = doc
        .blocks
        .iter()
        .filter(|(nome, _)| !nome.starts_with('*') && !nome.to_uppercase().starts_with("FIACAO-"))
        .filter_map(|(nome, b)| {
            let defs = definicoes(b);
            let tem = |tags: &[&str]| defs.iter().find(|a| tags.iter().any(|t| a.tag.trim().eq_ignore_ascii_case(t)));
            let familia = familia::ler(doc, nome);
            if familia.as_ref().is_some_and(|f| f.categoria == FamilyCategory::Nenhum) {
                return None;
            }
            if familia.is_none() && !eletricas.iter().any(|tags| tem(tags).is_some()) {
                return None;
            }
            let informado = tem(TAGS_TIPO).and_then(|a| TipoPonto::classificar(&a.text.value)).or_else(|| TipoPonto::classificar(nome));
            let tipo = familia.as_ref().and_then(|f| f.categoria.tipo(informado)).or(informado).unwrap_or(TipoPonto::Outro);
            let inseridos = doc.model.iter().filter(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block.eq_ignore_ascii_case(nome))).count();
            let atributos = defs
                .iter()
                .map(|a| AtributoBloco {
                    tag: a.tag.trim().to_string(),
                    padrao: a.text.value.clone(),
                    prompt: a.prompt.clone(),
                    constante: a.constant,
                })
                .collect();
            Some(BlocoPonto { nome: nome.clone(), tipo, inseridos, atributos, familia })
        })
        .collect();
    v.sort_by(|a, b| a.tipo.cmp(&b.tipo).then_with(|| a.nome.to_uppercase().cmp(&b.nome.to_uppercase())));
    v
}

/// Insere o bloco de ponto `nome` em `pos`, girado `rotacao` (graus), com os atributos da
/// definição: os valores de `valores` (tag, valor) por cima dos padrões. Quadros vão para a
/// camada ELE-QUADROS, o resto para ELE-PONTOS.
pub fn inserir(doc: &mut Drawing, nome: &str, pos: [f64; 2], rotacao: f64, valores: &[(String, String)]) -> Result<Handle, String> {
    if !(pos.iter().all(|v| v.is_finite()) && rotacao.is_finite()) {
        return Err("posição ou rotação inválida".into());
    }
    let info = blocos(doc)
        .into_iter()
        .find(|b| b.nome.eq_ignore_ascii_case(nome.trim()))
        .ok_or_else(|| format!("o desenho não tem o bloco de ponto \"{}\"", nome.trim()))?;
    let bloco: Arc<Block> = doc.blocks.get(&info.nome).cloned().ok_or("bloco não encontrado")?;
    for (tag, _) in valores {
        if !info.atributos.iter().any(|a| a.tag.eq_ignore_ascii_case(tag.trim()) && !a.constante) {
            return Err(format!("o bloco \"{}\" não tem o atributo {} (ou ele é constante)", info.nome, tag.trim()));
        }
    }
    let mut ins = Insert {
        block: info.nome.clone(),
        insert: Vec3::new(pos[0], pos[1], 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
        rotation: rotacao.to_radians(),
        attribs: Vec::new(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    };
    let m = ins.transform(bloco.base.xy());
    ins.attribs = definicoes(&bloco)
        .into_iter()
        .filter(|ad| !ad.constant)
        .map(|ad| {
            let mut k = EntityKind::Text(ad.text.clone());
            k.transform(&m);
            let mut texto = if let EntityKind::Text(t) = k { t } else { ad.text.clone() };
            if let Some((_, v)) = valores.iter().find(|(t, _)| t.trim().eq_ignore_ascii_case(ad.tag.trim())) {
                texto.value = v.clone();
            }
            Attrib {
                tag: ad.tag.clone(),
                text: texto,
                invisible: ad.invisible,
                constant: false,
                prompt: String::new(),
                props: Default::default(),
                multiline: ad.multiline.as_ref().map(|ml| ml.transformed(&m)),
                modes: ad.modes.for_reference(),
            }
        })
        .collect();
    let camada = match info.tipo {
        TipoPonto::Quadro => CAMADA_QUADROS,
        TipoPonto::Dados | TipoPonto::Rack => CAMADA_DADOS,
        _ => CAMADA_PONTOS,
    };
    doc.ensure_layer(camada);
    doc.add(&Space::Model, Common { layer: camada.into(), ..Common::default() }, EntityKind::Insert(ins)).map_err(|e| e.to_string())
}

/// Insere um ponto seguindo a família: os padrões dela preenchem os atributos e a altura segue a
/// regra da legenda (`editar::definir_altura`). `ALTURA` em `valores` pode ser número ou `h=1,45m`.
pub fn inserir_ponto(
    doc: &mut Drawing,
    nome: &str,
    pos: [f64; 2],
    rotacao: f64,
    valores: &[(String, String)],
    setup: &crate::setup::Setup,
) -> Result<Handle, String> {
    let e_altura = |t: &str| TAGS_ALTURA.iter().any(|a| t.trim().eq_ignore_ascii_case(a));
    let altura = match valores.iter().find(|(t, v)| e_altura(t) && !v.trim().is_empty()) {
        Some((_, v)) => Some(crate::ponto::altura_m(v).ok_or_else(|| format!("altura \"{v}\" inválida"))?),
        None => None,
    };
    let mut v: Vec<(String, String)> = valores.iter().filter(|(t, _)| !e_altura(t)).cloned().collect();
    let bloco = blocos(doc).into_iter().find(|b| b.nome.eq_ignore_ascii_case(nome.trim()));
    let familia = bloco.as_ref().and_then(|b| b.familia.clone());
    if let (Some(b), Some(f)) = (&bloco, &familia) {
        let livre = |tag: &str, v: &[(String, String)]| {
            b.atributos.iter().any(|a| !a.constante && a.tag.eq_ignore_ascii_case(tag)) && !v.iter().any(|(t, _)| t.eq_ignore_ascii_case(tag))
        };
        if let Some(p) = f.potencia.filter(|_| livre("POTENCIA", &v)) {
            v.push(("POTENCIA".into(), format!("{}VA", crate::ponto::br(p))));
        }
        if let Some(c) = f.classe_demanda.filter(|_| livre("TIPO_CARGA", &v)) {
            v.push(("TIPO_CARGA".into(), c.codigo().to_string()));
        }
        if let Some(fp) = f.fp.filter(|_| livre("FP", &v)) {
            v.push(("FP".into(), crate::ponto::br(fp)));
        }
    }
    let h = inserir(doc, nome, pos, rotacao, &v)?;
    // Sem família e sem altura pedida, o padrão que o próprio bloco traz fica como está.
    if altura.is_some() || familia.is_some() {
        crate::editar::definir_altura(doc, h, altura, setup)?;
    }
    Ok(h)
}

// ---------------------------------------------------------------- blocos e camadas base

/// A caixa de passagem que ELEELETRODUTO insere ao dividir um trecho.
pub const BLOCO_CAIXA: &str = "CX-PASSAGEM";
/// Blocos base, na ordem da barra de inserção.
pub const BLOCOS_BASE: [&str; 13] =
    ["TOM-SIM-BAIXA", "TOM-TUE", "ILU-OCT", "INT-SIM", "INT-2S", "INT-3S", "INT-PAR", "INT-INT", "QDG", "QDC", "RJ45", "RACK", BLOCO_CAIXA];

/// Um traço do símbolo, em metros.
enum Peca {
    Linha(f64, f64, f64, f64),
    Circulo(f64, f64, f64),
    /// Triângulo cheio.
    Cheio([(f64, f64); 3]),
    /// Anel cheio: raio interno e externo.
    Anel(f64, f64),
}

/// Um atributo da definição: tag, valor, pergunta, constante, posição do texto visível (m).
type Att = (&'static str, String, &'static str, bool, Option<(f64, f64)>);

fn att(tag: &'static str, valor: &str, prompt: &'static str) -> Att {
    (tag, valor.to_string(), prompt, false, None)
}

fn constante(tag: &'static str, valor: &str) -> Att {
    (tag, valor.to_string(), "", true, None)
}

fn visivel(tag: &'static str, valor: &str, prompt: &'static str, pos: (f64, f64)) -> Att {
    (tag, valor.to_string(), prompt, false, Some(pos))
}

/// O interruptor: a caixa na parede e o círculo; `marcas` diferenciam os tipos.
fn pecas_interruptor(marcas: &[Peca]) -> Vec<Peca> {
    let mut v = vec![
        Peca::Linha(-0.035, 0.0, 0.035, 0.0),
        Peca::Linha(-0.035, 0.0, -0.035, -0.0475),
        Peca::Linha(0.035, 0.0, 0.035, -0.0475),
        Peca::Linha(0.035, -0.0475, -0.035, -0.0475),
        Peca::Linha(0.0, 0.0, 0.0, 0.06),
        Peca::Circulo(0.0, 0.135, 0.075),
    ];
    for m in marcas {
        v.push(match m {
            Peca::Linha(a, b, c, d) => Peca::Linha(*a, *b, *c, *d),
            Peca::Circulo(a, b, c) => Peca::Circulo(*a, *b, *c),
            Peca::Cheio(p) => Peca::Cheio(*p),
            Peca::Anel(a, b) => Peca::Anel(*a, *b),
        });
    }
    v
}

fn pecas_tomada() -> Vec<Peca> {
    vec![
        Peca::Linha(-0.035, 0.0, 0.035, 0.0),
        Peca::Linha(0.035, 0.0, 0.035, -0.0475),
        Peca::Linha(0.035, -0.0475, -0.035, -0.0475),
        Peca::Linha(-0.035, -0.0475, -0.035, 0.0),
        Peca::Linha(0.0, 0.0, 0.0, 0.0595),
        Peca::Linha(-0.084, 0.0595, 0.084, 0.0595),
        Peca::Linha(0.0, 0.205, 0.084, 0.0595),
        Peca::Linha(-0.084, 0.0595, 0.0, 0.205),
    ]
}

/// A definição de um bloco base: as peças e os atributos.
fn definicao_base(nome: &str, tensao: &str) -> Option<(Vec<Peca>, Vec<Att>)> {
    let interruptor = |tipo: &str, marcas: &[Peca]| {
        (
            pecas_interruptor(marcas),
            vec![
                constante("TIPO_PONTO", "INTERRUPTOR"),
                constante("TIPO_INTERRUPTOR", tipo),
                att("TAG", "", "Identificação"),
                visivel("COMANDO", "", "Letra do comando (a, b, c...)", (0.10, 0.10)),
                att("CIRCUITO", "", "Circuito"),
                att("POTENCIA", "0W", "Potência (interruptor: 0W)"),
                att("TENSAO", tensao, "Tensão"),
                att("NUMERO_FASES", "1", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.10, 0.03)),
            ],
        )
    };
    let tecla = |x: f64| Peca::Linha(x, 0.205, x, 0.235);
    let quadro = |cheio: bool, tag: &str| {
        let mut p = vec![
            Peca::Linha(-0.1, -0.05, -0.1, 0.0),
            Peca::Linha(-0.1, 0.0, 0.1, 0.0),
            Peca::Linha(0.1, 0.0, 0.1, -0.05),
            Peca::Linha(0.1, -0.05, -0.1, -0.05),
            Peca::Linha(-0.1, -0.05, 0.1, 0.0),
        ];
        if cheio {
            p.push(Peca::Cheio([(-0.1, -0.05), (0.1, -0.05), (0.1, 0.0)]));
        }
        (
            p,
            vec![
                constante("TIPO_PONTO", "QUADRO"),
                visivel("TAG", tag, "Identificação do quadro", (0.12, -0.045)),
                att("TENSAO", "", "Tensão (ex.: 380/220V)"),
                att("NUMERO_FASES", "", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.12, -0.11)),
            ],
        )
    };
    Some(match nome {
        "TOM-SIM-BAIXA" => (
            pecas_tomada(),
            vec![
                constante("TIPO_PONTO", "TUG"),
                att("TAG", "", "Identificação"),
                visivel("CIRCUITO", "", "Circuito", (0.10, 0.10)),
                att("POTENCIA", "100VA", "Potência"),
                att("TENSAO", tensao, "Tensão"),
                att("NUMERO_FASES", "1", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.10, 0.03)),
                att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
                att("FP", "0,92", "Fator de potência"),
            ],
        ),
        "TOM-TUE" => {
            let mut p = pecas_tomada();
            p.push(Peca::Cheio([(-0.084, 0.0595), (0.084, 0.0595), (0.0, 0.205)]));
            (
                p,
                vec![
                    constante("TIPO_PONTO", "TUE"),
                    att("TAG", "", "Identificação"),
                    visivel("CIRCUITO", "", "Circuito", (0.10, 0.10)),
                    att("POTENCIA", "1000VA", "Potência"),
                    att("TENSAO", tensao, "Tensão"),
                    att("NUMERO_FASES", "1", "Número de fases"),
                    visivel("ALTURA", "", "Altura fora da legenda", (0.10, 0.03)),
                    att("TIPO_CARGA", "AQUECIMENTO", "Tipo de carga"),
                    att("FP", "1", "Fator de potência"),
                ],
            )
        }
        "ILU-OCT" => (
            vec![Peca::Anel(0.066, 0.093)],
            vec![
                constante("TIPO_PONTO", "ILUM"),
                att("TAG", "", "Identificação"),
                visivel("POTENCIA", "100VA", "Potência", (-0.065, 0.118)),
                visivel("COMANDO", "", "Letra do comando", (-0.118, -0.025)),
                visivel("CIRCUITO", "", "Circuito", (0.118, -0.025)),
                att("TENSAO", tensao, "Tensão"),
                att("NUMERO_FASES", "1", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.118, -0.09)),
                att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
                att("FP", "0,92", "Fator de potência"),
            ],
        ),
        "INT-SIM" => interruptor("1S", &[]),
        "INT-2S" => interruptor("2S", &[tecla(-0.02), tecla(0.02)]),
        "INT-3S" => interruptor("3S", &[tecla(-0.035), tecla(0.0), tecla(0.035)]),
        "INT-PAR" => interruptor("PARALELO", &[Peca::Linha(-0.053, 0.082, 0.053, 0.188)]),
        "INT-INT" => interruptor("INTERMEDIARIO", &[Peca::Linha(-0.053, 0.082, 0.053, 0.188), Peca::Linha(-0.053, 0.188, 0.053, 0.082)]),
        "QDG" => quadro(true, "QDG"),
        "QDC" => quadro(false, "QDC"),
        BLOCO_CAIXA => (
            vec![
                Peca::Linha(-0.05, -0.05, 0.05, -0.05),
                Peca::Linha(0.05, -0.05, 0.05, 0.05),
                Peca::Linha(0.05, 0.05, -0.05, 0.05),
                Peca::Linha(-0.05, 0.05, -0.05, -0.05),
                Peca::Linha(-0.05, -0.05, 0.05, 0.05),
            ],
            vec![
                constante("TIPO_PONTO", "CAIXA DE PASSAGEM"),
                att("TAG", "", "Identificação"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.07, -0.02)),
            ],
        ),
        "RJ45" => (
            vec![
                Peca::Linha(-0.035, 0.0, 0.035, 0.0),
                Peca::Linha(0.035, 0.0, 0.035, -0.0475),
                Peca::Linha(0.035, -0.0475, -0.035, -0.0475),
                Peca::Linha(-0.035, -0.0475, -0.035, 0.0),
                Peca::Linha(0.0, 0.0, 0.0, 0.0595),
                Peca::Linha(-0.075, 0.0595, 0.075, 0.0595),
                Peca::Linha(0.075, 0.0595, 0.075, 0.2095),
                Peca::Linha(0.075, 0.2095, -0.075, 0.2095),
                Peca::Linha(-0.075, 0.2095, -0.075, 0.0595),
            ],
            vec![
                constante("TIPO_PONTO", "DADOS"),
                visivel("ID_PONTO", "", "Identificação do ponto", (0.10, 0.10)),
                visivel("ALTURA", "", "Altura fora da legenda", (0.10, 0.03)),
            ],
        ),
        "RACK" => (
            vec![
                Peca::Linha(-0.1, -0.1, 0.1, -0.1),
                Peca::Linha(0.1, -0.1, 0.1, 0.0),
                Peca::Linha(0.1, 0.0, -0.1, 0.0),
                Peca::Linha(-0.1, 0.0, -0.1, -0.1),
                Peca::Linha(-0.1, -0.05, 0.1, -0.05),
            ],
            vec![
                constante("TIPO_PONTO", "RACK"),
                visivel("TAG", "RACK", "Identificação do rack", (0.12, -0.045)),
                visivel("ALTURA", "", "Altura fora da legenda", (0.12, -0.11)),
            ],
        ),
        _ => return None,
    })
}

fn entidade_da_peca(p: &Peca, u: &dyn Fn(f64) -> f64) -> EntityKind {
    match p {
        Peca::Linha(a, b, c, d) => EntityKind::Line(Line { a: Vec3::new(u(*a), u(*b), 0.0), b: Vec3::new(u(*c), u(*d), 0.0) }),
        Peca::Circulo(x, y, r) => EntityKind::Circle(Circle { center: Vec3::new(u(*x), u(*y), 0.0), radius: u(*r) }),
        Peca::Cheio(t) => {
            let v = t.map(|(x, y)| Vec3::new(u(x), u(y), 0.0));
            EntityKind::Solid(Solid { corners: [v[0], v[1], v[2], v[2]] })
        }
        Peca::Anel(r1, r2) => {
            let circulo = |r: f64| HatchLoop {
                vertices: vec![
                    PolyVertex { p: Vec2::new(u(r), 0.0), bulge: 1.0, start_width: 0.0, end_width: 0.0 },
                    PolyVertex { p: Vec2::new(-u(r), 0.0), bulge: 1.0, start_width: 0.0, end_width: 0.0 },
                ],
                outer: false,
                sources: Vec::new(),
            };
            EntityKind::Hatch(Hatch {
                pattern: "SOLID".into(),
                solid: true,
                loops: vec![circulo(*r1), circulo(*r2)],
                scale: 1.0,
                angle: 0.0,
                associative: false,
                style: 0,
                elevation: 0.0,
                gradient: None,
                origin: Vec2::new(0.0, 0.0),
                background: None,
                pattern_lines: Vec::new(),
            })
        }
    }
}

fn atributo_base(a: &Att, m: f64) -> Attrib {
    let (x, y) = a.4.unwrap_or((0.0, 0.0));
    let p = Vec3::new(x / m, y / m, 0.0);
    Attrib {
        tag: a.0.to_string(),
        text: Text {
            insert: p,
            align_pt: None,
            height: 0.05 / m,
            value: a.1.clone(),
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Left,
            valign: VAlign::Baseline,
        },
        invisible: a.4.is_none(),
        constant: a.3,
        prompt: a.2.to_string(),
        props: Default::default(),
        multiline: None,
        modes: Default::default(),
    }
}

/// Falta alguma camada ou bloco base no desenho.
pub fn falta_base(doc: &Drawing) -> bool {
    BLOCOS_BASE.iter().any(|n| !doc.blocks.contains_key(*n))
        || crate::fiacao::TipoCondutor::TODOS.iter().any(|t| !doc.blocks.contains_key(t.bloco()))
        || [CAMADA_PONTOS, CAMADA_QUADROS, CAMADA_DADOS, crate::fiacao::CAMADA, crate::eletroduto::CAMADA_TETO, crate::eletroduto::CAMADA_PISO]
            .iter()
            .any(|c| doc.layer(c).is_none())
}

/// Auto-bootstrap: põe no desenho as camadas e os blocos base que faltam. O que já existe (um bloco
/// redesenhado pelo projetista, por exemplo) fica como está. Devolve se mudou algo.
pub fn garantir_base(doc: &mut Drawing, setup: &crate::setup::Setup, metros: f64) -> bool {
    if !falta_base(doc) {
        return false;
    }
    let m = if metros > 0.0 { metros } else { 1.0 };
    let tensao = format!("{}V", crate::ponto::br(setup.sistema.tensao_fn()));
    for (nome, cor) in [(CAMADA_PONTOS, 7), (CAMADA_QUADROS, 2), (CAMADA_DADOS, 5)] {
        if doc.ensure_layer(nome)
            && let Some(l) = doc.layer_mut(nome)
        {
            l.color = Color::Index(cor);
        }
    }
    for i in crate::rota::Instalacao::TODAS {
        crate::eletroduto::garantir_camada(doc, i);
    }
    crate::fiacao::garantir_blocos(doc, m);
    for nome in BLOCOS_BASE {
        if doc.blocks.contains_key(nome) {
            continue;
        }
        let Some((pecas, atributos)) = definicao_base(nome, &tensao) else { continue };
        let mut b = Block::new(nome);
        let u = |v: f64| v / m;
        for p in &pecas {
            let k = entidade_da_peca(p, &u);
            b.entities.push(Entity::new(doc.new_handle(), k));
        }
        for a in &atributos {
            let k = EntityKind::AttDef(atributo_base(a, m));
            b.entities.push(Entity::new(doc.new_handle(), k));
        }
        doc.blocks.insert(nome.to_string(), Arc::new(b));
        if let Some(f) = familia::da_base(nome) {
            let _ = familia::gravar(doc, nome, Some(&f));
        }
    }
    true
}

// ---------------------------------------------------------------- utilitários do editor de blocos

/// As geometrias de partida que o editor de blocos oferece: (código, nome).
///
/// Só contornos: preencher um símbolo é com a Hachura nativa do CADCraft.
pub const GEOMETRIAS: [(&str, &str); 5] = [
    ("tomada", "Triângulo de tomada"),
    ("comando", "Círculo de comando"),
    ("luminaria", "Luminária octogonal"),
    ("contorno4x2", "Contorno 4x2"),
    ("contorno4x4", "Contorno 4x4"),
];

const CHAVE_CONECTOR: &str = "connector";
const CAMADA_CONECTOR: &str = "Defpoints";

fn retangulo(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Peca> {
    vec![Peca::Linha(x0, y0, x1, y0), Peca::Linha(x1, y0, x1, y1), Peca::Linha(x1, y1, x0, y1), Peca::Linha(x0, y1, x0, y0)]
}

/// Insere a geometria padrão `forma` no espaço de trabalho (o bloco em edição). Medidas em metros,
/// proporcionais aos símbolos base.
pub fn inserir_geometria(doc: &mut Drawing, forma: &str, metros: f64) -> Result<usize, String> {
    let m = if metros > 0.0 { metros } else { 1.0 };
    let triangulo =
        || vec![Peca::Linha(-0.084, 0.0595, 0.084, 0.0595), Peca::Linha(0.0, 0.205, 0.084, 0.0595), Peca::Linha(-0.084, 0.0595, 0.0, 0.205)];
    let pecas = match forma {
        "tomada" => triangulo(),
        "tomadaMedia" => {
            let mut p = triangulo();
            p.push(Peca::Cheio([(-0.084, 0.0595), (0.0, 0.0595), (0.0, 0.205)]));
            p
        }
        "tomadaAlta" => {
            let mut p = triangulo();
            p.push(Peca::Cheio([(-0.084, 0.0595), (0.084, 0.0595), (0.0, 0.205)]));
            p
        }
        "comando" => vec![Peca::Circulo(0.0, 0.135, 0.075)],
        "luminaria" => {
            // Octógono com o raio do símbolo base.
            let r: f64 = 0.093;
            let v = |i: usize| {
                let a = std::f64::consts::FRAC_PI_8 + std::f64::consts::FRAC_PI_4 * i as f64;
                (r * a.cos(), r * a.sin())
            };
            (0..8)
                .map(|i| {
                    let (a, b) = (v(i), v(i + 1));
                    Peca::Linha(a.0, a.1, b.0, b.1)
                })
                .collect()
        }
        "contorno4x2" => retangulo(-0.05, -0.05, 0.05, 0.0),
        "contorno4x4" => retangulo(-0.05, -0.1, 0.05, 0.0),
        _ => return Err(format!("geometria \"{forma}\" desconhecida")),
    };
    let u = |v: f64| v / m;
    for p in &pecas {
        doc.add(&Space::Model, Common::default(), entidade_da_peca(p, &u)).map_err(|e| e.to_string())?;
    }
    Ok(pecas.len())
}

/// Move o bloco em edição para o ponto `p` virar a origem (0,0), que é o ponto de inserção e de
/// conexão dos eletrodutos, e marca o conector com um círculo na camada Defpoints, que não plota.
pub fn definir_conector(doc: &mut Drawing, p: [f64; 2], metros: f64) -> Result<Handle, String> {
    if !p.iter().all(|v| v.is_finite()) {
        return Err("ponto inválido".into());
    }
    let m = if metros > 0.0 { metros } else { 1.0 };
    let antigos: Vec<Handle> = doc
        .model
        .iter()
        .filter(|e| crate::xdata::ler(&e.common.xdata).iter().any(|(k, _)| k.eq_ignore_ascii_case(CHAVE_CONECTOR)))
        .map(|e| e.handle)
        .collect();
    for h in antigos {
        doc.model.remove(h);
    }
    let mover = cadcraft_doc::geom::Mat3::translate(Vec2::new(-p[0], -p[1]));
    let todos: Vec<Entity> = doc.model.iter().map(|e| (**e).clone()).collect();
    for mut e in todos {
        e.kind.transform(&mover);
        doc.model.replace(e);
    }
    doc.ensure_layer(CAMADA_CONECTOR);
    let mut x = Vec::new();
    crate::xdata::gravar(&mut x, CHAVE_CONECTOR, Some("1"));
    let c = Common { layer: CAMADA_CONECTOR.into(), xdata: x, ..Common::default() };
    doc.add(&Space::Model, c, EntityKind::Circle(Circle { center: Vec3::new(0.0, 0.0, 0.0), radius: 0.02 / m })).map_err(|e| e.to_string())
}

/// Cria no bloco em edição os atributos padrão da categoria, os que ainda não existem. Devolve
/// quantos entraram.
pub fn gerar_atributos(doc: &mut Drawing, categoria: FamilyCategory, metros: f64) -> Result<usize, String> {
    use FamilyCategory as C;
    let m = if metros > 0.0 { metros } else { 1.0 };
    let altura = |pos| visivel("ALTURA", "", "Altura fora da legenda", pos);
    let lista: Vec<Att> = match categoria {
        C::Nenhum => return Err("um bloco CAD comum não tem atributos elétricos: escolha uma categoria".into()),
        C::PontoForca => vec![
            constante("TIPO_PONTO", "TUG"),
            att("TAG", "", "Identificação"),
            visivel("CIRCUITO", "", "Circuito", (0.10, 0.10)),
            altura((0.10, 0.03)),
            att("POTENCIA", "100VA", "Potência"),
            att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
            att("FP", "0,92", "Fator de potência"),
        ],
        C::Luminaria => vec![
            constante("TIPO_PONTO", "ILUM"),
            att("TAG", "", "Identificação"),
            visivel("POTENCIA", "100VA", "Potência", (-0.065, 0.118)),
            visivel("COMANDO", "", "Letra do comando", (-0.118, -0.025)),
            visivel("CIRCUITO", "", "Circuito", (0.118, -0.025)),
            altura((0.118, -0.09)),
            att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
            att("FP", "0,92", "Fator de potência"),
        ],
        C::Interruptor => vec![
            constante("TIPO_PONTO", "INTERRUPTOR"),
            att("TAG", "", "Identificação"),
            visivel("COMANDO", "", "Letra do comando (a, b, c...)", (0.10, 0.10)),
            att("CIRCUITO", "", "Circuito"),
            att("POTENCIA", "0W", "Potência (interruptor: 0W)"),
            altura((0.10, 0.03)),
        ],
        C::Dados => vec![constante("TIPO_PONTO", "DADOS"), visivel("ID_PONTO", "", "Identificação do ponto", (0.10, 0.10)), altura((0.10, 0.03))],
        C::Especial => {
            vec![constante("TIPO_PONTO", "ESPECIAL"), visivel("ID_PONTO", "", "Identificação do ponto", (0.10, 0.10)), altura((0.10, 0.03))]
        }
        C::QuadroEletrico => vec![
            constante("TIPO_PONTO", "QUADRO"),
            visivel("TAG", "QD", "Identificação do quadro", (0.12, -0.045)),
            att("TENSAO", "", "Tensão (ex.: 380/220V)"),
            att("NUMERO_FASES", "", "Número de fases"),
            altura((0.12, -0.11)),
        ],
        C::QuadroDados => {
            vec![constante("TIPO_PONTO", "RACK"), visivel("TAG", "RACK", "Identificação do rack", (0.12, -0.045)), altura((0.12, -0.11))]
        }
        C::Caixa => vec![constante("TIPO_PONTO", "CAIXA DE PASSAGEM"), att("TAG", "", "Identificação"), altura((0.07, -0.02))],
    };
    let existe =
        |doc: &Drawing, tag: &str| doc.model.iter().any(|e| matches!(&e.kind, EntityKind::AttDef(a) if a.tag.trim().eq_ignore_ascii_case(tag)));
    let mut n = 0;
    for a in &lista {
        if !existe(doc, a.0) {
            doc.add(&Space::Model, Common::default(), EntityKind::AttDef(atributo_base(a, m))).map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    Ok(n)
}

/// A rotação (graus) que alinha um ponto à parede mais próxima: a linha mais perto de `q`, fora das
/// camadas elétricas, dentro do raio de captura. `None`: nenhuma parede ali.
pub fn alinhar_a_parede(doc: &Drawing, setup: &crate::setup::Setup, q: [f64; 2], metros: f64) -> Option<f64> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let raio = setup.raio_captura / metros;
    doc.model
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line(_) | EntityKind::LwPolyline(_)))
        .filter(|e| !e.common.layer.to_uppercase().starts_with("ELE-") && !setup.e_camada_eletroduto(&e.common.layer))
        .filter_map(|e| {
            let t = crate::rota::tracado(&e.kind, 0.01 / metros)?;
            let (_, d, i) = crate::rota::perto_do_tracado(&t, q)?;
            let (a, b) = (t.get(i)?, t.get(i + 1)?);
            (d <= raio).then_some((d, (b[1] - a[1]).atan2(b[0] - a[0])))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, ang)| crate::fiacao::angulo_de_leitura(ang).to_degrees())
}

// ---------------------------------------------------------------- modelo do ELEPONTO

/// O que o próximo ELEPONTO insere: o bloco, a rotação (graus) e os valores dos atributos.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Modelo {
    pub bloco: String,
    pub rotacao: f64,
    /// Alinhar à parede mais próxima, quando houver uma sob o cursor.
    pub rotacao_auto: bool,
    pub valores: Vec<(String, String)>,
    /// O quadro que alimenta os pontos inseridos. `None`: o único do desenho, ou definido ao criar
    /// o circuito.
    #[serde(serialize_with = "crate::ponto::handle_hex_opcional")]
    pub quadro: Option<Handle>,
}

/// O modelo de cada desenho aberto (`uid` do desenho): outro desenho ou outra sessão no mesmo
/// processo não usa o modelo deste.
static MODELO: std::sync::Mutex<std::collections::BTreeMap<u64, Modelo>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Escolhe (ou limpa) o que o ELEPONTO insere no desenho `desenho` (a janela Inserir pontos
/// escolhe antes de começar o comando).
pub fn definir_modelo(desenho: u64, m: Option<Modelo>) {
    let mut g = MODELO.lock().unwrap_or_else(|e| e.into_inner());
    match m {
        Some(m) => {
            g.insert(desenho, m);
        }
        None => {
            g.remove(&desenho);
        }
    }
}

/// O modelo escolhido para o desenho `desenho` (fica para as próximas vezes).
pub fn modelo(desenho: u64) -> Option<Modelo> {
    MODELO.lock().unwrap_or_else(|e| e.into_inner()).get(&desenho).cloned()
}

/// Exclui pontos elétricos do desenho. Recusa (sem excluir nada) se algum handle não for um
/// ponto elétrico. Devolve quantos saíram.
pub fn excluir(doc: &mut Drawing, pontos: &[Handle]) -> Result<usize, String> {
    for h in pontos {
        if crate::ponto::ler_ponto(doc, *h).is_none() {
            return Err(format!("o objeto {} não é um ponto elétrico", h.hex()));
        }
    }
    Ok(pontos.iter().filter(|h| doc.model.remove(**h).is_some()).count())
}
