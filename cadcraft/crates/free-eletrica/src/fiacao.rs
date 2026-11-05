// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Fiação: quais condutores de cada circuito passam em cada eletroduto, e as tags de fiação.
//!
//! Regras, por circuito, sobre os caminhos do grafo estrito:
//! - fase: do quadro às tomadas, às luminárias sem interruptor e ao primeiro interruptor de cada
//!   comando (o mais perto do quadro);
//! - neutro e proteção: do quadro às cargas, conforme o esquema do circuito;
//! - retorno: do último interruptor do comando às luminárias com a letra dele;
//! - retornos paralelos (`a'` e `a"`): entre os interruptores de um mesmo comando (three-way e
//!   four-way).
//!
//! **Tags não destrutivas:** cada eletroduto tem uma tag, um bloco próprio
//! (`ELE_TAG_FIACAO_<handle>`) inserido no meio dele. Regenerar troca o conteúdo do bloco e mantém
//! o deslocamento que o projetista deu à tag: basta movê-la com MOVE. Afastada do eletroduto, a
//! tag ganha uma linha de chamada. O eletroduto de cada trecho é o menor do tipo do projeto que
//! comporta os condutores com a ocupação máxima da NBR 5410 (6.2.11.1.6), salvo se travado à mão.
//!
//! As marcas são blocos (`FIACAO-FASE`, `FIACAO-NEUTRO`, `FIACAO-RETORNO`, `FIACAO-PROTECAO`)
//! perpendiculares ao trecho, criados com um desenho padrão quando o desenho não os tem; o
//! projetista pode redesenhá-los. Tudo fica na camada `ELE-FIACAO` (cor 7).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use cadcraft_doc::color::Color;
use cadcraft_doc::geom::Vec3;
use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, HAlign, Handle, Insert, Line, Lineweight, Space, Text, VAlign};
use serde::Serialize;

use crate::Analise;
use crate::nbr5410::Isolacao;
use crate::ponto::{PontoEletrico, TipoPonto, br};
use crate::quadro;
use crate::rota::{self, Instalacao, Rede};
use crate::setup::{Setup, TamanhoEletroduto};
use crate::xdata;

/// Camada das tags de fiação.
pub const CAMADA: &str = "ELE-FIACAO";
/// Chave do XDATA das marcas soltas da versão 1 (`FIACAO=<handle do quadro>`): só para apagá-las.
const CHAVE_ANTIGA: &str = "FIACAO";
/// XDATA da tag: o eletroduto dela e o meio dele na última regeneração.
pub const CHAVE_PAI: &str = "parent_conduit_handle";
const CHAVE_OFFSET_X: &str = "user_offset_x";
const CHAVE_OFFSET_Y: &str = "user_offset_y";
const CHAVE_ANCORA_X: &str = "anchor_x";
const CHAVE_ANCORA_Y: &str = "anchor_y";
/// Prefixo do bloco de cada tag.
pub const PREFIXO_TAG: &str = "ELE_TAG_FIACAO_";
/// Quantos caracteres de `passing_wires` cabem num grupo 1000 do XDATA, com a chave.
const FIOS_POR_GRUPO: usize = 200;

/// XDATA das linhas dos blocos de marca criados pelo Free Elétrica: `FIACAO_BLOCO=<versão>`. Um
/// bloco de versão anterior é trocado pelo desenho atual; um sem a marca (redesenhado pelo
/// projetista) fica.
const CHAVE_BLOCO: &str = "FIACAO_BLOCO";
const VERSAO_BLOCOS: &str = "2";

/// O bloco no desenho é um desenho padrão de versão anterior do Free Elétrica.
fn bloco_padrao_antigo(b: &Block, tipo: TipoCondutor, m: f64) -> bool {
    let versoes: Vec<String> =
        b.entities.iter().filter_map(|e| xdata::ler(&e.common.xdata).into_iter().find(|(k, _)| k == CHAVE_BLOCO).map(|(_, v)| v)).collect();
    if !versoes.is_empty() {
        return versoes.iter().any(|v| v != VERSAO_BLOCOS);
    }
    // Sem marca: os blocos da primeira versão, reconhecidos pela geometria.
    let linhas: Vec<((f64, f64), (f64, f64))> = b
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Line(l) => Some(((l.a.x * m, l.a.y * m), (l.b.x * m, l.b.y * m))),
            _ => None,
        })
        .collect();
    let perto = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
    linhas.len() == b.entities.len()
        && tipo
            .tracos_antigos()
            .iter()
            .any(|antigo| antigo.len() == linhas.len() && antigo.iter().zip(&linhas).all(|(x, y)| perto(x.0, y.0) && perto(x.1, y.1)))
}

/// Distância entre marcas no trecho e altura dos textos, em metros.
const PASSO: f64 = 0.04;
const ENTRE_CIRCUITOS: f64 = 0.05;
pub const ALTURA_TEXTO: f64 = 0.05;
/// O diâmetro do eletroduto é escrito menor que o circuito e a seção.
pub const ALTURA_DIAMETRO: f64 = 0.035;
/// Folga entre as marcas e os textos, e entre a seção e o diâmetro.
const FOLGA: f64 = 0.015;
/// Meia altura das marcas: elas vão de -0,05 a 0,05 m em torno do eletroduto.
const MEIA_MARCA: f64 = 0.05;
/// Centro dos textos de cima (circuito) e de baixo (seção), a partir do eixo do eletroduto.
const AFASTAMENTO_TEXTO: f64 = MEIA_MARCA + FOLGA + ALTURA_TEXTO / 2.0;
/// Centro do diâmetro: abaixo da seção, com folga.
const AFASTAMENTO_DIAMETRO: f64 = AFASTAMENTO_TEXTO + ALTURA_TEXTO / 2.0 + FOLGA + ALTURA_DIAMETRO / 2.0;
/// A tag afastada mais que isto do eletroduto (m) ganha linha de chamada.
const OFFSET_COM_CHAMADA: f64 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoCondutor {
    Fase,
    Neutro,
    Retorno,
    Protecao,
}

impl TipoCondutor {
    pub const TODOS: [TipoCondutor; 4] = [Self::Fase, Self::Neutro, Self::Retorno, Self::Protecao];

    /// O bloco da marca.
    pub fn bloco(self) -> &'static str {
        match self {
            Self::Fase => "FIACAO-FASE",
            Self::Neutro => "FIACAO-NEUTRO",
            Self::Retorno => "FIACAO-RETORNO",
            Self::Protecao => "FIACAO-PROTECAO",
        }
    }

    /// O desenho padrão da marca, em metros, com o eletroduto no eixo x: fase, traço que
    /// atravessa o eletroduto; neutro, o traço com uma bandeira para a esquerda no topo;
    /// retorno, um traço curto só acima do eletroduto; terra (proteção), o traço com uma barra
    /// no topo, em T.
    fn tracos(self) -> Vec<((f64, f64), (f64, f64))> {
        const H: f64 = 0.05;
        let haste = ((0.0, -H), (0.0, H));
        match self {
            Self::Fase => vec![haste],
            Self::Neutro => vec![haste, ((-0.02, H), (0.0, H))],
            Self::Retorno => vec![((0.0, 0.0), (0.0, H * 0.6))],
            Self::Protecao => vec![haste, ((-0.02, H), (0.02, H))],
        }
    }

    /// Os desenhos padrão das versões anteriores, para trocar por este quando o bloco no
    /// desenho ainda é um deles.
    fn tracos_antigos(self) -> Vec<Vec<((f64, f64), (f64, f64))>> {
        const H: f64 = 0.05;
        let haste = ((0.0, -H), (0.0, H));
        match self {
            Self::Fase => vec![vec![haste, ((-0.015, H - 0.015), (0.0, H))]],
            Self::Neutro => vec![vec![((0.0, 0.01), (0.0, H))]],
            Self::Retorno => vec![vec![haste, ((-0.025, H), (0.0, H))]],
            Self::Protecao => vec![],
        }
    }
}

/// Os condutores de um circuito num trecho.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Condutores {
    pub circuito: String,
    /// Fases (0 a 3).
    pub fases: u8,
    pub neutro: bool,
    pub protecao: bool,
    /// Rótulos dos retornos: a letra do comando, e `a'` e `a"` nos retornos paralelos.
    pub retornos: BTreeSet<String>,
    /// Seção do circuito (mm²), a escolhida no quadro de cargas.
    pub secao: Option<f64>,
    /// Isolação dos condutores do circuito.
    pub isolacao: Isolacao,
    /// Tipo de cabo do circuito (entra no diâmetro externo).
    pub tipo_cabo: crate::nbr5410::TipoCabo,
}

impl Condutores {
    /// As marcas, na ordem em que são desenhadas: fases, neutro, retornos, proteção.
    pub fn marcas(&self) -> Vec<(TipoCondutor, Option<&str>)> {
        let mut v: Vec<(TipoCondutor, Option<&str>)> = (0..self.fases).map(|_| (TipoCondutor::Fase, None)).collect();
        if self.neutro {
            v.push((TipoCondutor::Neutro, None));
        }
        v.extend(self.retornos.iter().map(|l| (TipoCondutor::Retorno, Some(l.as_str()))));
        if self.protecao {
            v.push((TipoCondutor::Protecao, None));
        }
        v
    }

    fn vazio(&self) -> bool {
        self.fases == 0 && !self.neutro && !self.protecao && self.retornos.is_empty()
    }

    /// Os condutores no formato de `passing_wires`: `1:F,N,PE,R_a:2,5`.
    fn serializado(&self) -> String {
        let mut papeis: Vec<String> = (0..self.fases).map(|_| "F".to_string()).collect();
        if self.neutro {
            papeis.push("N".into());
        }
        papeis.extend(self.retornos.iter().map(|r| format!("R_{r}")));
        if self.protecao {
            papeis.push("PE".into());
        }
        format!("{}:{}:{}", self.circuito, papeis.join(","), self.secao.map_or("?".to_string(), br))
    }
}

/// Um eletroduto com os condutores que passam nele.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trecho {
    /// O eletroduto (a entidade).
    #[serde(serialize_with = "hex")]
    pub handle: Handle,
    /// Os blocos das pontas.
    #[serde(serialize_with = "hex")]
    pub de: Handle,
    #[serde(serialize_with = "hex")]
    pub para: Handle,
    pub instalacao: Instalacao,
    pub tipo: rota::TipoEletroduto,
    /// O perfil de eletroduto do trecho: dele vem a tabela de diâmetros.
    pub perfil: Option<String>,
    /// Comprimento com subidas e descidas (m).
    pub comprimento: f64,
    /// Meio do eletroduto e o ângulo dele ali.
    pub meio: [f64; 2],
    pub angulo: f64,
    pub circuitos: Vec<Condutores>,
    /// O eletroduto do trecho. `None`: nenhum tamanho do tipo comporta (ou falta o diâmetro de
    /// alguma seção nas configurações).
    pub eletroduto: Option<TamanhoEletroduto>,
    /// Ocupação da seção interna do eletroduto (0 a 1).
    pub ocupacao: Option<f64>,
    /// O diâmetro foi travado à mão.
    pub diametro_travado: bool,
    /// Os condutores de proteção que o compartilhamento tirou do trecho: (seção, tipo de cabo).
    pub pe_removidos: Vec<(f64, crate::nbr5410::TipoCabo)>,
}

impl Trecho {
    /// Quantos condutores passam no trecho.
    pub fn condutores(&self) -> usize {
        self.circuitos.iter().map(|k| k.marcas().len()).sum()
    }

    /// A ocupação passa do limite da NBR 5410 para esse número de condutores.
    pub fn ocupacao_excedida(&self) -> bool {
        self.ocupacao.is_some_and(|o| o > crate::setup::ocupacao_maxima(self.condutores()) + 1e-9)
    }
}

/// Os diâmetros externos dos condutores do trecho.
fn diametros(t: &Trecho, setup: &Setup, avisos: &mut Vec<String>) -> Option<Vec<f64>> {
    let mut v = Vec::new();
    for k in &t.circuitos {
        let Some(secao) = k.secao else {
            avisos.push(format!("circuito {}: sem seção, eletroduto não dimensionado", k.circuito));
            return None;
        };
        let Some(d) = setup.diametro_do_cabo(secao, k.tipo_cabo) else {
            avisos.push(format!("sem diâmetro externo para {} mm² ({}) nas configurações", br(secao), k.tipo_cabo.abreviado()));
            return None;
        };
        v.extend(std::iter::repeat_n(d, k.marcas().len()));
    }
    Some(v)
}

/// Num trecho com vários circuitos, fica um só condutor de proteção, o de maior seção entre eles, no
/// mesmo eletroduto das fases (NBR 5410, 6.4.3.1). Cabe ao projetista conferir a seção resultante.
fn compartilhar_protecao(t: &mut Trecho) {
    let maior = t
        .circuitos
        .iter()
        .enumerate()
        .filter(|(_, k)| k.protecao)
        .max_by(|a, b| a.1.secao.unwrap_or(0.0).total_cmp(&b.1.secao.unwrap_or(0.0)))
        .map(|(i, _)| i);
    for (i, k) in t.circuitos.iter_mut().enumerate() {
        if k.protecao && Some(i) != maior {
            k.protecao = false;
            if let Some(s) = k.secao {
                t.pe_removidos.push((s, k.tipo_cabo));
            }
        }
    }
}

/// Escolhe o eletroduto do trecho pela ocupação dos condutores; um diâmetro travado à mão só
/// tem a ocupação conferida.
fn dimensionar_eletroduto(t: &mut Trecho, travado: Option<f64>, setup: &Setup, avisos: &mut Vec<String>) {
    if setup.compartilhar_pe {
        compartilhar_protecao(t);
    }
    t.eletroduto = None;
    t.ocupacao = None;
    t.diametro_travado = false;
    let tamanho = travado.and_then(|n| setup.tamanhos_do_trecho(t.perfil.as_deref(), t.tipo).into_iter().find(|e| (e.nominal - n).abs() < 1e-6));
    if let (Some(n), None) = (travado, &tamanho) {
        avisos.push(format!("diâmetro travado Ø{} não existe no tipo {}", br(n), t.tipo.abreviado()));
    }
    if let Some(e) = &tamanho {
        t.eletroduto = Some(e.clone());
        t.diametro_travado = true;
    }
    if t.circuitos.is_empty() {
        return;
    }
    let Some(d) = diametros(t, setup, avisos) else { return };
    if let Some(e) = &tamanho {
        t.ocupacao = setup.ocupacao_em(&d, e);
        if t.ocupacao_excedida() {
            avisos.push(format!("eletroduto Ø{} travado à mão: ocupação acima do limite da NBR 5410", br(e.nominal)));
        }
        return;
    }
    match setup.eletroduto_do_trecho(&d, t.perfil.as_deref(), t.tipo) {
        Some((e, oc)) => {
            t.eletroduto = Some(e);
            t.ocupacao = Some(oc);
        }
        None => avisos.push(format!("{} condutores num trecho: nenhum eletroduto {} comporta", d.len(), t.tipo.abreviado())),
    }
}

/// O diâmetro travado à mão de um eletroduto.
pub fn diametro_travado(doc: &Drawing, h: Handle) -> Option<f64> {
    let xd = xdata::ler(&doc.model.get(h)?.common.xdata);
    let valor = |k: &str| xd.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone());
    if valor(rota::CHAVE_TRAVA_DIAMETRO).as_deref() != Some("1") {
        return None;
    }
    valor(rota::CHAVE_DIAMETRO).and_then(|v| crate::ponto::numero(&v))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fiacao {
    #[serde(serialize_with = "hex")]
    pub quadro: Handle,
    pub tag: String,
    /// Isolação dos condutores do quadro.
    pub isolacao: Isolacao,
    pub trechos: Vec<Trecho>,
    /// Metros por unidade do desenho.
    pub metros_por_unidade: f64,
    pub avisos: Vec<String>,
}

fn hex<S: serde::Serializer>(h: &Handle, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&h.hex())
}

/// As letras de um COMANDO: "a", "ab", "a,b" ou "a b".
pub fn letras(comando: &str) -> Vec<String> {
    comando.chars().filter(|c| c.is_alphabetic()).map(|c| c.to_lowercase().to_string()).collect()
}

fn trecho_de(rede: &Rede, e: usize, circuitos: Vec<Condutores>) -> Option<Trecho> {
    let a = rede.arestas.get(e)?;
    Some(Trecho {
        handle: a.eletroduto,
        de: rede.nos.get(a.de)?.handle,
        para: rede.nos.get(a.para)?.handle,
        instalacao: a.instalacao,
        tipo: a.tipo,
        perfil: a.perfil.clone(),
        comprimento: a.comprimento,
        meio: a.meio,
        angulo: a.angulo,
        circuitos,
        eletroduto: None,
        ocupacao: None,
        diametro_travado: false,
        pe_removidos: Vec::new(),
    })
}

/// Calcula os condutores em cada eletroduto dos circuitos do quadro.
pub fn calcular(doc: &Drawing, analise: &Analise, setup: &Setup, quadro: Handle) -> Result<Fiacao, String> {
    let resumo = quadro::rotear(doc, analise, setup, quadro)?;
    let q = analise.ponto(quadro).ok_or("quadro não encontrado")?;
    let rede = Rede::montar(doc, setup, &analise.pontos, resumo.metros_por_unidade);
    let no_q = rede.no_de(quadro).filter(|_| rede.conectado(quadro)).ok_or("nenhum eletroduto liga o quadro: não há onde marcar a fiação")?;
    let arvore_q = rede.arvore(no_q);
    let mut avisos = Vec::new();

    // aresta -> circuito -> condutores
    let mut por_aresta: BTreeMap<usize, BTreeMap<usize, Condutores>> = BTreeMap::new();
    for (ic, linha) in resumo.linhas.iter().enumerate() {
        let c = &linha.circuito;
        let membros: Vec<&PontoEletrico> = analise.pontos.iter().filter(|p| c.pontos.contains(&p.handle)).collect();
        let novo = || Condutores {
            circuito: c.id.clone(),
            secao: linha.escolha.secao,
            isolacao: linha.isolacao,
            tipo_cabo: linha.tipo_cabo,
            ..Condutores::default()
        };
        let mut marcar = |arestas: &[usize], f: &dyn Fn(&mut Condutores)| {
            for e in arestas {
                f(por_aresta.entry(*e).or_default().entry(ic).or_insert_with(novo));
            }
        };
        let caminho_do_quadro = |p: &PontoEletrico| rede.no_de(p.handle).and_then(|n| arvore_q.caminho(n));
        let esquema = linha.esquema;
        let fases = esquema.fases();
        // Os interruptores de cada letra, do mais perto ao mais longe do quadro.
        let mut por_letra: BTreeMap<String, Vec<&PontoEletrico>> = BTreeMap::new();
        for s in membros.iter().filter(|p| p.tipo == TipoPonto::Interruptor) {
            for l in s.comando.as_deref().map(letras).unwrap_or_default() {
                por_letra.entry(l).or_default().push(s);
            }
        }
        let distancia = |p: &PontoEletrico| rede.no_de(p.handle).and_then(|n| arvore_q.distancia.get(n).copied()).unwrap_or(f64::INFINITY);
        for lista in por_letra.values_mut() {
            lista.sort_by(|a, b| distancia(a).total_cmp(&distancia(b)));
        }
        // Só o primeiro interruptor de um comando recebe a fase; um interruptor sem letra também.
        let recebe_fase = |s: &PontoEletrico| {
            let ls = s.comando.as_deref().map(letras).unwrap_or_default();
            ls.is_empty() || ls.iter().any(|l| por_letra.get(l).and_then(|v| v.first()).is_some_and(|p| p.handle == s.handle))
        };
        for p in &membros {
            if matches!(p.tipo, TipoPonto::Caixa | TipoPonto::Quadro) {
                continue;
            }
            let Some(caminho) = caminho_do_quadro(p) else {
                avisos.push(format!("{}: sem caminho de eletrodutos até o quadro, fiação não marcada", p.rotulo()));
                continue;
            };
            let comandada = p.tipo == TipoPonto::Iluminacao
                && fases < 2
                && p.comando.as_deref().is_some_and(|cmd| letras(cmd).iter().any(|l| por_letra.contains_key(l)));
            match p.tipo {
                TipoPonto::Interruptor => {
                    if recebe_fase(p) {
                        marcar(&caminho, &|k: &mut Condutores| k.fases = k.fases.max(1));
                    }
                }
                _ if comandada => marcar(&caminho, &|k: &mut Condutores| {
                    k.neutro |= esquema.neutro();
                    k.protecao |= esquema.protecao();
                }),
                _ => marcar(&caminho, &|k: &mut Condutores| {
                    k.fases = k.fases.max(fases);
                    k.neutro |= esquema.neutro();
                    k.protecao |= esquema.protecao();
                }),
            }
        }
        for (letra, lista) in &por_letra {
            // Retornos paralelos entre os interruptores do comando.
            for par in lista.windows(2) {
                let caminho = rede.no_de(par[0].handle).zip(rede.no_de(par[1].handle)).and_then(|(a, b)| rede.arvore(a).caminho(b));
                match caminho {
                    Some(cam) => marcar(&cam, &|k: &mut Condutores| {
                        k.retornos.insert(format!("{letra}'"));
                        k.retornos.insert(format!("{letra}\""));
                    }),
                    None => avisos.push(format!("{}: sem caminho de eletrodutos até o outro interruptor do comando {letra}", par[1].rotulo())),
                }
            }
            // Retorno do último interruptor do comando às luminárias com a letra dele.
            let Some(no_s) = lista.last().and_then(|s| rede.no_de(s.handle)) else { continue };
            let arvore_s = rede.arvore(no_s);
            for lum in
                membros.iter().filter(|p| p.tipo == TipoPonto::Iluminacao && p.comando.as_deref().is_some_and(|cmd| letras(cmd).contains(letra)))
            {
                match rede.no_de(lum.handle).and_then(|n| arvore_s.caminho(n)) {
                    Some(cam) => marcar(&cam, &|k: &mut Condutores| {
                        k.retornos.insert(letra.clone());
                    }),
                    None => avisos.push(format!("{}: sem caminho de eletroduto até o interruptor do comando {letra}", lum.rotulo())),
                }
            }
        }
    }

    let mut trechos: Vec<Trecho> = por_aresta
        .into_iter()
        .filter_map(|(e, circuitos)| {
            let circuitos: Vec<Condutores> = circuitos.into_values().filter(|k| !k.vazio()).collect();
            if circuitos.is_empty() { None } else { trecho_de(&rede, e, circuitos) }
        })
        .collect();
    for t in &mut trechos {
        dimensionar_eletroduto(t, diametro_travado(doc, t.handle), setup, &mut avisos);
    }
    avisos.sort();
    avisos.dedup();
    Ok(Fiacao { quadro, tag: q.rotulo(), isolacao: resumo.isolacao, trechos, metros_por_unidade: resumo.metros_por_unidade, avisos })
}

/// Todos os eletrodutos do desenho com os condutores de todos os quadros que passam neles. Os
/// sem fiação também entram, sem circuitos; os soltos vêm à parte.
pub fn todos_os_trechos(doc: &Drawing, analise: &Analise, setup: &Setup) -> (Vec<Trecho>, Vec<rota::Solto>, Vec<String>) {
    let (metros, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    let rede = Rede::montar(doc, setup, &analise.pontos, metros);
    let mut mapa: BTreeMap<u64, Trecho> = BTreeMap::new();
    for i in 0..rede.arestas.len() {
        if let Some(mut t) = trecho_de(&rede, i, Vec::new()) {
            let travado = diametro_travado(doc, t.handle);
            dimensionar_eletroduto(&mut t, travado, setup, &mut Vec::new());
            mapa.insert(t.handle.0, t);
        }
    }
    let mut avisos = Vec::new();
    for q in analise.quadros() {
        if !rede.conectado(q.handle) {
            continue;
        }
        match calcular(doc, analise, setup, q.handle) {
            Ok(f) => {
                avisos.extend(f.avisos);
                for t in f.trechos {
                    match mapa.get_mut(&t.handle.0) {
                        // Com outro quadro no mesmo eletroduto, os condutores somam.
                        Some(m) if !m.circuitos.is_empty() => {
                            m.circuitos.extend(t.circuitos);
                            dimensionar_eletroduto(m, diametro_travado(doc, t.handle), setup, &mut avisos);
                        }
                        _ => {
                            mapa.insert(t.handle.0, t);
                        }
                    }
                }
            }
            Err(e) => avisos.push(e),
        }
    }
    avisos.sort();
    avisos.dedup();
    (mapa.into_values().collect(), rede.soltos, avisos)
}

// ---------------------------------------------------------------- tags

/// As tags de fiação do desenho: (tag, eletroduto).
pub fn tags(doc: &Drawing) -> Vec<(Handle, Handle)> {
    doc.model
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Insert(_)))
        .filter_map(|e| {
            let pai = xdata::ler(&e.common.xdata).into_iter().find(|(k, _)| k.eq_ignore_ascii_case(CHAVE_PAI))?.1;
            Some((e.handle, Handle::parse_hex(&pai)?))
        })
        .collect()
}

/// Apaga as marcas soltas da versão 1 (cada marca e cada texto como entidade).
fn apagar_marcas_antigas(doc: &mut Drawing) -> usize {
    let hs: Vec<Handle> = doc
        .model
        .iter()
        .filter(|e| xdata::ler(&e.common.xdata).iter().any(|(k, _)| k.eq_ignore_ascii_case(CHAVE_ANTIGA)))
        .map(|e| e.handle)
        .collect();
    hs.iter().filter(|h| doc.model.remove(**h).is_some()).count()
}

/// Apaga tags: as dos eletrodutos dados, ou todas. Devolve quantas apagou.
pub fn limpar(doc: &mut Drawing, eletrodutos: Option<&[Handle]>) -> usize {
    let mut n = apagar_marcas_antigas(doc);
    for (tag, pai) in tags(doc) {
        if eletrodutos.is_none_or(|l| l.contains(&pai)) && doc.model.remove(tag).is_some() {
            doc.blocks.remove(&nome_do_bloco(pai));
            n += 1;
            if let Some(mut e) = doc.model.get(pai).map(|e| (**e).clone()) {
                xdata::gravar(&mut e.common.xdata, rota::CHAVE_TAG, None);
                doc.model.replace(e);
            }
        }
    }
    n
}

fn nome_do_bloco(eletroduto: Handle) -> String {
    format!("{PREFIXO_TAG}{}", eletroduto.hex())
}

/// Cria os blocos das marcas que o desenho ainda não tem, com o desenho padrão
/// ([`TipoCondutor`]), e a camada `ELE-FIACAO` (cor 7, traço 0,35 mm). `m` = metros por unidade.
pub fn garantir_blocos(doc: &mut Drawing, m: f64) {
    let u = |v: f64| v / m;
    for tipo in TipoCondutor::TODOS {
        if let Some(b) = doc.blocks.get(tipo.bloco())
            && !bloco_padrao_antigo(b, tipo, m)
        {
            // O desenho atual, ou um bloco redesenhado pelo projetista: fica.
            continue;
        }
        let mut b = Block::new(tipo.bloco());
        for (a, c) in tipo.tracos() {
            let h = doc.new_handle();
            let linha = Line { a: Vec3::new(u(a.0), u(a.1), 0.0), b: Vec3::new(u(c.0), u(c.1), 0.0) };
            let mut e = Entity::new(h, EntityKind::Line(linha));
            xdata::gravar(&mut e.common.xdata, CHAVE_BLOCO, Some(VERSAO_BLOCOS));
            b.entities.push(e);
        }
        doc.blocks.insert(tipo.bloco().to_string(), Arc::new(b));
    }
    let nova = doc.ensure_layer(CAMADA);
    if let Some(l) = doc.layer_mut(CAMADA) {
        // Camada nova, ou a das versões antigas (vermelha): cor 7 e traço mais grosso.
        if nova || l.color == Color::Index(1) {
            l.color = Color::Index(7);
            l.lineweight = Lineweight::Mm100(35);
        }
    }
}

fn texto(valor: String, em: [f64; 2], altura: f64) -> EntityKind {
    let p = Vec3::new(em[0], em[1], 0.0);
    EntityKind::Text(Text {
        insert: p,
        align_pt: Some(p),
        height: altura,
        value: valor,
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign: HAlign::Center,
        valign: VAlign::Middle,
    })
}

/// Largura estimada de um texto (m) na fonte Standard: 0,8 da altura por caractere (`%%c`, o
/// símbolo de diâmetro, conta como um).
pub fn largura_texto(texto: &str, altura: f64) -> f64 {
    texto.replace("%%c", "Ø").chars().count() as f64 * 0.8 * altura
}

/// O que um grupo de marcas ocupa ao longo do trecho (m), a partir da primeira marca: as marcas,
/// o número do circuito sobre a primeira, as letras sobre os retornos e a seção centrada embaixo.
fn faixa_do_grupo(k: &Condutores, grupo: &[(TipoCondutor, Option<&str>)], setup: &Setup) -> (f64, f64) {
    let vao = grupo.len().saturating_sub(1) as f64 * PASSO;
    let (mut lo, mut hi) = (0.0f64, vao);
    let mut cobrir = |centro: f64, texto: &str| {
        let meia = largura_texto(texto, ALTURA_TEXTO) / 2.0;
        lo = lo.min(centro - meia);
        hi = hi.max(centro + meia);
    };
    for (i, (_, letra)) in grupo.iter().enumerate() {
        let x = i as f64 * PASSO;
        match (i, letra) {
            (0, Some(l)) => cobrir(x, &format!("{}{l}", k.circuito)),
            (0, None) => cobrir(x, &k.circuito),
            (_, Some(l)) => cobrir(x, l),
            (_, None) => {}
        }
    }
    if let Some(x) = k.secao.filter(|x| !setup.omitida(*x)) {
        cobrir(vao / 2.0, &format!("#{}", br(x)));
    }
    (lo, hi)
}

/// O conteúdo do bloco de uma tag, em coordenadas locais: o eixo x segue o eletroduto. Sem
/// circuitos, um aviso no lugar da fiação. `chamada`: o ponto do eletroduto, em coordenadas
/// locais, quando a tag foi afastada dele.
fn bloco_da_tag(
    doc: &mut Drawing,
    nome: &str,
    circuitos: &[Condutores],
    diametro: Option<f64>,
    chamada: Option<[f64; 2]>,
    m: f64,
    setup: &Setup,
) -> Block {
    let mut b = Block::new(nome);
    let (passo, entre, acima) = (PASSO / m, ENTRE_CIRCUITOS / m, AFASTAMENTO_TEXTO / m);
    let grupos: Vec<Vec<(TipoCondutor, Option<&str>)>> = circuitos.iter().map(Condutores::marcas).collect();
    // Cada grupo ocupa a largura das marcas ou dos textos dele (o que for maior), para os textos
    // de circuitos vizinhos não ficarem um sobre o outro.
    let faixas: Vec<(f64, f64)> = circuitos.iter().zip(&grupos).map(|(k, g)| faixa_do_grupo(k, g, setup)).collect();
    let largura: f64 = faixas.iter().map(|(lo, hi)| (hi - lo) / m).sum::<f64>() + faixas.len().saturating_sub(1) as f64 * entre;
    let mut borda = -largura / 2.0;
    let mut pecas: Vec<EntityKind> = Vec::new();
    for ((k, grupo), (lo, hi)) in circuitos.iter().zip(&grupos).zip(&faixas) {
        // A primeira marca fica a `-lo` da borda esquerda do grupo.
        let mut s = borda - lo / m;
        borda += (hi - lo) / m + entre;
        let inicio = s;
        for (i, (tipo, letra)) in grupo.iter().enumerate() {
            pecas.push(EntityKind::Insert(Insert {
                block: tipo.bloco().into(),
                insert: Vec3::new(s, 0.0, 0.0),
                scale: Vec3::new(1.0, 1.0, 1.0),
                rotation: 0.0,
                attribs: Vec::new(),
                cols: 1,
                rows: 1,
                col_spacing: 0.0,
                row_spacing: 0.0,
            }));
            // Em cima: o número do circuito sobre a primeira marca e a letra sobre cada
            // retorno (lado a lado leem "1b").
            let rotulo = match (i, letra) {
                (0, Some(l)) => Some(format!("{}{l}", k.circuito)),
                (0, None) => Some(k.circuito.clone()),
                (_, Some(l)) => Some(l.to_string()),
                (_, None) => None,
            };
            if let Some(r) = rotulo {
                pecas.push(texto(r, [s, acima], ALTURA_TEXTO / m));
            }
            s += passo;
        }
        // Embaixo do grupo: a seção, quando não omitida.
        if let Some(x) = k.secao.filter(|x| !setup.omitida(*x)) {
            pecas.push(texto(format!("#{}", br(x)), [(inicio + s - passo) / 2.0, -acima], ALTURA_TEXTO / m));
        }
    }
    if circuitos.is_empty() {
        pecas.push(texto("?".into(), [0.0, acima], ALTURA_TEXTO / m));
    }
    // Na linha de baixo, centrado sob toda a simbologia: o diâmetro do eletroduto do trecho.
    let d = diametro.map_or_else(|| "%%c?".to_string(), |n| format!("%%c{}", br(n)));
    pecas.push(texto(d, [0.0, -AFASTAMENTO_DIAMETRO / m], ALTURA_DIAMETRO / m));
    if let Some(c) = chamada {
        pecas.push(EntityKind::Line(Line { a: Vec3::new(0.0, 0.0, 0.0), b: Vec3::new(c[0], c[1], 0.0) }));
    }
    for k in pecas {
        b.entities.push(Entity::new(doc.new_handle(), k));
    }
    b
}

/// O ângulo para o texto ler direito: da esquerda para a direita, ou de baixo para cima.
pub fn angulo_de_leitura(a: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let r = a.rem_euclid(2.0 * pi);
    if r > 3.0 * pi / 2.0 + 1e-9 {
        r - 2.0 * pi
    } else if r > pi / 2.0 + 1e-9 {
        r - pi
    } else {
        r
    }
}

fn numero_do_xdata(xd: &[(String, String)], k: &str) -> Option<f64> {
    xd.iter().find(|(c, _)| c.eq_ignore_ascii_case(k)).and_then(|(_, v)| v.parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// Grava no eletroduto o que o roteador calculou: as pontas, o diâmetro e os condutores que passam.
fn gravar_no_eletroduto(doc: &mut Drawing, t: &Trecho, tag: Option<Handle>) {
    let Some(mut e) = doc.model.get(t.handle).map(|e| (**e).clone()) else { return };
    let x = &mut e.common.xdata;
    xdata::gravar(x, rota::CHAVE_ESQUEMA, Some(rota::VERSAO_ESQUEMA));
    xdata::gravar(x, rota::CHAVE_DE, Some(&t.de.hex()));
    xdata::gravar(x, rota::CHAVE_PARA, Some(&t.para.hex()));
    xdata::gravar(x, rota::CHAVE_INSTALACAO, Some(t.instalacao.codigo()));
    if !t.diametro_travado {
        let d = t.eletroduto.as_ref().map(|d| d.nominal.to_string());
        xdata::gravar(x, rota::CHAVE_DIAMETRO, d.as_deref());
    }
    // Os grupos 1000 do XDATA têm limite de tamanho: os condutores vão em partes.
    let fios = t.circuitos.iter().map(Condutores::serializado).collect::<Vec<_>>().join(";");
    let antigos: Vec<String> = xdata::ler(x).into_iter().map(|(k, _)| k).filter(|k| k.starts_with(rota::CHAVE_FIOS)).collect();
    for k in antigos {
        xdata::gravar(x, &k, None);
    }
    let chars: Vec<char> = fios.chars().collect();
    for (i, parte) in chars.chunks(FIOS_POR_GRUPO).enumerate() {
        xdata::gravar(x, &format!("{}.{i}", rota::CHAVE_FIOS), Some(&parte.iter().collect::<String>()));
    }
    if let Some(h) = tag {
        xdata::gravar(x, rota::CHAVE_TAG, Some(&h.hex()));
    }
    doc.model.replace(e);
}

/// O que uma regeneração fez.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Regeneracao {
    /// Tags atualizadas.
    pub tags: usize,
    /// Tags novas.
    pub criadas: usize,
    /// Tags e marcas antigas apagadas.
    pub apagadas: usize,
    /// Tabelas de quadro de cargas refeitas.
    pub tabelas: usize,
    /// Diagramas unifilares refeitos.
    pub unifilares: usize,
    pub avisos: Vec<String>,
}

/// Regenera tudo o que é gerado: atualiza as tags existentes (mantendo o deslocamento dado pelo
/// projetista), cria tags nos eletrodutos de `criar` (`None`: nenhuma nova), apaga as de
/// eletrodutos que sumiram e refaz as tabelas de quadro de cargas no desenho.
pub fn regenerar(doc: &mut Drawing, setup: &Setup, criar: Option<&[Handle]>) -> Result<Regeneracao, String> {
    let analise = crate::analisar(doc);
    let (m, _) = setup.unidade.metros(doc.header.i64("INSUNITS", 0));
    let m = if m > 0.0 { m } else { 1.0 };
    let (trechos, soltos, avisos) = todos_os_trechos(doc, &analise, setup);
    let mut reg = Regeneracao { avisos, ..Regeneracao::default() };
    reg.apagadas += apagar_marcas_antigas(doc);
    garantir_blocos(doc, m);

    let existentes = tags(doc);
    // O que existe de eletroduto: (handle, meio, ângulo, circuitos, diâmetro).
    let mut alvos: Vec<(Handle, [f64; 2], f64, Vec<Condutores>, Option<f64>)> =
        trechos.iter().map(|t| (t.handle, t.meio, t.angulo, t.circuitos.clone(), t.eletroduto.as_ref().map(|e| e.nominal))).collect();
    alvos.extend(soltos.iter().map(|s| (s.eletroduto, s.meio, s.angulo, Vec::new(), None)));
    for (tag, pai) in &existentes {
        if !alvos.iter().any(|a| a.0 == *pai) {
            doc.model.remove(*tag);
            doc.blocks.remove(&nome_do_bloco(*pai));
            reg.apagadas += 1;
        }
    }
    for (h, meio, angulo, circuitos, diametro) in alvos {
        let ja = existentes.iter().find(|(_, p)| *p == h).map(|(t, _)| *t);
        if ja.is_none() && !criar.is_some_and(|l| l.contains(&h)) {
            continue;
        }
        let rotacao = angulo_de_leitura(angulo);
        // O deslocamento: o que o projetista moveu em relação ao meio do eletroduto na última
        // regeneração. Se o eletroduto mudou, a tag o acompanha.
        let offset = ja
            .and_then(|t| doc.model.get(t))
            .and_then(|e| {
                let EntityKind::Insert(i) = &e.kind else { return None };
                let xd = xdata::ler(&e.common.xdata);
                let ancora = [numero_do_xdata(&xd, CHAVE_ANCORA_X)?, numero_do_xdata(&xd, CHAVE_ANCORA_Y)?];
                Some([i.insert.x - ancora[0], i.insert.y - ancora[1]])
            })
            .unwrap_or([0.0, 0.0]);
        let afastada = offset[0].hypot(offset[1]) * m > OFFSET_COM_CHAMADA;
        let (s, c) = rotacao.sin_cos();
        // O ponto do eletroduto em coordenadas locais: -offset girado de -rotacao.
        let chamada = afastada.then(|| [-offset[0] * c - offset[1] * s, offset[0] * s - offset[1] * c]);
        let nome = nome_do_bloco(h);
        let bloco = bloco_da_tag(doc, &nome, &circuitos, diametro, chamada, m, setup);
        doc.blocks.insert(nome.clone(), Arc::new(bloco));
        let posicao = Vec3::new(meio[0] + offset[0], meio[1] + offset[1], 0.0);
        let mut xd = Vec::new();
        xdata::gravar(&mut xd, rota::CHAVE_ESQUEMA, Some(rota::VERSAO_ESQUEMA));
        xdata::gravar(&mut xd, CHAVE_PAI, Some(&h.hex()));
        xdata::gravar(&mut xd, CHAVE_ANCORA_X, Some(&meio[0].to_string()));
        xdata::gravar(&mut xd, CHAVE_ANCORA_Y, Some(&meio[1].to_string()));
        xdata::gravar(&mut xd, CHAVE_OFFSET_X, Some(&offset[0].to_string()));
        xdata::gravar(&mut xd, CHAVE_OFFSET_Y, Some(&offset[1].to_string()));
        let tag = match ja.and_then(|t| doc.model.get(t).map(|e| (**e).clone())) {
            Some(mut e) => {
                if let EntityKind::Insert(i) = &mut e.kind {
                    i.insert = posicao;
                    i.rotation = rotacao;
                }
                e.common.xdata = xd;
                let t = e.handle;
                doc.model.replace(e);
                reg.tags += 1;
                t
            }
            None => {
                let ins = Insert {
                    block: nome,
                    insert: posicao,
                    scale: Vec3::new(1.0, 1.0, 1.0),
                    rotation: rotacao,
                    attribs: Vec::new(),
                    cols: 1,
                    rows: 1,
                    col_spacing: 0.0,
                    row_spacing: 0.0,
                };
                let t = doc
                    .add(&Space::Model, Common { layer: CAMADA.into(), xdata: xd, ..Common::default() }, EntityKind::Insert(ins))
                    .map_err(|e| e.to_string())?;
                reg.criadas += 1;
                t
            }
        };
        if let Some(t) = trechos.iter().find(|t| t.handle == h) {
            gravar_no_eletroduto(doc, t, Some(tag));
        }
    }
    reg.tabelas = crate::tabela::regenerar(doc, &analise, setup)?;
    reg.unifilares = crate::unifilar::regenerar(doc, &analise, setup)?;
    Ok(reg)
}

/// Regenera se o desenho já tem tags ou tabelas (o gatilho das edições no quadro de cargas).
pub fn regenerar_se_houver(doc: &mut Drawing, setup: &Setup) -> Result<Option<Regeneracao>, String> {
    if tags(doc).is_empty() && crate::tabela::tabelas(doc).is_empty() && crate::unifilar::unifilares(doc).is_empty() {
        return Ok(None);
    }
    regenerar(doc, setup, None).map(Some)
}

/// Resumo para o JSON dos comandos: quantas marcas de cada tipo.
pub fn contagem(trechos: &[Trecho]) -> HashMap<&'static str, usize> {
    let mut c: HashMap<&'static str, usize> = HashMap::new();
    for t in trechos {
        for k in &t.circuitos {
            for (tipo, _) in k.marcas() {
                *c.entry(tipo.bloco()).or_default() += 1;
            }
        }
    }
    c
}
