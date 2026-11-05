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
        .filter(|(nome, _)| !nome.starts_with('*') && !nome.to_uppercase().starts_with("FIACAO-") && !nome.ends_with(SUFIXO_EMPILHADO))
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
    encaixar_textos(&mut ins, &bloco);
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
/// A caixa de passagem octogonal de teto: derivação sem carga, sem potência nem comando.
pub const BLOCO_CAIXA_OCT: &str = "CX-OCT";
/// O raio do octógono da caixa de teto (m).
const RAIO_OCTOGONO: f64 = 0.06;
/// O ponto de luz no teto e o ponto de luz na parede (arandela).
pub const BLOCO_LUZ_TETO: &str = "ILU-OCT";
pub const BLOCO_ARANDELA: &str = "ILU-PAR";

/// O raio do símbolo de ponto de luz (m): o círculo do teto e o semicírculo da parede (NBR 5444).
pub const RAIO_LUZ: f64 = 0.10;
/// A altura dos textos dentro do símbolo de ponto de luz (m), quando cabem.
pub const TEXTO_LUZ: f64 = 0.03;
/// Um compartimento do símbolo de ponto de luz: (tag, centro x, centro y, largura, altura), em m. O retângulo
/// cabe inteiro na divisão do símbolo.
pub type Compartimento = (&'static str, f64, f64, f64, f64);
/// Teto: a potência na metade de cima; o comando e o circuito nos quartos de baixo.
const LUZ_TETO: [Compartimento; 3] =
    [("POTENCIA", 0.0, 0.042, 0.15, 0.036), ("COMANDO", -0.042, -0.040, 0.066, 0.036), ("CIRCUITO", 0.042, -0.040, 0.066, 0.036)];
/// Parede: o D com a face reta na parede (y = 0) e a divisória até a borda; o comando de um lado e o
/// circuito do outro. A potência fica nos dados do ponto.
const LUZ_PAREDE: [Compartimento; 2] = [("COMANDO", -0.042, 0.040, 0.066, 0.036), ("CIRCUITO", 0.042, 0.040, 0.066, 0.036)];

/// Os compartimentos de texto do ponto de luz base `nome` (ou do módulo empilhado dele).
pub fn compartimentos_de_luz(nome: &str) -> Option<&'static [Compartimento]> {
    match bloco_de_origem(nome) {
        BLOCO_LUZ_TETO => Some(&LUZ_TETO),
        BLOCO_ARANDELA => Some(&LUZ_PAREDE),
        _ => None,
    }
}

/// O circuito como a planta mostra dentro do ponto de luz: só o número (`1`), sem os traços que versões
/// anteriores punham em volta.
pub fn circuito_na_planta(circuito: &str) -> String {
    circuito.trim().trim_matches('-').trim().to_string()
}
/// Blocos base, na ordem da barra de inserção. Cada tomada e cada interruptor é um módulo só: duas tomadas
/// ou duas teclas no mesmo ponto são dois blocos empilhados ([`empilhar`]), cada um com o seu circuito e o
/// seu comando.
pub const BLOCOS_BASE: [&str; 18] = [
    "TOM-SIM-BAIXA",
    "TOM-SIM-MEDIA",
    "TOM-SIM-ALTA",
    "TOM-TUE",
    "ILU-OCT",
    BLOCO_ARANDELA,
    "INT-SIM",
    "INT-PAR",
    "INT-INT",
    "INT-SIM-ALTO",
    "INT-PAR-ALTO",
    "INT-INT-ALTO",
    "QDG",
    "QDC",
    "RJ45",
    "RACK",
    BLOCO_CAIXA,
    BLOCO_CAIXA_OCT,
];

/// A altura de uma tomada base, que o preenchimento do triângulo mostra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nivel {
    Baixo,
    Medio,
    Alto,
}

/// Uma tomada base: (dupla, altura).
pub fn tomada_base(nome: &str) -> Option<(bool, Nivel)> {
    Some(match nome {
        "TOM-SIM-BAIXA" => (false, Nivel::Baixo),
        "TOM-DUP-BAIXA" => (true, Nivel::Baixo),
        "TOM-SIM-MEDIA" => (false, Nivel::Medio),
        "TOM-DUP-MEDIA" => (true, Nivel::Medio),
        "TOM-SIM-ALTA" => (false, Nivel::Alto),
        "TOM-DUP-ALTA" => (true, Nivel::Alto),
        _ => return None,
    })
}

/// A função de cada tecla, que o preenchimento do círculo mostra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuncaoTecla {
    Simples,
    Paralelo,
    Intermediario,
}

/// Um interruptor base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterruptorBase {
    pub funcao: FuncaoTecla,
    pub secoes: u8,
    /// Ponto alto; sem isso, ponto médio.
    pub alto: bool,
}

/// O interruptor base de um nome: `INT-<SIM|2S|3S|PAR|INT>[-2S|-3S][-ALTO]`.
pub fn interruptor_base(nome: &str) -> Option<InterruptorBase> {
    let (corpo, alto) = match nome.strip_suffix("-ALTO") {
        Some(c) => (c, true),
        None => (nome, false),
    };
    let (funcao, secoes) = match corpo {
        "INT-SIM" => (FuncaoTecla::Simples, 1),
        "INT-2S" => (FuncaoTecla::Simples, 2),
        "INT-3S" => (FuncaoTecla::Simples, 3),
        "INT-PAR" => (FuncaoTecla::Paralelo, 1),
        "INT-PAR-2S" => (FuncaoTecla::Paralelo, 2),
        "INT-PAR-3S" => (FuncaoTecla::Paralelo, 3),
        "INT-INT" => (FuncaoTecla::Intermediario, 1),
        "INT-INT-2S" => (FuncaoTecla::Intermediario, 2),
        "INT-INT-3S" => (FuncaoTecla::Intermediario, 3),
        _ => return None,
    };
    Some(InterruptorBase { funcao, secoes, alto })
}

/// Um traço do símbolo, em metros.
enum Peca {
    Linha(f64, f64, f64, f64),
    Circulo(f64, f64, f64),
    /// Triângulo cheio.
    Cheio([(f64, f64); 3]),
    /// Círculo cheio: centro e raio.
    Disco(f64, f64, f64),
    /// Metade esquerda de um círculo, cheia: centro e raio.
    MeioDisco(f64, f64, f64),
    /// Arco anti-horário: centro, raio e ângulos de início e fim (graus).
    Arco(f64, f64, f64, f64, f64),
}

/// Um octógono regular de raio `r`, com lados horizontais e verticais.
fn octogono(r: f64) -> Vec<Peca> {
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

/// O ponto de luz no teto (NBR 5444): o círculo, o diâmetro horizontal e o raio que divide a metade de baixo.
fn pecas_luz_teto() -> Vec<Peca> {
    vec![Peca::Circulo(0.0, 0.0, RAIO_LUZ), Peca::Linha(-RAIO_LUZ, 0.0, RAIO_LUZ, 0.0), Peca::Linha(0.0, 0.0, 0.0, -RAIO_LUZ)]
}

/// O ponto de luz na parede (NBR 5444): o D com a face reta na parede e a divisória do meio da face até a borda.
fn pecas_arandela() -> Vec<Peca> {
    vec![Peca::Arco(0.0, 0.0, RAIO_LUZ, 0.0, 180.0), Peca::Linha(-RAIO_LUZ, 0.0, RAIO_LUZ, 0.0), Peca::Linha(0.0, 0.0, 0.0, RAIO_LUZ)]
}

/// O raio do círculo do interruptor e a altura do centro do primeiro (m).
const RAIO_TECLA: f64 = 0.075;
const CENTRO_TECLA: f64 = 0.135;
/// O triângulo da tomada: a base, o topo e a meia largura (m).
const BASE_TRIANGULO: f64 = 0.0595;
const TOPO_TRIANGULO: f64 = 0.205;
const MEIA_BASE: f64 = 0.084;

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

/// O desenho de interruptor das versões anteriores: a caixa na parede, um círculo e as marcas. Só para
/// reconhecer e trocar um bloco base antigo.
fn pecas_interruptor_antigo(marcas: &[Peca]) -> Vec<Peca> {
    let mut v = caixa_e_haste(0.06);
    v.push(Peca::Circulo(0.0, CENTRO_TECLA, RAIO_TECLA));
    for m in marcas {
        if let Peca::Linha(a, b, c, d) = m {
            v.push(Peca::Linha(*a, *b, *c, *d));
        }
    }
    v
}

/// O bloco base no desenho antigo, se o nome teve um.
fn geometria_antiga(nome: &str) -> Option<Vec<Peca>> {
    let tecla = |x: f64| Peca::Linha(x, 0.205, x, 0.235);
    let marcas: Vec<Peca> = match nome {
        "INT-SIM" => Vec::new(),
        "INT-2S" => vec![tecla(-0.02), tecla(0.02)],
        "INT-3S" => vec![tecla(-0.035), tecla(0.0), tecla(0.035)],
        "INT-PAR" => vec![Peca::Linha(-0.053, 0.082, 0.053, 0.188)],
        "INT-INT" => vec![Peca::Linha(-0.053, 0.082, 0.053, 0.188), Peca::Linha(-0.053, 0.188, 0.053, 0.082)],
        _ => return None,
    };
    Some(pecas_interruptor_antigo(&marcas))
}

/// O bloco é o desenho padrão de uma versão anterior: linhas e círculos iguais, na mesma ordem.
fn e_desenho_antigo(b: &Block, pecas: &[Peca], m: f64) -> bool {
    let perto = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let desenho: Vec<&Entity> = b.entities.iter().map(|e| &**e).filter(|e| !matches!(e.kind, EntityKind::AttDef(_))).collect();
    desenho.len() == pecas.len()
        && desenho.iter().zip(pecas).all(|(e, p)| match (&e.kind, p) {
            (EntityKind::Line(l), Peca::Linha(a, b, c, d)) => {
                perto(l.a.x * m, *a) && perto(l.a.y * m, *b) && perto(l.b.x * m, *c) && perto(l.b.y * m, *d)
            }
            (EntityKind::Circle(k), Peca::Circulo(x, y, r)) => perto(k.center.x * m, *x) && perto(k.center.y * m, *y) && perto(k.radius * m, *r),
            _ => false,
        })
}

/// A caixa 4x2 em planta: a largura ao longo da parede e o quanto entra nela (m). A 4x4 tem o dobro da
/// largura e a mesma profundidade.
const LARGURA_4X2: f64 = 0.07;
const PROFUNDIDADE_CAIXA: f64 = 0.0475;
/// A haste padrão, da caixa ao símbolo (m).
const HASTE: f64 = 0.06;

/// A caixa na parede e a haste até o símbolo.
fn caixa_e_haste(haste: f64) -> Vec<Peca> {
    let (l, p) = (LARGURA_4X2 / 2.0, PROFUNDIDADE_CAIXA);
    vec![
        Peca::Linha(-l, 0.0, l, 0.0),
        Peca::Linha(-l, 0.0, -l, -p),
        Peca::Linha(l, 0.0, l, -p),
        Peca::Linha(l, -p, -l, -p),
        Peca::Linha(0.0, 0.0, 0.0, haste),
    ]
}

/// O interruptor: um círculo por seção, empilhados e tangentes na direção da haste. Vazio é simples,
/// cheio é paralelo (three-way) e meio cheio é intermediário (four-way).
fn pecas_interruptor(i: InterruptorBase) -> Vec<Peca> {
    let mut v = caixa_e_haste(CENTRO_TECLA - RAIO_TECLA);
    for k in 0..i.secoes {
        let y = CENTRO_TECLA + 2.0 * RAIO_TECLA * f64::from(k);
        match i.funcao {
            FuncaoTecla::Simples => {}
            FuncaoTecla::Paralelo => v.push(Peca::Disco(0.0, y, RAIO_TECLA)),
            FuncaoTecla::Intermediario => v.push(Peca::MeioDisco(0.0, y, RAIO_TECLA)),
        }
        v.push(Peca::Circulo(0.0, y, RAIO_TECLA));
    }
    v
}

/// A tomada: um triângulo, ou dois empilhados na dupla. Vazio é baixa, meio cheio é média, cheio é alta.
fn pecas_tomada(dupla: bool, nivel: Nivel) -> Vec<Peca> {
    let mut v = caixa_e_haste(BASE_TRIANGULO);
    let altura = TOPO_TRIANGULO - BASE_TRIANGULO;
    for k in 0..(if dupla { 2 } else { 1 }) {
        let (y0, y1) = (BASE_TRIANGULO + altura * f64::from(k), TOPO_TRIANGULO + altura * f64::from(k));
        match nivel {
            Nivel::Baixo => {}
            Nivel::Medio => v.push(Peca::Cheio([(-MEIA_BASE, y0), (0.0, y0), (0.0, y1)])),
            Nivel::Alto => v.push(Peca::Cheio([(-MEIA_BASE, y0), (MEIA_BASE, y0), (0.0, y1)])),
        }
        v.push(Peca::Linha(-MEIA_BASE, y0, MEIA_BASE, y0));
        v.push(Peca::Linha(0.0, y1, MEIA_BASE, y0));
        v.push(Peca::Linha(-MEIA_BASE, y0, 0.0, y1));
    }
    v
}

/// A definição de um bloco base: as peças e os atributos.
fn definicao_base(nome: &str, tensao: &str) -> Option<(Vec<Peca>, Vec<Att>)> {
    let interruptor = |i: InterruptorBase| {
        let tipo = match (i.funcao, i.secoes) {
            (FuncaoTecla::Simples, n) => format!("{n}S"),
            (FuncaoTecla::Paralelo, 1) => "PARALELO".to_string(),
            (FuncaoTecla::Paralelo, n) => format!("PARALELO-{n}S"),
            (FuncaoTecla::Intermediario, 1) => "INTERMEDIARIO".to_string(),
            (FuncaoTecla::Intermediario, n) => format!("INTERMEDIARIO-{n}S"),
        };
        (
            pecas_interruptor(i),
            vec![
                constante("TIPO_PONTO", "INTERRUPTOR"),
                constante("TIPO_INTERRUPTOR", &tipo),
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
    if let Some(i) = interruptor_base(nome) {
        return Some(interruptor(i));
    }
    if let Some((dupla, nivel)) = tomada_base(nome) {
        return Some((
            pecas_tomada(dupla, nivel),
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
        ));
    }
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
        "TOM-TUE" => {
            let p = pecas_tomada(false, Nivel::Alto);
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
        BLOCO_LUZ_TETO => (
            pecas_luz_teto(),
            vec![
                constante("TIPO_PONTO", "ILUM"),
                att("TAG", "", "Identificação"),
                visivel("POTENCIA", "100VA", "Potência", (0.0, 0.0)),
                visivel("COMANDO", "", "Letra do comando", (0.0, 0.0)),
                visivel("CIRCUITO", "", "Circuito", (0.0, 0.0)),
                att("TENSAO", tensao, "Tensão"),
                att("NUMERO_FASES", "1", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.118, -0.09)),
                att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
                att("FP", "0,92", "Fator de potência"),
            ],
        ),
        BLOCO_ARANDELA => (
            pecas_arandela(),
            vec![
                constante("TIPO_PONTO", "ILUM"),
                att("TAG", "", "Identificação"),
                att("POTENCIA", "60VA", "Potência"),
                visivel("COMANDO", "", "Letra do comando", (0.0, 0.0)),
                visivel("CIRCUITO", "", "Circuito", (0.0, 0.0)),
                att("TENSAO", tensao, "Tensão"),
                att("NUMERO_FASES", "1", "Número de fases"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.118, 0.03)),
                att("TIPO_CARGA", "ILUM_TUG", "Tipo de carga"),
                att("FP", "0,92", "Fator de potência"),
            ],
        ),
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
        BLOCO_CAIXA_OCT => (
            octogono(RAIO_OCTOGONO),
            vec![
                constante("TIPO_PONTO", "CAIXA DE PASSAGEM"),
                att("TAG", "", "Identificação"),
                visivel("ALTURA", "", "Altura fora da legenda", (0.08, -0.02)),
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
        Peca::Disco(x, y, r) => cheio(vec![laco_circular(*x, *y, *r, u)]),
        Peca::MeioDisco(x, y, r) => {
            // De baixo para cima pela esquerda (horário), e a volta reta pelo diâmetro.
            let vertice = |vy: f64, bulge: f64| PolyVertex { p: Vec2::new(u(*x), u(vy)), bulge, start_width: 0.0, end_width: 0.0 };
            cheio(vec![HatchLoop { vertices: vec![vertice(*y - *r, -1.0), vertice(*y + *r, 0.0)], outer: false, sources: Vec::new() }])
        }
        Peca::Arco(x, y, r, a0, a1) => {
            EntityKind::Arc(cadcraft_doc::Arc { center: Vec3::new(u(*x), u(*y), 0.0), radius: u(*r), start: a0.to_radians(), end: a1.to_radians() })
        }
    }
}

/// A escala (unidades do bloco por metro) de um ponto de luz base desenhado como agora: o raio do círculo
/// ou do arco sobre [`RAIO_LUZ`], com o comando centralizado. `None` num bloco antigo ou redesenhado.
fn escala_de_luz(b: &Block) -> Option<f64> {
    let centralizado = definicoes(b)
        .iter()
        .any(|a| a.tag.trim().eq_ignore_ascii_case("COMANDO") && a.text.halign == HAlign::Center && a.text.valign == VAlign::Middle);
    if !centralizado {
        return None;
    }
    b.entities
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::Circle(c) => Some(c.radius),
            EntityKind::Arc(a) => Some(a.radius),
            _ => None,
        })
        .map(|r| r / RAIO_LUZ)
        .filter(|k| k.is_finite() && *k > 0.0)
}

/// Um texto centralizado no compartimento `c` do símbolo, na escala `k` (unidades por metro): a altura é a
/// que cabe, nunca maior que [`TEXTO_LUZ`].
fn texto_no_compartimento(c: &Compartimento, valor: String, estilo: &str, k: f64) -> Text {
    let (_, cx, cy, w, h) = *c;
    let n = valor.chars().count().max(1) as f64;
    let altura = TEXTO_LUZ.min(h).min(w / (n * crate::tabela::LARGURA_CARACTERE));
    let p = Vec3::new(cx * k, cy * k, 0.0);
    Text {
        insert: p,
        align_pt: Some(p),
        height: altura * k,
        value: valor,
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: estilo.to_string(),
        halign: HAlign::Center,
        valign: VAlign::Middle,
    }
}

/// Põe os textos de uma inserção de ponto de luz nos compartimentos do símbolo, centralizados e do tamanho
/// que cabe, e o circuito só com o número. Outros blocos ficam como estão. Devolve se mudou algo.
fn encaixar_textos(ins: &mut Insert, bloco: &Block) -> bool {
    let (Some(comps), Some(k)) = (compartimentos_de_luz(&ins.block), escala_de_luz(bloco)) else { return false };
    let mat = ins.transform(bloco.base.xy());
    let mut mudou = false;
    for a in &mut ins.attribs {
        let Some(c) = comps.iter().find(|c| a.tag.trim().eq_ignore_ascii_case(c.0)) else { continue };
        let valor = if c.0 == "CIRCUITO" { circuito_na_planta(&a.text.value) } else { a.text.value.trim().to_string() };
        let mut kind = EntityKind::Text(texto_no_compartimento(c, valor, &a.text.style, k));
        kind.transform(&mat);
        if let EntityKind::Text(t) = kind
            && t != a.text
        {
            a.text = t;
            mudou = true;
        }
    }
    mudou
}

/// Encaixa os textos dos pontos de luz do desenho nos compartimentos do símbolo (`h`: só esse). Devolve
/// quantos mudaram.
pub fn encaixar_textos_de_luz(doc: &mut Drawing, h: Option<Handle>) -> usize {
    let alvos: Vec<Entity> = doc
        .model
        .iter()
        .filter(|e| h.is_none_or(|h| e.handle == h))
        .filter(|e| matches!(&e.kind, EntityKind::Insert(i) if compartimentos_de_luz(&i.block).is_some()))
        .map(|e| (**e).clone())
        .collect();
    let mut n = 0;
    for mut e in alvos {
        let EntityKind::Insert(ins) = &mut e.kind else { continue };
        let Some(bloco) = doc.blocks.get(&ins.block).cloned() else { continue };
        if encaixar_textos(ins, &bloco) {
            doc.model.replace(e);
            n += 1;
        }
    }
    n
}

/// O ponto de luz no teto desenhado como nas versões anteriores: um anel cheio, sem divisórias.
fn e_luz_antiga(b: &Block) -> bool {
    let desenho: Vec<&EntityKind> = b.entities.iter().map(|e| &e.kind).filter(|k| !matches!(k, EntityKind::AttDef(_))).collect();
    matches!(desenho.as_slice(), [EntityKind::Hatch(h)] if h.loops.len() == 2)
}

fn laco_circular(x: f64, y: f64, r: f64, u: &dyn Fn(f64) -> f64) -> HatchLoop {
    let vertice = |vx: f64| PolyVertex { p: Vec2::new(u(vx), u(y)), bulge: 1.0, start_width: 0.0, end_width: 0.0 };
    HatchLoop { vertices: vec![vertice(x + r), vertice(x - r)], outer: false, sources: Vec::new() }
}

/// Um círculo cheio, em unidades do desenho.
pub(crate) fn disco(x: f64, y: f64, r: f64) -> EntityKind {
    cheio(vec![laco_circular(x, y, r, &|v| v)])
}

/// Uma hachura sólida dos laços.
fn cheio(loops: Vec<HatchLoop>) -> EntityKind {
    EntityKind::Hatch(Hatch {
        pattern: "SOLID".into(),
        solid: true,
        loops,
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
    BLOCOS_BASE.iter().any(|n| !doc.blocks.contains_key(*n) && !familia::excluida(doc, n))
        || BLOCOS_ANTIGOS.iter().any(|n| doc.blocks.contains_key(*n))
        || doc.blocks.get(BLOCO_LUZ_TETO).is_some_and(|b| e_luz_antiga(b))
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
        if familia::excluida(doc, nome) {
            continue;
        }
        if let Some(existente) = doc.blocks.get(nome) {
            let antigo =
                geometria_antiga(nome).is_some_and(|p| e_desenho_antigo(existente, &p, m)) || (nome == BLOCO_LUZ_TETO && e_luz_antiga(existente));
            if !antigo {
                completar_familia(doc, nome);
                continue;
            }
        }
        criar_bloco_base(doc, nome, &tensao, m);
    }
    migrar_antigos(doc, setup, m);
    // Os pontos de luz já inseridos levam os textos para dentro do símbolo novo.
    encaixar_textos_de_luz(doc, None);
    true
}

/// Cria (ou troca) a definição de um bloco base, com a família dele.
pub(crate) fn criar_bloco_base(doc: &mut Drawing, nome: &str, tensao: &str, m: f64) {
    let Some((pecas, atributos)) = definicao_base(nome, tensao) else { return };
    let mut b = Block::new(nome);
    let u = |v: f64| v / m;
    // As primeiras peças de um ponto de parede são a caixa 4x2 e a haste: marcadas, para o módulo
    // empilhado sair sem elas.
    let caixa = if pecas_com_caixa(nome) { 5 } else { 0 };
    for (i, p) in pecas.iter().enumerate() {
        let mut e = Entity::new(doc.new_handle(), entidade_da_peca(p, &u));
        if i < caixa {
            crate::xdata::gravar(&mut e.common.xdata, CHAVE_CAIXA_2D, Some("1"));
        }
        b.entities.push(e);
    }
    let luz = compartimentos_de_luz(nome).unwrap_or(&[]);
    for a in &atributos {
        let mut def = atributo_base(a, m);
        // No ponto de luz, os textos ficam centralizados dentro das divisões do símbolo.
        if let Some(c) = luz.iter().find(|c| c.0 == a.0) {
            def.text = texto_no_compartimento(c, def.text.value.clone(), &def.text.style, 1.0 / m);
        }
        b.entities.push(Entity::new(doc.new_handle(), EntityKind::AttDef(def)));
    }
    doc.blocks.insert(nome.to_string(), Arc::new(b));
    if let Some(f) = familia::da_base(nome) {
        let _ = familia::gravar(doc, nome, Some(&f));
    }
}

/// Os blocos de fábrica de versões anteriores que tinham mais de um módulo: tomadas duplas e interruptores
/// de 2 e 3 teclas.
pub const BLOCOS_ANTIGOS: [&str; 18] = [
    "TOM-DUP-BAIXA",
    "TOM-DUP-MEDIA",
    "TOM-DUP-ALTA",
    "INT-2S",
    "INT-3S",
    "INT-PAR-2S",
    "INT-PAR-3S",
    "INT-INT-2S",
    "INT-INT-3S",
    "INT-2S-ALTO",
    "INT-3S-ALTO",
    "INT-PAR-2S-ALTO",
    "INT-PAR-3S-ALTO",
    "INT-INT-2S-ALTO",
    "INT-INT-3S-ALTO",
    "TOM-DUP-BAIXA$EMP",
    "TOM-DUP-MEDIA$EMP",
    "TOM-DUP-ALTA$EMP",
];

/// O módulo de fábrica que substitui um bloco antigo, e quantos módulos ele tinha.
fn substituto(nome: &str) -> Option<(&'static str, usize)> {
    if let Some((true, nivel)) = tomada_base(nome) {
        return Some((
            match nivel {
                Nivel::Baixo => "TOM-SIM-BAIXA",
                Nivel::Medio => "TOM-SIM-MEDIA",
                Nivel::Alto => "TOM-SIM-ALTA",
            },
            2,
        ));
    }
    let i = interruptor_base(nome).filter(|i| i.secoes > 1)?;
    let modulo = match (i.funcao, i.alto) {
        (FuncaoTecla::Simples, false) => "INT-SIM",
        (FuncaoTecla::Paralelo, false) => "INT-PAR",
        (FuncaoTecla::Intermediario, false) => "INT-INT",
        (FuncaoTecla::Simples, true) => "INT-SIM-ALTO",
        (FuncaoTecla::Paralelo, true) => "INT-PAR-ALTO",
        (FuncaoTecla::Intermediario, true) => "INT-INT-ALTO",
    };
    Some((modulo, usize::from(i.secoes)))
}

/// Tira do desenho os blocos antigos de mais de um módulo. Cada inserção vira os módulos de agora,
/// empilhados no mesmo lugar: o primeiro fica com os atributos, o XDATA e a camada; os outros, com o mesmo
/// circuito e uma letra de comando cada. Um bloco antigo sem o módulo novo no desenho (excluído pelo
/// projetista) fica. Devolve quantas inserções foram trocadas.
pub fn migrar_antigos(doc: &mut Drawing, setup: &crate::setup::Setup, metros: f64) -> usize {
    let _ = setup;
    let mut n = 0;
    for antigo in BLOCOS_ANTIGOS {
        if !doc.blocks.contains_key(antigo) {
            continue;
        }
        let Some((modulo, quantos)) = substituto(bloco_de_origem(antigo)) else { continue };
        if !doc.blocks.contains_key(modulo) {
            continue;
        }
        let insercoes: Vec<Entity> =
            doc.model.iter().filter(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block == antigo)).map(|e| (**e).clone()).collect();
        let mut ok = true;
        for e in insercoes {
            if trocar_insercao(doc, &e, modulo, quantos, metros).is_err() {
                ok = false;
                continue;
            }
            n += 1;
        }
        if ok {
            let _ = familia::gravar(doc, antigo, None);
            doc.blocks.remove(antigo);
        }
    }
    n
}

fn trocar_insercao(doc: &mut Drawing, e: &Entity, modulo: &str, quantos: usize, metros: f64) -> Result<(), String> {
    let EntityKind::Insert(ins) = &e.kind else { return Err("não é inserção".into()) };
    let p = crate::ponto::ler_ponto(doc, e.handle).ok_or("não é ponto")?;
    let novo = blocos(doc).into_iter().find(|b| b.nome == modulo).ok_or("módulo ausente")?;
    let livre = |tag: &str| novo.atributos.iter().any(|a| !a.constante && a.tag.eq_ignore_ascii_case(tag));
    let valores: Vec<(String, String)> = ins
        .attribs
        .iter()
        .filter(|a| livre(&a.tag) && !a.tag.eq_ignore_ascii_case("TIPO_INTERRUPTOR"))
        .map(|a| (a.tag.trim().to_string(), a.text.value.clone()))
        .collect();
    let letras = p.comando.as_deref().map(crate::fiacao::letras).unwrap_or_default();
    let pos = [ins.insert.x, ins.insert.y];
    let rotacao = ins.rotation.to_degrees();
    let mut primeiro = None;
    for k in 0..quantos {
        let mut v: Vec<(String, String)> = if k == 0 {
            valores.clone()
        } else {
            // Os outros módulos: o circuito do ponto antigo; a potência fica a da família.
            valores.iter().filter(|(t, _)| crate::ponto::TAGS_CIRCUITO.iter().any(|c| t.eq_ignore_ascii_case(c))).cloned().collect()
        };
        if !letras.is_empty() && livre("COMANDO") {
            v.retain(|(t, _)| !t.eq_ignore_ascii_case("COMANDO"));
            v.push(("COMANDO".into(), letras.get(k).cloned().unwrap_or_default()));
        }
        let h = inserir(doc, modulo, pos, rotacao, &v)?;
        // O XDATA (quadro, altura própria, vínculos) e a camada do ponto antigo valem para os módulos.
        if let Some(mut x) = doc.model.get(h).map(|x| (**x).clone()) {
            x.common.xdata = e.common.xdata.clone();
            x.common.layer = e.common.layer.clone();
            doc.model.replace(x);
        }
        match primeiro {
            None => primeiro = Some(h),
            Some(base) => {
                empilhar(doc, h, Some(base), metros)?;
            }
        }
    }
    doc.model.remove(e.handle);
    // Os eletrodutos gravados no ponto antigo passam ao primeiro módulo.
    if let Some(novo) = primeiro {
        crate::eletroduto::trocar_ponta(doc, e.handle, novo);
    }
    Ok(())
}

/// Os blocos base que começam pela caixa 4x2 na parede.
fn pecas_com_caixa(nome: &str) -> bool {
    tomada_base(nome).is_some() || interruptor_base(nome).is_some() || matches!(nome, "TOM-TUE" | "RJ45")
}

/// Uma família base de desenho anterior ganha os textos da legenda e da lista de materiais. O que o
/// projetista já escreveu fica.
fn completar_familia(doc: &mut Drawing, nome: &str) {
    let (Some(mut f), Some(base)) = (familia::ler(doc, nome), familia::da_base(nome)) else { return };
    if f.legenda.trim().is_empty() && f.desc_material.trim().is_empty() {
        f.legenda = base.legenda;
        f.desc_material = base.desc_material;
        let _ = familia::gravar(doc, nome, Some(&f));
    }
}

// ---------------------------------------------------------------- utilitários do editor de blocos

/// As geometrias de partida que o editor de blocos oferece: (código, nome).
///
/// Só contornos: preencher um símbolo é com a Hachura nativa do CADCraft.
pub const GEOMETRIAS: [(&str, &str); 7] = [
    ("tomada", "Triângulo de tomada"),
    ("comando", "Círculo de comando"),
    ("luminaria", "Ponto de luz no teto"),
    ("arandela", "Ponto de luz na parede"),
    ("haste", "Haste / conector padrão"),
    ("contorno4x2", "Caixa 4x2"),
    ("contorno4x4", "Caixa 4x4"),
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
        "luminaria" => pecas_luz_teto(),
        "arandela" => pecas_arandela(),
        // Em planta, a 4x4 entra na parede o mesmo que a 4x2 e tem o dobro da largura ao longo dela.
        "contorno4x2" => retangulo(-LARGURA_4X2 / 2.0, -PROFUNDIDADE_CAIXA, LARGURA_4X2 / 2.0, 0.0),
        "contorno4x4" => retangulo(-LARGURA_4X2, -PROFUNDIDADE_CAIXA, LARGURA_4X2, 0.0),
        "haste" => vec![Peca::Linha(0.0, 0.0, 0.0, HASTE)],
        _ => return Err(format!("geometria \"{forma}\" desconhecida")),
    };
    // A caixa e a haste levam o papel de base: somem quando um módulo desta família é empilhado.
    let base = matches!(forma, "contorno4x2" | "contorno4x4" | "haste");
    let u = |v: f64| v / m;
    for p in &pecas {
        let mut c = Common::default();
        if base {
            crate::xdata::gravar(&mut c.xdata, CHAVE_PAPEL, Some(PAPEL_BASE));
        }
        doc.add(&Space::Model, c, entidade_da_peca(p, &u)).map_err(|e| e.to_string())?;
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
            visivel("POTENCIA", "100VA", "Potência", (0.0, 0.0)),
            visivel("COMANDO", "", "Letra do comando", (0.0, 0.0)),
            visivel("CIRCUITO", "", "Circuito", (0.0, 0.0)),
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
            let mut def = atributo_base(a, m);
            // Luminária: os textos centralizados nas divisões do ponto de luz no teto.
            if let Some(c) = LUZ_TETO.iter().find(|c| categoria == C::Luminaria && c.0 == a.0) {
                def.text = texto_no_compartimento(c, def.text.value.clone(), &def.text.style, 1.0 / m);
            }
            doc.add(&Space::Model, Common::default(), EntityKind::AttDef(def)).map_err(|e| e.to_string())?;
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

/// Exclui um bloco do desenho, com a família dele. Com inserções, só se `com_insercoes` disser: elas saem
/// junto. Um bloco de fábrica fica marcado, e a preparação do desenho não o recria. Devolve quantas
/// inserções saíram.
pub fn excluir_bloco(doc: &mut Drawing, nome: &str, com_insercoes: bool) -> Result<usize, String> {
    let info = blocos(doc)
        .into_iter()
        .find(|b| b.nome.eq_ignore_ascii_case(nome.trim()))
        .ok_or_else(|| format!("o desenho não tem o bloco \"{}\"", nome.trim()))?;
    if info.inseridos > 0 && !com_insercoes {
        return Err(format!("o bloco \"{}\" tem {} inserção(ões) no desenho", info.nome, info.inseridos));
    }
    let hs: Vec<Handle> =
        doc.model.iter().filter(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block.eq_ignore_ascii_case(&info.nome))).map(|e| e.handle).collect();
    for h in &hs {
        doc.model.remove(*h);
    }
    familia::gravar(doc, &info.nome, None)?;
    doc.blocks.remove(&info.nome);
    if BLOCOS_BASE.contains(&info.nome.as_str()) {
        familia::marcar_excluida(doc, &info.nome, true);
    }
    Ok(hs.len())
}

/// Traz de volta os blocos de fábrica excluídos do desenho (a próxima preparação os recria).
pub fn restaurar_base(doc: &mut Drawing) {
    for n in BLOCOS_BASE {
        if familia::excluida(doc, n) {
            familia::marcar_excluida(doc, n, false);
        }
    }
}

// ---------------------------------------------------------------- módulos empilhados

/// XDATA das peças da caixa 4x2 (e da haste) de um bloco de parede.
pub const CHAVE_CAIXA_2D: &str = "wall_box";
/// O sufixo do bloco de um módulo empilhado: o mesmo símbolo, sem a caixa, com a base do símbolo na origem.
pub const SUFIXO_EMPILHADO: &str = "$EMP";
/// Até esta distância (m) do ponto base, um ponto novo de parede é empilhado na caixa dele.
pub const TOLERANCIA_EMPILHAR: f64 = 0.10;

/// O papel de uma peça no desenho de uma família (XDATA): a caixa e a haste de um ponto de parede são
/// [`PAPEL_BASE`], e somem no módulo empilhado.
pub const CHAVE_PAPEL: &str = "ROLE";
pub const PAPEL_BASE: &str = "BASE_BOX_CONNECTOR";

/// O nome do bloco de origem de um módulo empilhado.
pub fn bloco_de_origem(nome: &str) -> &str {
    nome.strip_suffix(SUFIXO_EMPILHADO).unwrap_or(nome)
}

/// A peça é a caixa ou a haste de um ponto de parede: marcada com [`PAPEL_BASE`] (qualquer família) ou com
/// [`CHAVE_CAIXA_2D`] (os blocos de fábrica).
pub fn e_peca_de_base(e: &Entity) -> bool {
    crate::xdata::ler(&e.common.xdata)
        .iter()
        .any(|(k, v)| k == CHAVE_CAIXA_2D || (k.eq_ignore_ascii_case(CHAVE_PAPEL) && v.trim().eq_ignore_ascii_case(PAPEL_BASE)))
}

/// O ponto tem caixa de parede onde outros módulos se empilham: a família diz 4x2 ou 4x4, ou o desenho do
/// bloco tem a caixa e a haste marcadas.
fn tem_caixa_de_parede(doc: &Drawing, p: &crate::ponto::PontoEletrico) -> bool {
    matches!(p.caixa, Some(familia::CaixaFisica::C4x2 | familia::CaixaFisica::C4x4))
        || doc.blocks.get(bloco_de_origem(&p.bloco)).is_some_and(|b| b.entities.iter().any(|e| e_peca_de_base(e)))
}

/// A faixa vertical (y mínimo e máximo) do desenho do bloco, sem atributos; com `sem_caixa`, sem a caixa 4x2.
fn faixa_vertical(b: &Block, sem_caixa: bool) -> Option<(f64, f64)> {
    let mut ys: Vec<f64> = Vec::new();
    for e in b.entities.iter() {
        if sem_caixa && e_peca_de_base(e) {
            continue;
        }
        match &e.kind {
            EntityKind::Line(l) => ys.extend([l.a.y, l.b.y]),
            EntityKind::Circle(c) => ys.extend([c.center.y - c.radius, c.center.y + c.radius]),
            EntityKind::Solid(s) => ys.extend(s.corners.iter().map(|c| c.y)),
            EntityKind::Arc(a) => ys.extend([a.center.y - a.radius, a.center.y + a.radius]),
            EntityKind::LwPolyline(p) => ys.extend(p.vertices.iter().map(|v| v.p.y)),
            _ => {}
        }
    }
    let ys: Vec<f64> = ys.into_iter().filter(|y| y.is_finite()).collect();
    let min = ys.iter().copied().reduce(f64::min)?;
    let max = ys.iter().copied().reduce(f64::max)?;
    Some((min, max))
}

/// O bloco do módulo empilhado de `nome`, refeito a cada empilhamento (a família pode ter sido redesenhada),
/// com a família copiada. Num bloco sem as peças da caixa marcadas, o desenho fica inteiro.
fn bloco_empilhado(doc: &mut Drawing, nome: &str) -> Result<String, String> {
    let nome = bloco_de_origem(nome).to_string();
    let novo = format!("{nome}{SUFIXO_EMPILHADO}");
    let origem = doc.blocks.get(&nome).cloned().ok_or_else(|| format!("o desenho não tem o bloco \"{nome}\""))?;
    let (base, _) = faixa_vertical(&origem, true).unwrap_or((0.0, 0.0));
    let descer = cadcraft_doc::geom::Mat3::translate(Vec2::new(0.0, -base));
    let mut b = Block::new(&novo);
    for e in origem.entities.iter() {
        if e_peca_de_base(e) {
            continue;
        }
        let mut k = e.kind.clone();
        k.transform(&descer);
        let mut x = Entity::new(doc.new_handle(), k);
        x.common = e.common.clone();
        b.entities.push(x);
    }
    doc.blocks.insert(novo.clone(), Arc::new(b));
    let f = familia::ler(doc, &nome);
    familia::gravar(doc, &novo, f.as_ref())?;
    Ok(novo)
}

/// O ponto base da caixa em que `h` cai: um ponto de parede com caixa (de fábrica ou de família própria), não
/// empilhado, a até [`TOLERANCIA_EMPILHAR`].
fn base_para(doc: &Drawing, analise: &crate::Analise, h: Handle, metros: f64) -> Option<Handle> {
    let p = analise.ponto(h)?;
    let tol = TOLERANCIA_EMPILHAR / metros;
    analise
        .pontos
        .iter()
        .filter(|o| o.handle != h && o.host.is_none() && tem_caixa_de_parede(doc, o))
        .map(|o| (o.handle, (o.posicao[0] - p.posicao[0]).hypot(o.posicao[1] - p.posicao[1])))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(o, _)| o)
}

/// Empilha o ponto `h` na caixa do ponto `base` (`None`: a caixa de parede que estiver no mesmo lugar). Na
/// planta, o módulo sai sem a caixa 4x2 e encosta no topo do último símbolo; no grafo, ele fica no ponto base.
/// Devolve o ponto base, ou `None` quando não havia onde empilhar.
pub fn empilhar(doc: &mut Drawing, h: Handle, base: Option<Handle>, metros: f64) -> Result<Option<Handle>, String> {
    let metros = if metros > 0.0 { metros } else { 1.0 };
    let analise = crate::analisar(doc);
    let p = analise.ponto(h).ok_or("o objeto não é um ponto elétrico")?.clone();
    if p.host.is_some() {
        return Ok(p.host);
    }
    let base = match base {
        Some(b) => {
            let o = analise.ponto(b).ok_or("a base não é um ponto elétrico")?;
            if b == h {
                return Err("um ponto não se empilha nele mesmo".into());
            }
            Some(o.host.unwrap_or(b))
        }
        None if tem_caixa_de_parede(doc, &p) => base_para(doc, &analise, h, metros),
        None => None,
    };
    let Some(base) = base else { return Ok(None) };
    if analise.pontos.iter().any(|o| o.host == Some(h)) {
        return Err("o ponto já é a base de outros".into());
    }
    let b = analise.ponto(base).ok_or("a base não é um ponto elétrico")?.clone();
    // O topo da pilha: o da base mais a altura de cada módulo já empilhado, na ordem em que entraram.
    let altura_do = |doc: &Drawing, nome: &str, sem_caixa: bool| doc.blocks.get(nome).and_then(|bl| faixa_vertical(bl, sem_caixa));
    let mut topo = altura_do(doc, &b.bloco, false).map_or(0.0, |(_, max)| max);
    let mut empilhados: Vec<&crate::ponto::PontoEletrico> = analise.pontos.iter().filter(|o| o.host == Some(base) && o.handle != h).collect();
    empilhados.sort_by_key(|o| o.handle.0);
    for o in empilhados {
        topo += altura_do(doc, &o.bloco, true).map_or(0.0, |(min, max)| max - min);
    }
    let variante = bloco_empilhado(doc, &p.bloco)?;
    let rotacao = match doc.model.get(base).map(|e| &e.kind) {
        Some(EntityKind::Insert(i)) => i.rotation,
        _ => 0.0,
    };
    let (s, c) = rotacao.sin_cos();
    let destino = [b.posicao[0] - s * topo, b.posicao[1] + c * topo];
    let mut e = doc.model.get(h).map(|e| (**e).clone()).ok_or("ponto não encontrado")?;
    let EntityKind::Insert(ins) = &mut e.kind else { return Err("o ponto não é um bloco".into()) };
    // A base do símbolo desce para a origem no bloco empilhado: os atributos acompanham.
    let descida = doc.blocks.get(&p.bloco).and_then(|bl| faixa_vertical(bl, true)).map_or(0.0, |(min, _)| min);
    let (so, co) = ins.rotation.sin_cos();
    let antigo = [ins.insert.x, ins.insert.y];
    let levar = |q: Vec3| {
        let (dx, dy) = (q.x - antigo[0], q.y - antigo[1]);
        let local = [dx * co + dy * so, -dx * so + dy * co - descida];
        Vec3::new(destino[0] + local[0] * c - local[1] * s, destino[1] + local[0] * s + local[1] * c, q.z)
    };
    for a in &mut ins.attribs {
        a.text.insert = levar(a.text.insert);
        a.text.align_pt = a.text.align_pt.map(levar);
        a.text.rotation += rotacao - ins.rotation;
    }
    ins.block = variante;
    ins.insert = Vec3::new(destino[0], destino[1], 0.0);
    ins.rotation = rotacao;
    crate::xdata::gravar(&mut e.common.xdata, crate::ponto::CHAVE_HOST, Some(&base.hex()));
    doc.model.replace(e);
    Ok(Some(base))
}
