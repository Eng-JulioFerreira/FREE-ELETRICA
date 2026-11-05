// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Free Elétrica: a interface. Nada fica aberto sem ser pedido.
//!
//! 1. A paleta (`show`): as abas Elétrica e Telecom, com seções recolhíveis e ferramentas planas,
//!    como as Tool Sets do CADCraft.
//! 2. A mini-barra (`sobre_canvas`): sobre o desenho, só enquanto ELEPONTO, ELEELETRODUTO ou
//!    ELECIRCUITO esperam cliques. Muda o que o comando usa sem interrompê-lo.
//! 3. As janelas (`janelas`): blocos, inspetor de atributos, quadro de cargas em duas abas,
//!    dimensionamento, eletrodutos, pontos, conferência, materiais e configurações. Sem barra de
//!    título: o cabeçalho é interno, e cada uma fecha no X dele ou com Esc, a última aberta primeiro.
//! 4. A sobreposição (`sobre_canvas`): os halos da inspeção visual e o destaque do circuito.
//!
//! Toda alteração passa por um comando do motor (`app.run`/`app.start`): a interface não decide
//! nada de engenharia.
//!
//! Os textos ficam em português: o Free Elétrica é um addon para projetos brasileiros e não passa
//! pelo catálogo de traduções do CADCraft. Só os nomes dos painéis, o item de menu e os nomes dos
//! comandos passam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use cadcraft_doc::Handle;
use cadcraft_doc::geom::Vec2;
use craft_ui::docking::{Action, Node};
use craft_ui::layout::SplitSize;
use egui::{Align, Color32, Layout, RichText, Stroke, vec2};
use free_eletrica::inspecao::{Item, Nivel};
use free_eletrica::nbr5410::{DISJUNTORES, Esquema, SECOES, TipoCabo};
use free_eletrica::ponto::br;
use free_eletrica::rota::{Instalacao, TipoEletroduto};
use free_eletrica::setup::{DiametroCondutor, TamanhoEletroduto, diametros_padrao, eletrodutos_padrao};
use free_eletrica::{self as ele, CaboTelecom, CategoriaTelecom, CorCabo, PontoEletrico, Setup, Sistema, TipoPonto, Unidade};
use serde_json::{Value, json};

use crate::CadApp;
use crate::docking::Panel;
use crate::theme::Tokens;

const COR_AVISO: Color32 = Color32::from_rgb(232, 176, 72);
const COR_VIOLACAO: Color32 = Color32::from_rgb(235, 87, 87);
const COR_DESTAQUE_PONTO: Color32 = Color32::from_rgb(0, 229, 255);
const COR_DESTAQUE_TRECHO: Color32 = Color32::from_rgb(255, 235, 59);
/// Altura das linhas das tabelas.
const LINHA: f32 = 20.0;
/// Altura da paleta, com todas as ferramentas à vista.
const ALTURA_PALETA: f32 = 680.0;

/// Chave das configurações do Free Elétrica no armazenamento de preferências do CADCraft.
pub const PREFS_KEY: &str = "freeeletrica.setup";
/// A chave de quando o projeto se chamava CRAFT ELE: lida só quando a nova não existe.
pub const PREFS_KEY_ANTIGA: &str = "craftele.setup";

static JANELAS: LazyLock<Mutex<Janelas>> = LazyLock::new(|| Mutex::new(Janelas::default()));
static ABRIR_AO_INICIAR: AtomicBool = AtomicBool::new(false);

fn trava<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// O estado da interface que não vai para o desenho.
#[derive(Default)]
struct Janelas {
    /// Configurações: o rascunho até o OK.
    setup: Option<Rascunho>,
    /// As janelas abertas, a última por cima: é a que Esc fecha.
    abertas: Vec<Qual>,
    cargas: Cargas,
    pontos: Pontos,
    /// As opções da janela de dimensionamento.
    dimensionar: OpcoesDimensionar,
    /// A janela de blocos.
    blocos: Blocos,
    /// Inspeção visual ligada.
    inspecao: bool,
    /// O circuito destacado no desenho: (quadro, circuito).
    destaque: Option<(Handle, String)>,
    /// O filtro de categoria da mini-barra de pontos.
    filtro_barra: FiltroPontos,
    /// A aba da paleta.
    aba_paleta: AbaPaleta,
}

/// As janelas do Free Elétrica.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Qual {
    Setup,
    Cargas,
    Dimensionar,
    Blocos,
    Atributos,
    Eletrodutos,
    Pontos,
    Conferencia,
    Materiais,
}

/// Abre uma janela, por cima das outras.
fn abrir(q: Qual) -> Result<Value, String> {
    let mut j = trava(&JANELAS);
    j.abertas.retain(|x| *x != q);
    j.abertas.push(q);
    Ok(Value::Null)
}

/// Fecha uma janela e o que só existe com ela.
fn fechar(j: &mut Janelas, q: Qual) {
    j.abertas.retain(|x| *x != q);
    if q == Qual::Setup {
        j.setup = None;
    }
}

/// A janela de pontos: o filtro e a exclusão pendente.
#[derive(Clone, Default)]
struct Pontos {
    filtro: FiltroPontos,
    busca: String,
    /// Ponto com a exclusão pedida, ou `Some(None)` para todos os filtrados (espera a confirmação).
    excluir: Option<Option<Handle>>,
}

#[derive(Clone)]
struct Rascunho {
    setup: Setup,
    aba: Aba,
    novo_cabo: CaboTelecom,
    erro: Option<String>,
    /// A tabela de demanda aberta no editor.
    tabela_editada: Option<ele::demanda::TabelaDemanda>,
    erro_tabela: Option<String>,
}

/// As abas do quadro de cargas.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum AbaCargas {
    #[default]
    Resumo,
    Ficha,
}

/// As premissas do dimensionamento, até processar.
#[derive(Clone, Default)]
struct OpcoesDimensionar {
    compartilhar_pe: bool,
    temperatura: f64,
    fca_manual: bool,
    fca: f64,
}

/// A janela de blocos.
#[derive(Clone, Default)]
struct Blocos {
    filtro: FiltroPontos,
    nome: String,
    categoria: Option<ele::familia::FamilyCategory>,
}

#[derive(Clone, Default)]
struct Cargas {
    aba: AbaCargas,
    /// O circuito aberto na ficha.
    selecionado: Option<String>,
    quadro: Option<Handle>,
    /// Descrições em edição: (circuito, texto).
    descricoes: Vec<(String, String)>,
    /// Circuito com a exclusão pedida (espera a confirmação).
    excluir: Option<String>,
    nova_descricao: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Aba {
    Criterios,
    Alturas,
    Geral,
    Condutores,
    Eletrodutos,
    Agrupamento,
    Demanda,
    Telecom,
}

impl Aba {
    const TODAS: [Aba; 8] =
        [Aba::Criterios, Aba::Alturas, Aba::Geral, Aba::Condutores, Aba::Eletrodutos, Aba::Agrupamento, Aba::Demanda, Aba::Telecom];
    fn nome(self) -> &'static str {
        match self {
            Aba::Criterios => "Critérios elétricos",
            Aba::Alturas => "Alturas",
            Aba::Geral => "Geral",
            Aba::Condutores => "Condutores",
            Aba::Eletrodutos => "Eletrodutos",
            Aba::Agrupamento => "Agrupamento",
            Aba::Demanda => "Demanda / Concessionárias",
            Aba::Telecom => "Cabos",
        }
    }
}

/// O que a interface pede para o motor fazer, executado depois de desenhá-la.
enum Acao {
    /// `app.run(comando, parâmetros)`.
    Rodar(&'static str, Value),
    /// `app.start(comando)`: começa um comando interativo.
    Iniciar(&'static str),
    /// Manda uma palavra-chave para o comando em andamento.
    Palavra(&'static str),
    /// Roda um comando e mostra o resultado na barra de status.
    RodarComStatus(&'static str, Value, fn(&Value) -> String),
    /// Abre a ficha de um circuito.
    Ficha(Handle, String),
    /// Fecha uma janela.
    Fechar(Qual),
    /// Seleciona objetos no desenho.
    Selecionar(Vec<Handle>),
    /// Enquadra estes pontos na tela.
    Localizar(Vec<[f64; 2]>),
}

/// As configurações em uso (guardadas no motor: `free_eletrica::setup`).
pub fn setup() -> Setup {
    ele::setup::atual()
}

fn definir_setup(s: Setup) {
    ele::setup::definir(s);
}

/// As configurações como JSON, para o armazenamento do CADCraft ([`PREFS_KEY`]).
pub fn prefs_json() -> String {
    serde_json::to_string(&setup()).unwrap_or_default()
}

pub fn load_prefs(json: &str) {
    if let Ok(s) = serde_json::from_str::<Setup>(json) {
        definir_setup(s);
    }
}

/// Pede para a paleta abrir no primeiro quadro, se o usuário nunca a fechou. Só o executável
/// chama: o layout de fábrica (e os testes dele) continuam os do CADCraft.
pub fn abrir_ao_iniciar() {
    ABRIR_AO_INICIAR.store(true, Ordering::Relaxed);
}

/// Abre a paleta (comando ELETRICA, menu Janela). Na primeira vez ela fica acima das Tool Sets.
pub fn open(app: &mut CadApp) -> Result<Value, String> {
    let panel = Panel::Eletrica;
    let primeira = !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel);
    let r = crate::docking::command(app, false, &json!({ "operation": "open", "panel": panel.id() }))?;
    if primeira && app.ui.docking.layout.contains(&Panel::ToolSets) {
        let mover = json!({ "operation": "move", "panel": panel.id(), "target": Panel::ToolSets.id(), "zone": "top" });
        if let Ok(r) = crate::docking::command(app, false, &mover) {
            // A paleta só tem botões: altura fixa, e o resto da coluna fica com as Tool Sets.
            redimensionar(app, panel, SplitSize::FixedFirst(ALTURA_PALETA));
            return Ok(r);
        }
    }
    Ok(r)
}

/// Muda o tamanho da divisão que tem `panel` como filho. Sem divisão (flutuando), não faz nada.
fn redimensionar(app: &mut CadApp, panel: Panel, size: SplitSize) {
    let mut caminho = Vec::new();
    if let Some(raiz) = &app.ui.docking.layout.root
        && caminho_do_painel(raiz, panel, &mut caminho)
        && let Ok(acao) = serde_json::to_value(Action::<Panel>::ResizeSplit { path: caminho, size })
    {
        let _ = crate::docking::command(app, false, &json!({ "action": acao }));
    }
}

/// O caminho (`true` = segundo filho) até a divisão com um filho que é só as abas de `panel`.
fn caminho_do_painel(no: &Node<Panel>, panel: Panel, caminho: &mut Vec<bool>) -> bool {
    let Node::Split { first, second, .. } = no else { return false };
    let e_o_painel = |n: &Node<Panel>| matches!(n, Node::Tabs { panels, .. } if panels.contains(&panel));
    if e_o_painel(first) || e_o_painel(second) {
        return true;
    }
    for (lado, filho) in [(false, first), (true, second)] {
        caminho.push(lado);
        if caminho_do_painel(filho, panel, caminho) {
            return true;
        }
        caminho.pop();
    }
    false
}

fn abrir_setup() -> Result<Value, String> {
    let rascunho = Rascunho {
        setup: setup(),
        aba: Aba::Criterios,
        novo_cabo: CaboTelecom { categoria: CategoriaTelecom::Utp, quantidade: 4, ..CaboTelecom::default() },
        erro: None,
        tabela_editada: None,
        erro_tabela: None,
    };
    trava(&JANELAS).setup = Some(rascunho);
    abrir(Qual::Setup)
}

/// ELEPONTOS: prepara o desenho e começa a inserir, com a mini-barra. O clique já insere.
fn comecar_a_inserir(app: &mut CadApp) -> Result<Value, String> {
    app.run("eletrica.bootstrap", json!({}))?;
    app.start("eleponto");
    Ok(Value::Null)
}

/// Começa a inserir já com um bloco escolhido.
fn inserir_bloco(app: &mut CadApp, bloco: &str) -> Result<Value, String> {
    app.run("eletrica.bootstrap", json!({}))?;
    app.run("eletrica.ponto.modelo", json!({ "bloco": bloco }))?;
    app.start("eleponto");
    Ok(Value::Null)
}

/// Comandos do Free Elétrica que só existem na interface: a paleta, as janelas e a inspeção. `None`
/// quando `id` não é um deles.
///
/// Digitados: ELETRICA (paleta), ELECONFIG, ELEPONTOS, ELECARGAS, ELEROTEAR, ELECONFERIR e
/// ELEINSPECAO. Todo o resto é comando do motor (`crates/engine/src/cmd/eletrica.rs`).
pub fn route(app: &mut CadApp, id: &str, _params: &Value) -> Option<Result<Value, String>> {
    match id {
        "ui.panel.eletrica" | "eletrica" | "freeeletrica" => Some(open(app)),
        "ui.eletrica.setup" | "eleconfig" => Some(abrir_setup()),
        // Conferir é a inspeção visual ligada, com a lista dos problemas no painel.
        "ui.eletrica.conferir" | "eleconferir" => {
            trava(&JANELAS).inspecao = true;
            Some(abrir(Qual::Conferencia))
        }
        "ui.eletrica.inspecao" | "eleinspecao" => {
            let mut j = trava(&JANELAS);
            j.inspecao = !j.inspecao;
            Some(Ok(json!({ "inspecao": j.inspecao })))
        }
        "ui.eletrica.pontos" | "elepontos" => Some(comecar_a_inserir(app)),
        "ui.eletrica.lista" => Some(abrir(Qual::Pontos)),
        "ui.eletrica.telecom.dados" => Some(inserir_bloco(app, "RJ45")),
        "ui.eletrica.telecom.rack" => Some(inserir_bloco(app, "RACK")),
        "ui.eletrica.telecom.lista" => {
            let mut j = trava(&JANELAS);
            j.pontos.filtro = FiltroPontos::Dados;
            drop(j);
            Some(abrir(Qual::Pontos))
        }
        "ui.eletrica.telecom.cabos" => {
            let r = abrir_setup();
            if let Some(rascunho) = trava(&JANELAS).setup.as_mut() {
                rascunho.aba = Aba::Telecom;
            }
            Some(r)
        }
        "ui.eletrica.blocos" | "eleblocos" => Some(abrir(Qual::Blocos)),
        "ui.eletrica.atributos" | "eleatributos" => Some(abrir(Qual::Atributos)),
        "ui.eletrica.dimensionar" | "eledimensionar" => {
            let s = setup();
            trava(&JANELAS).dimensionar = OpcoesDimensionar {
                compartilhar_pe: s.compartilhar_pe,
                temperatura: s.temperatura_ambiente,
                fca_manual: s.fca_manual.is_some(),
                fca: s.fca_manual.unwrap_or(0.8),
            };
            Some(abrir(Qual::Dimensionar))
        }
        "ui.eletrica.materiais" | "elemateriais" => Some(abrir(Qual::Materiais)),
        // Rotear fiação vive dentro do Quadro de cargas; ELEROTEAR abre o quadro de cargas.
        "ui.eletrica.cargas" | "elecargas" | "ui.eletrica.rotear" | "elerotear" => Some(abrir(Qual::Cargas)),
        "ui.eletrica.eletrodutos" => Some(abrir(Qual::Eletrodutos)),
        _ => None,
    }
}

/// Tab durante ELEPONTO gira o ponto em 90°. `false`: Tab segue o caminho normal do CADCraft.
pub fn tab(app: &mut CadApp) -> bool {
    let inserindo = app.session.running.as_ref().is_some_and(|r| r.id == "eleponto");
    if !inserindo || !app.cmd.buffer.is_empty() || !app.session.current_prompt().is_some_and(|p| p.accept.point) {
        return false;
    }
    if let Err(e) = app.session.input(cadcraft_engine::Input::Keyword("Girar".into())) {
        app.session.echo(e.to_string());
    }
    true
}

fn pequeno(texto: impl Into<String>, cor: Color32) -> RichText {
    RichText::new(texto).font(crate::theme::small()).color(cor)
}

/// Número com casas fixas e vírgula decimal.
fn fixo(v: f64, casas: usize) -> String {
    let texto = format!("{v:.casas$}").replace('.', ",");
    // Soma vazia ou arredondamento dão -0: zero não tem sinal.
    match texto.strip_prefix('-') {
        Some(resto) if resto.chars().all(|c| c == '0' || c == ',') => resto.to_string(),
        _ => texto,
    }
}

/// Campo numérico com vírgula decimal ("2,80 m"); aceita digitar com vírgula ou ponto.
fn campo<'a>(valor: &'a mut f64, faixa: std::ops::RangeInclusive<f64>, passo: f64, casas: usize, sufixo: &'static str) -> egui::DragValue<'a> {
    egui::DragValue::new(valor)
        .range(faixa)
        .speed(passo)
        .custom_formatter(move |n, _| format!("{}{sufixo}", format!("{n:.casas$}").replace('.', ",")))
        .custom_parser(ele::ponto::numero)
}

/// Uma célula de tabela com largura fixa. Números à direita, texto à esquerda.
fn celula(ui: &mut egui::Ui, largura: f32, texto: impl Into<RichText>, direita: bool) -> egui::Response {
    // Pintada num retângulo de largura exata: a coluna nunca cresce nem invade a vizinha.
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, LINHA), egui::Sense::hover());
    let galley = egui::WidgetText::from(texto.into()).into_galley(ui, Some(egui::TextWrapMode::Truncate), largura, egui::TextStyle::Body);
    let meio = galley.size().y / 2.0;
    let pos = if direita { rect.right_center() - vec2(galley.size().x, meio) } else { rect.left_center() - vec2(0.0, meio) };
    let texto = galley.text().to_string();
    resposta.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &texto));
    ui.painter().galley(pos, galley, ui.visuals().text_color());
    resposta
}

/// Cabeçalho de coluna (texto pequeno, apagado).
fn cabecalho(ui: &mut egui::Ui, t: &Tokens, colunas: &[(&str, f32, bool)]) {
    for (nome, largura, direita) in colunas {
        celula(ui, *largura, pequeno(*nome, t.text_dim).strong(), *direita);
    }
    ui.end_row();
}

fn numero(texto: impl Into<String>) -> RichText {
    RichText::new(texto).monospace()
}

/// Um cadeado desenhado, clicável: fechado quando o valor foi escolhido à mão, aberto quando é
/// automático; vermelho quando o valor escolhido viola a norma.
fn cadeado(ui: &mut egui::Ui, travado: bool, violacao: bool, dica: &str) -> egui::Response {
    let (rect, resposta) = ui.allocate_exact_size(vec2(14.0, LINHA), egui::Sense::click());
    let cor = if violacao {
        COR_VIOLACAO
    } else if resposta.hovered() {
        ui.visuals().strong_text_color()
    } else if travado {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let p = ui.painter();
    let c = rect.center();
    let corpo = egui::Rect::from_center_size(c + vec2(0.0, 3.0), vec2(9.0, 7.0));
    if travado {
        p.rect_filled(corpo, 1.0, cor);
    } else {
        p.rect_stroke(corpo, 1.0, Stroke::new(1.2, cor), egui::StrokeKind::Inside);
    }
    // A alça: fechada encosta dos dois lados; aberta, só do esquerdo.
    let (esq, dir, topo) = (corpo.left() + 2.0, corpo.right() - 2.0, corpo.top() - 4.0);
    let mut alca = vec![egui::pos2(esq, corpo.top()), egui::pos2(esq, topo), egui::pos2(dir, topo)];
    alca.push(egui::pos2(dir, if travado { corpo.top() } else { topo + 1.5 }));
    p.add(egui::Shape::line(alca, Stroke::new(1.2, cor)));
    let nome = if travado { "Travado" } else { "Automático" };
    resposta.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, nome));
    resposta.on_hover_text(dica)
}

/// Escolha do quadro. Devolve se ele mudou.
fn seletor_quadro(ui: &mut egui::Ui, id: &str, analise: &ele::Analise, quadro: &mut Option<Handle>) -> bool {
    let quadros = analise.quadros();
    let mut mudou = false;
    if quadro.is_none_or(|h| !quadros.iter().any(|q| q.handle == h)) {
        *quadro = quadros.first().map(|q| q.handle);
        mudou = true;
    }
    ui.label("Quadro");
    let atual = quadros.iter().find(|q| Some(q.handle) == *quadro).map_or(String::new(), |q| q.rotulo());
    egui::ComboBox::from_id_salt((id, "quadro")).selected_text(atual).width(140.0).show_ui(ui, |ui| {
        for q in &quadros {
            if ui.selectable_label(*quadro == Some(q.handle), q.rotulo()).clicked() {
                *quadro = Some(q.handle);
                mudou = true;
            }
        }
    });
    mudou
}

// ---------------------------------------------------------------- zona 1: a paleta

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let tem_desenho = app.session.state().is_ok();
    // No editor de blocos, a paleta vira o painel da família do bloco.
    let editando = app.session.state().ok().and_then(|st| st.block_edit.as_ref().filter(|b| !b.current).map(|b| b.name.clone()));
    if let Some(nome) = editando {
        let familia = app.session.doc().ok().and_then(|d| ele::familia::ler(d, &nome));
        let mut acoes = Vec::new();
        egui::ScrollArea::vertical().id_salt("dock-eletrica-familia").auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| propriedades_da_familia(ui, &t, &nome, familia.as_ref(), &mut acoes));
        });
        executar(app, acoes);
        return;
    }
    let inspecao = trava(&JANELAS).inspecao;
    let mut aba = trava(&JANELAS).aba_paleta;
    let mut comando: Option<&'static str> = None;
    let mut alternar: Option<String> = None;
    let recolhidas = app.ui.collapsed_groups.clone();
    egui::Frame::NONE.fill(t.panel).show(ui, |ui| {
        let r = ui.max_rect();
        let p = ui.painter().clone();
        // As abas das disciplinas, como Desenho | Modelagem nas Tool Sets.
        let altura_abas = 28.0;
        let mut x = r.left();
        for (qual, nome) in [(AbaPaleta::Eletrica, "Elétrica"), (AbaPaleta::Telecom, "Telecom")] {
            let tr = egui::Rect::from_min_size(egui::pos2(x, r.top()), vec2(92.0, altura_abas));
            let ativa = aba == qual;
            let resp = ui.interact(tr, ui.id().with(("ele-aba-paleta", nome)), egui::Sense::click());
            p.rect_filled(tr, 0.0, if ativa { t.tab_active } else { t.chrome });
            p.text(tr.center(), egui::Align2::CENTER_CENTER, nome, egui::FontId::proportional(13.5), if ativa { t.text } else { t.text_dim });
            crate::icons::describe_control(ui, &resp, nome, Some(ativa), true);
            if resp.clicked() {
                aba = qual;
            }
            x += 92.0;
        }
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(x, r.top()), egui::pos2(r.right(), r.top() + altura_abas)), 0.0, t.chrome);
        let corpo = egui::Rect::from_min_max(egui::pos2(r.left(), r.top() + altura_abas), r.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(corpo), |ui| {
            egui::ScrollArea::vertical().id_salt("dock-eletrica").auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let secoes = if aba == AbaPaleta::Eletrica { &PALETA_ELETRICA[..] } else { &PALETA_TELECOM[..] };
                for secao in secoes {
                    let chave = chave_da_secao(secao.nome);
                    let recolhida = recolhidas.contains(&chave);
                    if cabecalho_da_secao(ui, &t, secao.nome, recolhida) {
                        alternar = Some(chave);
                    }
                    if !recolhida {
                        for f in secao.ferramentas {
                            let ativa = f.comando == "ui.eletrica.inspecao" && inspecao;
                            if linha_de_ferramenta(ui, &t, f, tem_desenho || !f.exige_desenho, ativa) {
                                comando = Some(f.comando);
                            }
                        }
                    }
                    let (sr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
                    ui.painter().hline(sr.x_range().shrink(6.0), sr.center().y, Stroke::new(1.0, t.border));
                }
            });
        });
    });
    trava(&JANELAS).aba_paleta = aba;
    if let Some(chave) = alternar {
        let grupos = &mut app.ui.collapsed_groups;
        if let Some(i) = grupos.iter().position(|g| *g == chave) {
            grupos.remove(i);
        } else {
            grupos.push(chave);
        }
    }
    if let Some(c) = comando {
        if c.starts_with("ui.") {
            if let Err(e) = app.run(c, Value::Null) {
                app.set_status(format!("Free Elétrica: {e}"));
            }
        } else {
            app.start(c);
        }
    }
}

/// As disciplinas da paleta.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum AbaPaleta {
    #[default]
    Eletrica,
    Telecom,
}

/// Um botão da paleta.
struct Ferramenta {
    icone: Icone,
    rotulo: &'static str,
    comando: &'static str,
    dica: &'static str,
    exige_desenho: bool,
}

/// Uma seção recolhível da paleta.
struct Secao {
    nome: &'static str,
    ferramentas: &'static [Ferramenta],
}

const fn ferr(icone: Icone, rotulo: &'static str, comando: &'static str, dica: &'static str) -> Ferramenta {
    Ferramenta { icone, rotulo, comando, dica, exige_desenho: true }
}

const PALETA_ELETRICA: [Secao; 6] = [
    Secao {
        nome: "Pontos e Famílias",
        ferramentas: &[
            ferr(
                Icone::Bloco,
                "Blocos e famílias...",
                "ui.eletrica.blocos",
                "Crie um bloco elétrico novo ou edite os do desenho, com categoria, altura padrão e caixa (ELEBLOCOS)",
            ),
            ferr(
                Icone::Tomada,
                "Inserir pontos",
                "ui.eletrica.pontos",
                "A mini-barra escolhe o bloco, a potência, a altura e o quadro; cada clique no desenho insere um ponto. Tab gira (ELEPONTOS)",
            ),
            ferr(Icone::Lista, "Lista de pontos...", "ui.eletrica.lista", "Pontos do desenho por categoria: altura, selecionar e excluir"),
            ferr(
                Icone::Etiqueta,
                "Inspetor de atributos...",
                "ui.eletrica.atributos",
                "Selecione um bloco: valores, visibilidade e posição de cada atributo (ELEATRIBUTOS)",
            ),
        ],
    },
    Secao {
        nome: "Circuitos e Comandos",
        ferramentas: &[
            ferr(
                Icone::Circuito,
                "Criar circuito",
                "elecircuito",
                "Selecione o quadro, o número e os pontos do circuito; a barra sobre o desenho edita descrição, esquema e tensão (ELECIRCUITO)",
            ),
            ferr(Icone::Interruptor, "Comandos e luminárias", "elecomando", "Selecione o interruptor e as luminárias que ele comanda (ELECOMANDO)"),
        ],
    },
    Secao {
        nome: "Quadros e Dimensionamento",
        ferramentas: &[
            ferr(
                Icone::Quadro,
                "Quadro de cargas...",
                "ui.eletrica.cargas",
                "Resumo dos circuitos de um quadro e a ficha de engenharia de cada um (ELECARGAS)",
            ),
            ferr(
                Icone::Engrenagem,
                "Dimensionar projeto...",
                "ui.eletrica.dimensionar",
                "Esquemas, cabos e grupos DR por circuito; calcule, revise e aplique ao quadro (ELEDIMENSIONAR)",
            ),
        ],
    },
    Secao {
        nome: "Eletrodutos",
        ferramentas: &[
            ferr(
                Icone::Eletroduto,
                "Inserir eletroduto",
                "eleeletroduto",
                "Clique nos blocos em sequência; a mini-barra escolhe o perfil e o traçado. Clicar no meio de um eletroduto insere uma caixa de passagem (ELEELETRODUTO)",
            ),
            ferr(Icone::Lista, "Lista de eletrodutos...", "ui.eletrica.eletrodutos", "Trechos, com tipo, cota, diâmetro e ocupação"),
        ],
    },
    Secao {
        nome: "Documentação e Tabelas",
        ferramentas: &[
            ferr(
                Icone::Chamada,
                "Tags de fiação",
                "elefiacao",
                "Põe a simbologia de fiação e o diâmetro nos eletrodutos; a posição que você der fica guardada (ELEFIACAO)",
            ),
            ferr(Icone::Grade, "Tabela do quadro", "elequadro_tabela", "Desenha o quadro de cargas num ponto da planta (ELEQUADRO_TABELA)"),
            ferr(Icone::Atualizar, "Atualizar tags e tabelas", "eleregen", "Refaz o que é gerado sem mudar as posições (ELEREGEN)"),
            ferr(
                Icone::Prancheta,
                "Lista de materiais...",
                "ui.eletrica.materiais",
                "Cabos por seção e tipo, eletrodutos por tipo e diâmetro, caixas, pontos e disjuntores (ELEMATERIAIS)",
            ),
        ],
    },
    Secao {
        nome: "Projeto e Inspeção",
        ferramentas: &[
            ferr(
                Icone::Olho,
                "Inspecionar projeto",
                "ui.eletrica.inspecao",
                "Halos no desenho: vermelho falta circuito ou comando, laranja sem caminho até o quadro, amarelo acima do limite, verde válido (ELEINSPECAO)",
            ),
            Ferramenta {
                icone: Icone::Controles,
                rotulo: "Configurações...",
                comando: "ui.eletrica.setup",
                dica: "Critérios do projeto, alturas, cabos, eletrodutos e demanda (ELECONFIG)",
                exige_desenho: false,
            },
        ],
    },
];

const PALETA_TELECOM: [Secao; 3] = [
    Secao {
        nome: "Pontos e Racks",
        ferramentas: &[
            ferr(
                Icone::Tomada,
                "Inserir ponto de dados",
                "ui.eletrica.telecom.dados",
                "A mini-barra já começa com o bloco RJ45; cada clique no desenho insere um ponto",
            ),
            ferr(
                Icone::Quadro,
                "Inserir rack",
                "ui.eletrica.telecom.rack",
                "A mini-barra já começa com o bloco RACK; cada clique no desenho insere um rack",
            ),
            ferr(
                Icone::Lista,
                "Lista de pontos de dados...",
                "ui.eletrica.telecom.lista",
                "Pontos de dados do desenho: altura, selecionar e excluir",
            ),
        ],
    },
    Secao {
        nome: "Infraestrutura",
        ferramentas: &[
            ferr(
                Icone::Eletroduto,
                "Inserir eletroduto",
                "eleeletroduto",
                "O mesmo eletroduto da aba Elétrica, escolhendo o perfil e o traçado na mini-barra",
            ),
            ferr(Icone::Lista, "Lista de eletrodutos...", "ui.eletrica.eletrodutos", "Trechos, com tipo, cota, diâmetro e ocupação"),
        ],
    },
    Secao {
        nome: "Cabos",
        ferramentas: &[Ferramenta {
            icone: Icone::Cabo,
            rotulo: "Catálogo de cabos...",
            comando: "ui.eletrica.telecom.cabos",
            dica: "UTP, fibra e coaxial cadastrados, com o diâmetro de cada um",
            exige_desenho: false,
        }],
    },
];

/// A chave da seção em `app.ui.collapsed_groups`, junto das Tool Sets: fica salva com o programa.
fn chave_da_secao(nome: &str) -> String {
    format!("eletrica/{nome}")
}

/// O cabeçalho de uma seção, com a seta. Devolve se foi clicado.
fn cabecalho_da_secao(ui: &mut egui::Ui, t: &Tokens, nome: &str, recolhida: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), egui::Sense::click());
    let p = ui.painter();
    let seta = egui::Rect::from_center_size(egui::pos2(r.left() + 11.0, r.center().y), vec2(11.0, 11.0));
    crate::icons::paint(p, seta, if recolhida { crate::icons::Icon::ChevronRight } else { crate::icons::Icon::ChevronDown }, false);
    p.text(egui::pos2(r.left() + 22.0, r.center().y), egui::Align2::LEFT_CENTER, nome, egui::FontId::proportional(12.5), t.text);
    crate::icons::describe_control(ui, &resp, nome, Some(!recolhida), false);
    resp.clicked()
}

/// Uma linha da paleta: ícone e texto, sem moldura, com um fundo leve sob o mouse. Devolve se foi clicada.
fn linha_de_ferramenta(ui: &mut egui::Ui, t: &Tokens, f: &Ferramenta, habilitada: bool, ativa: bool) -> bool {
    ui.add_enabled_ui(habilitada, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 25.0), egui::Sense::click());
        let p = ui.painter();
        if ativa {
            p.rect_filled(r, 0.0, t.tab_active);
        } else if resp.hovered() {
            p.rect_filled(r, 0.0, t.control_hover.gamma_multiply(0.6));
        }
        let cor = if habilitada { t.text } else { t.text_dim };
        let caixa = egui::Rect::from_center_size(egui::pos2(r.left() + 19.0, r.center().y), vec2(16.0, 16.0));
        desenhar_icone(p, caixa, f.icone, cor, if habilitada { COR_ICONE } else { t.text_dim });
        p.text(egui::pos2(r.left() + 34.0, r.center().y), egui::Align2::LEFT_CENTER, f.rotulo, egui::FontId::proportional(12.5), cor);
        crate::icons::describe_control(ui, &resp, f.rotulo, None, false);
        resp.on_hover_text(f.dica).on_disabled_hover_text(f.dica).clicked()
    })
    .inner
}

/// O azul de detalhe dos ícones da paleta.
const COR_ICONE: Color32 = Color32::from_rgb(0x4D, 0xA3, 0xFF);

/// Os ícones da paleta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Icone {
    Bloco,
    Tomada,
    Lista,
    Etiqueta,
    Circuito,
    Interruptor,
    Quadro,
    Engrenagem,
    Eletroduto,
    Chamada,
    Grade,
    Atualizar,
    Prancheta,
    Olho,
    Controles,
    Cabo,
}

/// Desenha um ícone em traço fino, com um detalhe em azul. As coordenadas vão de 0 a 16.
fn desenhar_icone(p: &egui::Painter, caixa: egui::Rect, icone: Icone, cor: Color32, azul: Color32) {
    let e = caixa.width() / 16.0;
    let em = |x: f32, y: f32| caixa.min + vec2(x * e, y * e);
    let traco = Stroke::new(1.1, cor);
    let detalhe = Stroke::new(1.4, azul);
    let l = |pontos: &[(f32, f32)], s: Stroke| {
        p.add(egui::Shape::line(pontos.iter().map(|(x, y)| em(*x, *y)).collect(), s));
    };
    let ret = |x0: f32, y0: f32, x1: f32, y1: f32, s: Stroke| {
        p.rect_stroke(egui::Rect::from_min_max(em(x0, y0), em(x1, y1)), 1.0, s, egui::StrokeKind::Middle);
    };
    let circ = |x: f32, y: f32, r: f32, s: Stroke| {
        p.circle_stroke(em(x, y), r * e, s);
    };
    match icone {
        Icone::Bloco => {
            ret(2.0, 3.0, 14.0, 13.0, traco);
            l(&[(5.0, 10.0), (8.0, 5.0), (11.0, 10.0), (5.0, 10.0)], detalhe);
        }
        Icone::Tomada => {
            circ(7.0, 9.0, 5.0, traco);
            l(&[(5.5, 7.5), (5.5, 10.5)], traco);
            l(&[(8.5, 7.5), (8.5, 10.5)], traco);
            l(&[(13.0, 1.5), (13.0, 6.5)], detalhe);
            l(&[(10.5, 4.0), (15.5, 4.0)], detalhe);
        }
        Icone::Lista => {
            for y in [4.0, 8.0, 12.0] {
                l(&[(1.5, y), (3.5, y)], detalhe);
                l(&[(5.5, y), (14.5, y)], traco);
            }
        }
        Icone::Etiqueta => {
            l(&[(1.5, 5.0), (7.0, 2.0), (12.5, 5.0), (12.5, 11.0), (1.5, 11.0), (1.5, 5.0)], traco);
            circ(10.5, 10.5, 3.0, detalhe);
            l(&[(12.7, 12.7), (15.0, 15.0)], detalhe);
        }
        Icone::Circuito => {
            l(&[(1.0, 8.0), (4.0, 8.0)], traco);
            l(&[(12.0, 8.0), (15.0, 8.0)], traco);
            l(&[(9.5, 2.0), (5.5, 8.5), (8.5, 8.5), (6.5, 14.0), (11.0, 7.0), (8.0, 7.0), (9.5, 2.0)], detalhe);
        }
        Icone::Interruptor => {
            circ(3.0, 12.0, 1.5, traco);
            circ(13.0, 12.0, 1.5, traco);
            l(&[(4.2, 11.2), (11.5, 6.5)], traco);
            circ(8.0, 3.5, 2.5, detalhe);
        }
        Icone::Quadro => {
            ret(2.5, 1.5, 13.5, 14.5, traco);
            l(&[(2.5, 5.5), (13.5, 5.5)], traco);
            for y in [8.0, 10.5, 13.0] {
                l(&[(5.0, y), (11.0, y)], detalhe);
            }
        }
        Icone::Engrenagem => {
            circ(8.0, 8.0, 4.5, traco);
            for (dx, dy) in [(0.0, 1.0), (1.0, 0.0), (0.7, 0.7), (0.7, -0.7)] {
                l(&[(8.0 + dx * 4.5, 8.0 + dy * 4.5), (8.0 + dx * 6.5, 8.0 + dy * 6.5)], traco);
                l(&[(8.0 - dx * 4.5, 8.0 - dy * 4.5), (8.0 - dx * 6.5, 8.0 - dy * 6.5)], traco);
            }
            circ(8.0, 8.0, 1.8, detalhe);
        }
        Icone::Eletroduto => {
            ret(1.0, 11.0, 4.0, 14.0, traco);
            ret(12.0, 2.0, 15.0, 5.0, traco);
            l(&[(4.0, 12.5), (7.0, 11.5), (10.0, 9.0), (12.0, 6.5), (13.5, 5.0)], detalhe);
        }
        Icone::Chamada => {
            ret(6.5, 2.0, 15.0, 8.0, traco);
            l(&[(6.5, 6.0), (1.5, 13.5)], detalhe);
            circ(1.8, 13.5, 0.8, detalhe);
        }
        Icone::Grade => {
            ret(1.5, 2.5, 14.5, 13.5, traco);
            l(&[(1.5, 6.5), (14.5, 6.5)], detalhe);
            l(&[(1.5, 10.0), (14.5, 10.0)], traco);
            l(&[(6.0, 2.5), (6.0, 13.5)], traco);
            l(&[(10.5, 2.5), (10.5, 13.5)], traco);
        }
        Icone::Atualizar => {
            let arco = |ini: f32, fim: f32, s: Stroke| {
                let v: Vec<(f32, f32)> = (0..=10)
                    .map(|i| {
                        let a = (ini + (fim - ini) * i as f32 / 10.0).to_radians();
                        (8.0 + 5.5 * a.cos(), 8.0 + 5.5 * a.sin())
                    })
                    .collect();
                l(&v, s);
            };
            arco(200.0, 340.0, traco);
            arco(20.0, 160.0, detalhe);
            l(&[(13.2, 2.2), (13.2, 6.1), (9.5, 5.2)], traco);
            l(&[(2.8, 13.8), (2.8, 9.9), (6.5, 10.8)], detalhe);
        }
        Icone::Prancheta => {
            ret(3.0, 3.0, 13.0, 15.0, traco);
            ret(6.0, 1.5, 10.0, 4.5, detalhe);
            l(&[(5.5, 8.0), (10.5, 8.0)], traco);
            l(&[(5.5, 11.0), (10.5, 11.0)], traco);
        }
        Icone::Olho => {
            let lado = |sinal: f32| -> Vec<(f32, f32)> {
                (0..=12)
                    .map(|i| {
                        let x = i as f32 / 12.0;
                        (1.0 + 14.0 * x, 8.0 + sinal * 4.5 * (x * std::f32::consts::PI).sin())
                    })
                    .collect()
            };
            l(&lado(1.0), traco);
            l(&lado(-1.0), traco);
            circ(8.0, 8.0, 2.0, detalhe);
        }
        Icone::Controles => {
            for (y, x) in [(3.5, 10.0), (8.0, 5.0), (12.5, 11.0)] {
                l(&[(1.5, y), (14.5, y)], traco);
                p.circle_filled(em(x, y), 1.8 * e, azul);
            }
        }
        Icone::Cabo => {
            l(&[(1.5, 13.0), (5.0, 13.0), (7.0, 8.0), (9.0, 3.0), (14.5, 3.0)], traco);
            for (x, y) in [(1.5, 13.0), (14.5, 3.0)] {
                circ(x, y, 1.4, detalhe);
            }
        }
    }
}

fn ferramenta(ui: &mut egui::Ui, texto: &str, ativa: bool, dica: &str) -> egui::Response {
    let largura = ui.available_width();
    ui.add_enabled(ativa, egui::Button::new(texto).min_size(vec2(largura, 26.0))).on_hover_text(dica).on_disabled_hover_text(dica)
}

// ---------------------------------------------------------------- o que foi lido do desenho

type ChaveResumo = ((u64, u64), Setup, Handle);
type ChaveDestaque = ((u64, u64), Setup, (Handle, String));

/// O que já foi lido do desenho aberto, refeito a cada edição.
#[derive(Clone, Default)]
struct Estado {
    /// (uid, revisão) do desenho analisado.
    chave: Option<(u64, u64)>,
    analise: Arc<ele::Analise>,
    /// Os blocos de ponto do desenho.
    blocos: Arc<Vec<ele::biblioteca::BlocoPonto>>,
    resumo: Option<(ChaveResumo, Arc<Result<ele::ResumoQuadro, String>>)>,
    inspecao: Option<(((u64, u64), Setup), Arc<Vec<Item>>)>,
    eletrodutos: Option<(((u64, u64), Setup), Arc<Eletrodutos>)>,
    destaque: Option<(ChaveDestaque, Arc<Destaque>)>,
    materiais: Option<(((u64, u64), Setup), Arc<ele::materiais::Materiais>)>,
}

/// O que a aba Eletrodutos mostra.
#[derive(Default)]
struct Eletrodutos {
    trechos: Vec<ele::fiacao::Trecho>,
    soltos: Vec<ele::rota::Solto>,
}

/// A geometria do circuito destacado, em coordenadas do desenho.
#[derive(Default)]
struct Destaque {
    pontos: Vec<[f64; 2]>,
    trechos: Vec<Vec<[f64; 2]>>,
}

impl Estado {
    fn atualizar(&mut self, app: &CadApp) {
        let chave = app.session.state().ok().map(|st| (st.uid, st.revision));
        if chave == self.chave {
            return;
        }
        self.chave = chave;
        self.analise = Arc::new(app.session.state().map(|st| ele::analisar(&st.doc)).unwrap_or_default());
        self.blocos = Arc::new(app.session.state().map(|st| ele::biblioteca::blocos(&st.doc)).unwrap_or_default());
    }

    fn resumo(&mut self, app: &CadApp, s: &Setup, quadro: Handle) -> Option<Arc<Result<ele::ResumoQuadro, String>>> {
        let chave = (self.chave?, s.clone(), quadro);
        if let Some((k, r)) = &self.resumo
            && *k == chave
        {
            return Some(r.clone());
        }
        let st = app.session.state().ok()?;
        let r = Arc::new(ele::rotear_quadro(&st.doc, &self.analise, s, quadro));
        self.resumo = Some((chave, r.clone()));
        Some(r)
    }

    fn inspecao(&mut self, app: &CadApp, s: &Setup) -> Arc<Vec<Item>> {
        let Some(c) = self.chave else { return Arc::default() };
        let chave = (c, s.clone());
        if let Some((k, r)) = &self.inspecao
            && *k == chave
        {
            return r.clone();
        }
        let itens = app.session.state().map(|st| ele::inspecao::inspecionar(&st.doc, &self.analise, s)).unwrap_or_default();
        let r = Arc::new(itens);
        self.inspecao = Some((chave, r.clone()));
        r
    }

    fn materiais(&mut self, app: &CadApp, s: &Setup) -> Arc<ele::materiais::Materiais> {
        let Some(c) = self.chave else { return Arc::default() };
        let chave = (c, s.clone());
        if let Some((k, r)) = &self.materiais
            && *k == chave
        {
            return r.clone();
        }
        let m = app.session.state().map(|st| ele::materiais::levantar(&st.doc, &self.analise, s)).unwrap_or_default();
        let r = Arc::new(m);
        self.materiais = Some((chave, r.clone()));
        r
    }

    fn eletrodutos(&mut self, app: &CadApp, s: &Setup) -> Arc<Eletrodutos> {
        let Some(c) = self.chave else { return Arc::default() };
        let chave = (c, s.clone());
        if let Some((k, r)) = &self.eletrodutos
            && *k == chave
        {
            return r.clone();
        }
        let e = app.session.state().map_or_else(
            |_| Eletrodutos::default(),
            |st| {
                let (trechos, soltos, _) = ele::fiacao::todos_os_trechos(&st.doc, &self.analise, s);
                Eletrodutos { trechos, soltos }
            },
        );
        let r = Arc::new(e);
        self.eletrodutos = Some((chave, r.clone()));
        r
    }

    fn destaque(&mut self, app: &CadApp, s: &Setup, alvo: &(Handle, String)) -> Arc<Destaque> {
        let Some(c) = self.chave else { return Arc::default() };
        let chave = (c, s.clone(), alvo.clone());
        if let Some((k, r)) = &self.destaque
            && *k == chave
        {
            return r.clone();
        }
        let pontos = self
            .analise
            .pontos
            .iter()
            .filter(|p| p.quadro == Some(alvo.0) && p.circuito.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&alvo.1)))
            .map(|p| p.posicao)
            .collect();
        let mut d = Destaque { pontos, trechos: Vec::new() };
        if let Ok(st) = app.session.state()
            && let Ok(f) = ele::fiacao::calcular(&st.doc, &self.analise, s, alvo.0)
        {
            let tolerancia = 2.0 / app.canvas.xf.map_or(100.0, |x| x.scale.max(1e-9));
            for t in f.trechos.iter().filter(|t| t.circuitos.iter().any(|k| k.circuito.eq_ignore_ascii_case(&alvo.1))) {
                if let Some(e) = st.doc.model.get(t.handle)
                    && let Some(linha) = ele::rota::tracado(&e.kind, tolerancia)
                {
                    d.trechos.push(linha);
                }
            }
        }
        let r = Arc::new(d);
        self.destaque = Some((chave, r.clone()));
        r
    }
}

fn id_estado() -> egui::Id {
    egui::Id::new("free-eletrica-estado")
}

/// O estado lido do desenho, atualizado; [`guardar`] devolve depois de usar.
fn ler_estado(app: &CadApp, ctx: &egui::Context) -> Estado {
    let mut st: Estado = ctx.data_mut(|d| d.get_temp(id_estado())).unwrap_or_default();
    st.atualizar(app);
    st
}

fn guardar(ctx: &egui::Context, st: Estado) {
    ctx.data_mut(|d| d.insert_temp(id_estado(), st));
}

fn executar(app: &mut CadApp, acoes: Vec<Acao>) {
    for a in acoes {
        match a {
            Acao::Rodar(id, p) => {
                if let Err(e) = app.run(id, p) {
                    app.set_status(format!("Free Elétrica: {e}"));
                }
            }
            Acao::RodarComStatus(id, p, texto) => match app.run(id, p) {
                Ok(v) => app.set_status(texto(&v)),
                Err(e) => app.set_status(format!("Free Elétrica: {e}")),
            },
            Acao::Ficha(q, c) => {
                let mut j = trava(&JANELAS);
                j.cargas.quadro = Some(q);
                j.cargas.aba = AbaCargas::Ficha;
                j.cargas.selecionado = Some(c);
            }
            Acao::Fechar(q) => fechar(&mut trava(&JANELAS), q),
            Acao::Iniciar(id) => app.start(id),
            Acao::Palavra(k) => {
                if let Err(e) = app.session.input(cadcraft_engine::Input::Keyword(k.into())) {
                    app.session.echo(e.to_string());
                }
            }
            Acao::Selecionar(h) => app.session.set_selection(h),
            Acao::Localizar(pontos) => localizar(app, &pontos),
        }
    }
}

/// Enquadra os pontos na tela, com folga; um ponto só fica com uma vista de 10 m.
fn localizar(app: &mut CadApp, pontos: &[[f64; 2]]) {
    let Some(primeiro) = pontos.first() else { return };
    let (mut min, mut max) = (*primeiro, *primeiro);
    for p in pontos {
        min = [min[0].min(p[0]), min[1].min(p[1])];
        max = [max[0].max(p[0]), max[1].max(p[1])];
    }
    let (m, _) = setup().unidade.metros(app.session.doc().map_or(0, |d| d.header.i64("INSUNITS", 0)));
    let minimo = 10.0 / if m > 0.0 { m } else { 1.0 };
    let aspecto = app.canvas.rect.map_or(1.5, |r| f64::from(r.width() / r.height().max(1.0))).max(0.1);
    let altura = ((max[1] - min[1]).max((max[0] - min[0]) / aspecto) * 1.4).max(minimo);
    let centro = Vec2::new((min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0);
    if let Ok(st) = app.session.state_mut() {
        st.set_view(cadcraft_engine::View { center: centro, height: altura });
    }
}

/// A moldura das janelas: cantos redondos, como a mini-barra.
fn moldura_de_janela(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::window(&ctx.global_style()).corner_radius(8.0)
}

/// O cabeçalho dentro de uma janela sem barra de título. Devolve se pediram para fechar.
fn cabecalho_de_janela(ui: &mut egui::Ui, titulo: &str) -> bool {
    let mut fechar = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(titulo).strong().size(14.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
            let cor = if resp.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
            let c = r.shrink(5.0);
            let s = Stroke::new(1.3, cor);
            ui.painter().line_segment([c.left_top(), c.right_bottom()], s);
            ui.painter().line_segment([c.right_top(), c.left_bottom()], s);
            crate::icons::describe_control(ui, &resp, "Fechar", None, false);
            fechar = resp.on_hover_text("Fechar (Esc)").clicked();
        });
    });
    ui.separator();
    fechar
}

/// Uma janela do Free Elétrica: sem barra de título, com cabeçalho interno. Devolve se continua aberta.
fn janela(ctx: &egui::Context, titulo: &str, pos: (f32, f32), tamanho: (f32, f32), conteudo: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut aberta = true;
    egui::Window::new(format!("Free Elétrica · {titulo}"))
        .id(egui::Id::new(("ele-janela", titulo)))
        .title_bar(false)
        .frame(moldura_de_janela(ctx))
        .default_pos(egui::pos2(pos.0, pos.1))
        .default_width(tamanho.0)
        .default_height(tamanho.1)
        .show(ctx, |ui| {
            if cabecalho_de_janela(ui, titulo) {
                aberta = false;
            }
            conteudo(ui);
        });
    aberta
}

/// Chamado a cada quadro, com ou sem a paleta visível: a abertura na primeira vez e as janelas.
pub fn janelas(app: &mut CadApp, ctx: &egui::Context) {
    if ABRIR_AO_INICIAR.swap(false, Ordering::Relaxed) {
        let panel = Panel::Eletrica;
        if !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel) {
            let _ = open(app);
        }
    }
    // O painel de baixo não existe mais: um workspace salvo com ele aberto o perde aqui.
    if app.ui.docking.layout.contains(&Panel::QuadroCargas) {
        let _ = crate::docking::command(app, false, &json!({ "operation": "close", "panel": Panel::QuadroCargas.id() }));
    }
    // Esc fecha a última janela aberta. Com um comando em andamento, Esc é dele; com um menu aberto
    // ou um campo em edição, é deles.
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if esc && app.session.running.is_none() && !ctx.egui_wants_keyboard_input() && !egui::Popup::is_any_open(ctx) {
        let mut j = trava(&JANELAS);
        if let Some(q) = j.abertas.last().copied() {
            fechar(&mut j, q);
        }
    }
    let (abertas, mut cargas, mut pontos, mut destaque, mut dimensionar, mut blocos) = {
        let j = trava(&JANELAS);
        (j.abertas.clone(), j.cargas.clone(), j.pontos.clone(), j.destaque.clone(), j.dimensionar.clone(), j.blocos.clone())
    };
    if abertas.is_empty() {
        return;
    }
    let t = Tokens::get();
    let mut st = ler_estado(app, ctx);
    let s = setup();
    let tem_desenho = st.chave.is_some();
    let mut acoes: Vec<Acao> = Vec::new();
    let mut fechadas: Vec<Qual> = Vec::new();
    for q in abertas {
        let aberta = match q {
            Qual::Setup => {
                janela_setup(ctx, &t, tem_desenho, &mut acoes);
                trava(&JANELAS).setup.is_some()
            }
            Qual::Cargas => janela(ctx, "Quadro de cargas", (240.0, 110.0), (1340.0, 460.0), |ui| {
                if tem_desenho {
                    aba_cargas(ui, app, &t, &mut st, &s, &mut cargas, &mut destaque, &mut acoes);
                } else {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                }
            }),
            Qual::Dimensionar => janela(ctx, "Dimensionar projeto", (520.0, 170.0), (440.0, 300.0), |ui| {
                janela_dimensionar(ui, &t, &mut dimensionar, tem_desenho, &mut acoes)
            }),
            Qual::Blocos => janela(ctx, "Blocos e famílias", (300.0, 130.0), (720.0, 460.0), |ui| {
                janela_blocos(ui, &t, &st, &mut blocos, tem_desenho, &mut acoes)
            }),
            Qual::Atributos => janela(ctx, "Inspetor de atributos", (560.0, 140.0), (540.0, 400.0), |ui| janela_atributos(ui, app, &t, &mut acoes)),
            Qual::Eletrodutos => {
                janela(ctx, "Lista de eletrodutos", (280.0, 140.0), (980.0, 420.0), |ui| aba_lista_eletrodutos(ui, app, &t, &mut st, &s, &mut acoes))
            }
            Qual::Pontos => {
                janela(ctx, "Lista de pontos", (300.0, 160.0), (860.0, 420.0), |ui| lista_de_pontos(ui, &t, &st.analise, &s, &mut pontos, &mut acoes))
            }
            Qual::Conferencia => janela(ctx, "Conferência do projeto", (480.0, 140.0), (520.0, 460.0), |ui| {
                let itens = st.inspecao(app, &s);
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| aba_conferencia(ui, &t, &st.analise, &itens, &mut acoes));
            }),
            Qual::Materiais => janela(ctx, "Lista de materiais", (420.0, 120.0), (620.0, 520.0), |ui| {
                let m = st.materiais(app, &s);
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| lista_de_materiais(ui, &t, &m));
            }),
        };
        if !aberta {
            fechadas.push(q);
        }
    }
    {
        let mut j = trava(&JANELAS);
        j.cargas = cargas;
        j.pontos = pontos;
        j.destaque = destaque;
        j.dimensionar = dimensionar;
        j.blocos = blocos;
        for q in fechadas {
            fechar(&mut j, q);
        }
    }
    guardar(ctx, st);
    executar(app, acoes);
}

// ---------------------------------------------------------------- zonas 2 e 4: sobre o desenho

/// Desenha, sobre o desenho no espaço do modelo, o destaque do circuito, os halos da inspeção e a
/// barra de contexto do comando em andamento.
pub fn sobre_canvas(app: &mut CadApp, ui: &mut egui::Ui, painter: &egui::Painter, xf: &crate::canvas::Xf) {
    let (inspecao, destaque) = {
        let j = trava(&JANELAS);
        (j.inspecao, j.destaque.clone())
    };
    let comando = app.session.running.as_ref().map(|r| r.id.clone());
    let barra = comando.as_deref().filter(|c| matches!(*c, "eleponto" | "eleeletroduto" | "elecircuito")).map(str::to_string);
    if !inspecao && destaque.is_none() && barra.is_none() {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut st = ler_estado(app, &ctx);
    let s = setup();
    if let Some(alvo) = &destaque {
        let d = st.destaque(app, &s, alvo);
        desenhar_destaque(painter, xf, &d);
    }
    if inspecao {
        let itens = st.inspecao(app, &s);
        desenhar_inspecao(painter, xf, &itens, ctx.pointer_hover_pos());
        legenda_inspecao(&ctx, xf, &itens);
    }
    let mut acoes = Vec::new();
    if let Some(c) = barra {
        barra_de_contexto(app, &ctx, xf, &c, &st, &s, &mut acoes);
    }
    guardar(&ctx, st);
    executar(app, acoes);
}

fn na_tela(xf: &crate::canvas::Xf, p: [f64; 2]) -> egui::Pos2 {
    xf.to_screen(Vec2::new(p[0], p[1]))
}

fn desenhar_destaque(painter: &egui::Painter, xf: &crate::canvas::Xf, d: &Destaque) {
    for t in &d.trechos {
        let linha: Vec<egui::Pos2> = t.iter().map(|p| na_tela(xf, *p)).collect();
        painter.add(egui::Shape::line(linha, Stroke::new(3.0, COR_DESTAQUE_TRECHO)));
    }
    for p in &d.pontos {
        painter.circle_stroke(na_tela(xf, *p), 11.0, Stroke::new(2.5, COR_DESTAQUE_PONTO));
    }
}

/// A cor de um nível da inspeção.
fn cor_do_nivel(n: Nivel) -> Color32 {
    match n {
        Nivel::Vermelho => Color32::from_rgb(235, 64, 52),
        Nivel::Laranja => Color32::from_rgb(245, 140, 30),
        Nivel::Amarelo => Color32::from_rgb(250, 215, 60),
        Nivel::Verde => Color32::from_rgb(70, 190, 100),
    }
}

fn desenhar_inspecao(painter: &egui::Painter, xf: &crate::canvas::Xf, itens: &[Item], cursor: Option<egui::Pos2>) {
    let mut sob_o_cursor: Option<&Item> = None;
    // Do menos grave ao mais grave: o mais grave fica por cima.
    for i in itens.iter().rev() {
        let c = na_tela(xf, i.pos);
        if !painter.clip_rect().contains(c) {
            continue;
        }
        let cor = cor_do_nivel(i.nivel);
        if i.nivel == Nivel::Verde {
            painter.circle_stroke(c, 8.0, Stroke::new(1.5, cor));
        } else {
            painter.circle_filled(c, 13.0, cor.gamma_multiply(0.28));
            painter.circle_stroke(c, 13.0, Stroke::new(2.0, cor));
        }
        if cursor.is_some_and(|p| p.distance(c) <= 14.0) {
            sob_o_cursor = Some(i);
        }
    }
    if let (Some(i), Some(p)) = (sob_o_cursor, cursor) {
        let fonte = crate::theme::small();
        let galley = painter.layout(i.motivo.clone(), fonte, Color32::WHITE, 320.0);
        let canto = p + vec2(16.0, 16.0);
        let fundo = egui::Rect::from_min_size(canto, galley.size()).expand(4.0);
        painter.rect_filled(fundo, 3.0, Color32::from_black_alpha(220));
        painter.rect_stroke(fundo, 3.0, Stroke::new(1.0, cor_do_nivel(i.nivel)), egui::StrokeKind::Outside);
        painter.galley(canto, galley, Color32::WHITE);
    }
}

/// As contagens da inspeção, no canto de baixo do desenho.
fn legenda_inspecao(ctx: &egui::Context, xf: &crate::canvas::Xf, itens: &[Item]) {
    let conta = |n: Nivel| itens.iter().filter(|i| i.nivel == n).count();
    egui::Area::new(egui::Id::new("ele-legenda-inspecao")).order(egui::Order::Middle).fixed_pos(xf.rect.left_bottom() + vec2(8.0, -36.0)).show(
        ctx,
        |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Inspeção").strong());
                    for (n, nome) in [
                        (Nivel::Vermelho, "falta dado"),
                        (Nivel::Laranja, "sem caminho"),
                        (Nivel::Amarelo, "acima do limite"),
                        (Nivel::Verde, "válido"),
                    ] {
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 5.0, cor_do_nivel(n));
                        ui.label(format!("{} {nome}", conta(n)));
                    }
                });
            });
        },
    );
}

/// O bloco de cada tipo da barra de inserção: (rótulo, bloco).
const TIPOS_BARRA: [(&str, &str); 13] = [
    ("TUG", "TOM-SIM-BAIXA"),
    ("TUE", "TOM-TUE"),
    ("Iluminação", "ILU-OCT"),
    ("Interruptor 1S", "INT-SIM"),
    ("Interruptor 2S", "INT-2S"),
    ("Interruptor 3S", "INT-3S"),
    ("Paralelo", "INT-PAR"),
    ("Intermediário", "INT-INT"),
    ("Quadro QDG", "QDG"),
    ("Quadro QDC", "QDC"),
    ("Ponto de dados", "RJ45"),
    ("Rack", "RACK"),
    ("Caixa de passagem", "CX-PASSAGEM"),
];

/// As alturas prontas: (nome, valor). Teto usa o pé-direito.
fn alturas(s: &Setup) -> [(&'static str, f64); 5] {
    [("Piso", 0.0), ("Baixa", 0.30), ("Média", 1.20), ("Alta", 2.10), ("Teto", s.pe_direito)]
}

/// A barra sobre o desenho, enquanto o comando espera cliques. Não rouba o comando: o que ela muda
/// vale no próximo clique.
fn barra_de_contexto(app: &CadApp, ctx: &egui::Context, xf: &crate::canvas::Xf, comando: &str, st: &Estado, s: &Setup, acoes: &mut Vec<Acao>) {
    let Ok(uid) = app.session.state().map(|d| d.uid) else { return };
    egui::Area::new(egui::Id::new("ele-barra-contexto")).order(egui::Order::Middle).fixed_pos(xf.rect.left_top() + vec2(8.0, 8.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width((xf.rect.width() - 32.0).max(120.0));
            ui.horizontal_wrapped(|ui| match comando {
                "eleponto" => {
                    let mut filtro = trava(&JANELAS).filtro_barra;
                    barra_ponto(ui, uid, st, s, &mut filtro, acoes);
                    trava(&JANELAS).filtro_barra = filtro;
                }
                "eleeletroduto" => barra_eletroduto(ui, uid, acoes),
                _ => barra_circuito(ui, uid, st, acoes),
            });
        });
    });
}

fn barra_ponto(ui: &mut egui::Ui, uid: u64, st: &Estado, s: &Setup, filtro: &mut FiltroPontos, acoes: &mut Vec<Acao>) {
    let Some(m) = ele::biblioteca::modelo(uid) else { return };
    let Some(bloco) = st.blocos.iter().find(|b| b.nome.eq_ignore_ascii_case(&m.bloco)) else { return };
    let tem = |tag: &str| bloco.atributos.iter().any(|a| !a.constante && a.tag.eq_ignore_ascii_case(tag));
    let padrao = |tag: &str| bloco.atributos.iter().find(|a| a.tag.eq_ignore_ascii_case(tag)).map_or(String::new(), |a| a.padrao.clone());
    let valor = |tag: &str| m.valores.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map_or_else(|| padrao(tag), |(_, v)| v.clone());
    let enviar = |acoes: &mut Vec<Acao>, nome: &str, valores: &[(String, String)], quadro: Option<Handle>| {
        let mapa: serde_json::Map<String, Value> = valores.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
        acoes.push(Acao::Rodar(
            "eletrica.ponto.modelo",
            json!({ "bloco": nome, "rotacao": m.rotacao, "rotacaoAuto": m.rotacao_auto, "valores": mapa, "quadro": quadro.map(|h| h.hex()) }),
        ));
    };
    let com = |tag: &str, v: String| -> Vec<(String, String)> {
        let mut l: Vec<(String, String)> = m.valores.iter().filter(|(t, _)| !t.eq_ignore_ascii_case(tag)).cloned().collect();
        l.push((tag.to_string(), v));
        l
    };

    ui.label("Categoria");
    egui::ComboBox::from_id_salt("ele-barra-filtro").selected_text(filtro.nome()).width(110.0).show_ui(ui, |ui| {
        for f in FiltroPontos::TODOS {
            if ui.selectable_label(*filtro == f, f.nome()).clicked() {
                *filtro = f;
                // O bloco acompanha a categoria: o próximo clique já é dela.
                if !f.aceita(bloco.tipo)
                    && let Some(b) = st.blocos.iter().find(|b| f.aceita(b.tipo))
                {
                    enviar(acoes, &b.nome, &[], m.quadro);
                }
            }
        }
    });
    ui.label("Bloco");
    let nome_de = |nome: &str| TIPOS_BARRA.iter().find(|(_, b)| b.eq_ignore_ascii_case(nome)).map_or(nome, |(n, _)| n).to_string();
    egui::ComboBox::from_id_salt("ele-barra-tipo").selected_text(nome_de(&bloco.nome)).width(140.0).show_ui(ui, |ui| {
        for b in st.blocos.iter().filter(|b| filtro.aceita(b.tipo)) {
            if ui.selectable_label(bloco.nome == b.nome, nome_de(&b.nome)).on_hover_text(b.nome.as_str()).clicked() {
                enviar(acoes, &b.nome, &[], m.quadro);
            }
        }
    });
    if tem("POTENCIA") && bloco.tipo != TipoPonto::Interruptor {
        ui.separator();
        ui.label("Potência");
        let mut va = ele::ponto::numero(valor("POTENCIA").trim_end_matches(|c: char| c.is_alphabetic()).trim()).unwrap_or(0.0);
        if ui.add(campo(&mut va, 0.0..=100_000.0, 10.0, 0, " VA")).changed() {
            enviar(acoes, &bloco.nome, &com("POTENCIA", format!("{}VA", fixo(va, 0))), m.quadro);
        }
    }
    ui.separator();
    ui.label("Altura");
    let legenda = bloco.altura_padrao(s);
    let mut altura = Some(valor("ALTURA")).filter(|v| !v.trim().is_empty()).and_then(|v| ele::ponto::altura_m(&v)).unwrap_or(legenda);
    if ui.add(campo(&mut altura, 0.0..=50.0, 0.01, 2, " m")).on_hover_text(format!("Legenda do projeto: {} m", fixo(legenda, 2))).changed() {
        enviar(acoes, &bloco.nome, &com("ALTURA", fixo(altura, 2)), m.quadro);
    }
    let atual = alturas(s).into_iter().find(|(_, a)| (a - altura).abs() < 0.005).map_or("Outra", |(n, _)| n);
    egui::ComboBox::from_id_salt("ele-barra-altura").selected_text(atual).width(64.0).show_ui(ui, |ui| {
        for (nome, a) in alturas(s) {
            if ui.selectable_label((a - altura).abs() < 0.005, format!("{nome} ({} m)", fixo(a, 2))).clicked() {
                enviar(acoes, &bloco.nome, &com("ALTURA", fixo(a, 2)), m.quadro);
            }
        }
    });
    if (altura - legenda).abs() >= 0.005 {
        ui.label(RichText::new(ele::editar::texto_de_altura(altura)).weak());
    }
    if bloco.tipo.e_de_forca() && bloco.tipo != TipoPonto::Quadro {
        ui.separator();
        ui.label("Quadro pai");
        let quadros = st.analise.quadros();
        let atual = m.quadro.and_then(|h| quadros.iter().find(|q| q.handle == h)).map_or("Automático".to_string(), |q| q.rotulo());
        egui::ComboBox::from_id_salt("ele-barra-quadro").selected_text(atual).width(110.0).show_ui(ui, |ui| {
            if ui.selectable_label(m.quadro.is_none(), "Automático").on_hover_text("O único quadro do desenho, ou o do circuito ao criá-lo").clicked()
            {
                enviar(acoes, &bloco.nome, &m.valores, None);
            }
            for q in &quadros {
                if ui.selectable_label(m.quadro == Some(q.handle), q.rotulo()).clicked() {
                    enviar(acoes, &bloco.nome, &m.valores, Some(q.handle));
                }
            }
        });
    }
}

fn barra_eletroduto(ui: &mut egui::Ui, uid: u64, acoes: &mut Vec<Acao>) {
    let m = ele::eletroduto::modelo(uid);
    ui.label("Traçado");
    for tr in ele::eletroduto::Tracado::TODOS {
        if ui.selectable_label(m.tracado == tr, tr.nome()).clicked() {
            acoes.push(Acao::Rodar("eletrica.eletroduto.modelo", json!({ "tracado": tr.codigo() })));
        }
    }
    ui.separator();
    ui.label("Cota");
    for i in Instalacao::TODAS {
        if ui.selectable_label(m.instalacao == i, i.nome()).clicked() {
            acoes.push(Acao::Rodar("eletrica.eletroduto.modelo", json!({ "instalacao": i.codigo() })));
        }
    }
    ui.separator();
    ui.label("Tipo");
    egui::ComboBox::from_id_salt("ele-barra-duto").selected_text(m.tipo.nome()).width(290.0).show_ui(ui, |ui| {
        for t in TipoEletroduto::TODOS {
            if ui.selectable_label(m.tipo == t, t.nome()).clicked() {
                acoes.push(Acao::Rodar("eletrica.eletroduto.modelo", json!({ "tipo": t.codigo() })));
            }
        }
    });
    if m.tracado == ele::eletroduto::Tracado::Arco {
        ui.separator();
        ui.label("Flecha");
        let mut f = m.flecha;
        if ui
            .add(campo(&mut f, 0.0..=ele::eletroduto::FLECHA_MAXIMA, 0.01, 2, ""))
            .on_hover_text("Curvatura como fração do comprimento; 0 é reto")
            .changed()
        {
            acoes.push(Acao::Rodar("eletrica.eletroduto.modelo", json!({ "flecha": f })));
        }
    }
    if m.tracado != ele::eletroduto::Tracado::Reto && ui.button("Inverter").on_hover_text("Curva ou canto para o outro lado").clicked() {
        acoes.push(Acao::Palavra("Inverter"));
    }
    ui.separator();
    ui.label(RichText::new("Clique no vazio: desvio").weak())
        .on_hover_text("Um clique fora de blocos e eletrodutos vira um ponto de passagem do trajeto");
}

fn barra_circuito(ui: &mut egui::Ui, uid: u64, st: &Estado, acoes: &mut Vec<Acao>) {
    let Some((q, id)) = ele::circuito::ativo(uid) else {
        ui.label("Selecione o quadro e informe o número do circuito.");
        return;
    };
    let Some(quadro) = st.analise.ponto(q) else { return };
    let props = ele::quadro::propriedades(quadro);
    let p = props.iter().find(|(k, _)| k.eq_ignore_ascii_case(&id)).map(|(_, v)| v.clone()).unwrap_or_default();
    let gravada = ele::quadro::descricoes(quadro).into_iter().find(|(k, _)| k.eq_ignore_ascii_case(&id)).map_or(String::new(), |(_, d)| d);
    ui.label(RichText::new(format!("Circuito {id} · {}", quadro.rotulo())).strong());
    ui.separator();
    ui.label("Descrição");
    // O texto em edição fica na memória da barra até sair do campo.
    let chave = egui::Id::new(("ele-barra-descricao", q, id.as_str()));
    let mut texto: String = ui.ctx().data_mut(|d| d.get_temp(chave)).unwrap_or_else(|| gravada.clone());
    let r = ui.add(egui::TextEdit::singleline(&mut texto).hint_text("Iluminação social").desired_width(180.0));
    if r.lost_focus() && texto.trim() != gravada {
        acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "descricao": texto.trim() })));
    }
    ui.ctx().data_mut(|d| d.insert_temp(chave, texto));
    ui.separator();
    ui.label("Esquema");
    let atual = p.esquema.map_or("Automático", Esquema::codigo);
    egui::ComboBox::from_id_salt("ele-barra-esquema").selected_text(atual).width(90.0).show_ui(ui, |ui| {
        if ui.selectable_label(p.esquema.is_none(), "Automático").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "esquema": null })));
        }
        for e in Esquema::TODOS {
            if ui.selectable_label(p.esquema == Some(e), e.codigo()).clicked() {
                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "esquema": e.codigo() })));
            }
        }
    });
    ui.separator();
    ui.label("Tensão");
    let atual = p.tensao.map_or("Dos pontos".to_string(), |t| format!("{} V", fixo(t, 0)));
    egui::ComboBox::from_id_salt("ele-barra-tensao").selected_text(atual).width(90.0).show_ui(ui, |ui| {
        if ui.selectable_label(p.tensao.is_none(), "Dos pontos").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "tensao": null })));
        }
        for v in [127.0, 220.0, 380.0] {
            if ui.selectable_label(p.tensao == Some(v), format!("{} V", fixo(v, 0))).clicked() {
                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "tensao": v })));
            }
        }
    });
}

// ---------------------------------------------------------------- zona 3: as janelas

/// O nome de um quadro de cargas e os botões gerais; a tabela vem de `quadro_de_cargas`.
#[allow(clippy::too_many_arguments)]
fn aba_cargas(
    ui: &mut egui::Ui,
    app: &CadApp,
    t: &Tokens,
    st: &mut Estado,
    s: &Setup,
    c: &mut Cargas,
    destaque: &mut Option<(Handle, String)>,
    acoes: &mut Vec<Acao>,
) {
    if st.analise.quadros().is_empty() {
        ui.label(pequeno("Nenhum quadro no desenho: insira um com Inserir pontos, categoria Quadros/Racks.", t.text_dim));
        return;
    }
    let analise = st.analise.clone();
    ui.horizontal(|ui| {
        if seletor_quadro(ui, "ele-cargas", &analise, &mut c.quadro) {
            c.descricoes.clear();
            c.excluir = None;
        }
    });
    let Some(q) = c.quadro else { return };
    ui.horizontal(|ui| {
        for (aba, nome) in [(AbaCargas::Resumo, "Resumo"), (AbaCargas::Ficha, "Ficha de engenharia")] {
            if ui.selectable_label(c.aba == aba, nome).clicked() {
                c.aba = aba;
            }
        }
    });
    ui.separator();
    let Some(resumo) = st.resumo(app, s, q) else { return };
    match resumo.as_ref() {
        Err(e) => {
            ui.label(pequeno(e.as_str(), COR_AVISO));
        }
        Ok(r) if c.aba == AbaCargas::Resumo => {
            egui::ScrollArea::both()
                .id_salt("ele-cargas-rolagem")
                .auto_shrink([false, true])
                .show(ui, |ui| quadro_de_cargas(ui, t, &analise, s, r, c, destaque, acoes));
        }
        Ok(r) => ficha_de_engenharia(ui, t, s, r, c, acoes),
    }
}

/// A aba da ficha: a lista dos circuitos e a engenharia do escolhido.
fn ficha_de_engenharia(ui: &mut egui::Ui, t: &Tokens, s: &Setup, r: &ele::ResumoQuadro, c: &mut Cargas, acoes: &mut Vec<Acao>) {
    if r.linhas.is_empty() {
        ui.label(pequeno("Nenhum circuito com pontos neste quadro.", t.text_dim));
        return;
    }
    if !c.selecionado.as_ref().is_some_and(|id| r.linhas.iter().any(|l| l.circuito.id.eq_ignore_ascii_case(id))) {
        c.selecionado = r.linhas.first().map(|l| l.circuito.id.clone());
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(190.0);
            egui::ScrollArea::vertical().id_salt("ele-ficha-lista").auto_shrink([false, true]).show(ui, |ui| {
                for l in &r.linhas {
                    let id = &l.circuito.id;
                    let nome = if l.descricao.is_empty() { format!("Circuito {id}") } else { format!("{id}  {}", l.descricao) };
                    if ui.selectable_label(c.selecionado.as_deref() == Some(id.as_str()), nome).clicked() {
                        c.selecionado = Some(id.clone());
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            egui::ScrollArea::vertical().id_salt("ele-ficha-detalhe").auto_shrink([false, true]).show(ui, |ui| {
                if let Some(l) = r.linhas.iter().find(|l| c.selecionado.as_deref().is_some_and(|id| l.circuito.id.eq_ignore_ascii_case(id))) {
                    detalhes_do_circuito(ui, s, r.quadro, l, acoes);
                }
            });
        });
    });
}

/// A janela do dimensionamento: as premissas e o botão que processa.
fn janela_dimensionar(ui: &mut egui::Ui, t: &Tokens, o: &mut OpcoesDimensionar, tem_desenho: bool, acoes: &mut Vec<Acao>) {
    ui.label(pequeno(
        "Lançar pontos, circuitos e eletrodutos não muda o dimensionamento. O quadro de cargas, as tags e as tabelas recebem o resultado aqui.",
        t.text_dim,
    ));
    ui.add_space(4.0);
    ui.checkbox(&mut o.compartilhar_pe, "Compartilhar o condutor de proteção (PE)")
        .on_hover_text("Circuitos no mesmo eletroduto dividem um PE, o de maior seção entre eles (NBR 5410, 6.4.3.1). Confira a seção resultante");
    egui::Grid::new("ele-dimensionar").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Temperatura ambiente (FCT)");
        ui.add(campo(&mut o.temperatura, 10.0..=60.0, 1.0, 0, " °C"));
        ui.end_row();
        ui.checkbox(&mut o.fca_manual, "FCA fixo").on_hover_text("Sem marcar, vale o FCA real de cada trecho (tabela 42)");
        if o.fca_manual {
            ui.add(campo(&mut o.fca, 0.1..=1.0, 0.01, 2, ""));
        } else {
            ui.label(pequeno("real, por trecho", t.text_dim));
        }
        ui.end_row();
    });
    ui.add_space(6.0);
    if ui.add_enabled(tem_desenho, egui::Button::new(RichText::new("Processar cálculo").strong()).min_size(vec2(200.0, 28.0))).clicked() {
        acoes.push(Acao::RodarComStatus(
            "eletrica.dimensionar",
            json!({ "compartilharPe": o.compartilhar_pe, "temperaturaAmbiente": o.temperatura, "fcaManual": if o.fca_manual { json!(o.fca) } else { Value::Null } }),
            |v| format!("Projeto dimensionado: {} circuito(s), {} aviso(s)", v["circuitos"], v["avisos"]),
        ));
    }
}

/// A janela de blocos: criar um novo e editar os do desenho.
fn janela_blocos(ui: &mut egui::Ui, t: &Tokens, st: &Estado, b: &mut Blocos, tem_desenho: bool, acoes: &mut Vec<Acao>) {
    use ele::familia::FamilyCategory;
    ui.horizontal(|ui| {
        let rotulo = ui.label("Nome do bloco");
        ui.add(egui::TextEdit::singleline(&mut b.nome).desired_width(170.0)).labelled_by(rotulo.id);
        let categoria = b.categoria.unwrap_or_default();
        egui::ComboBox::from_id_salt("ele-blocos-categoria").selected_text(categoria.nome()).width(190.0).show_ui(ui, |ui| {
            for c in FamilyCategory::TODAS.into_iter().filter(|c| *c != FamilyCategory::Nenhum) {
                if ui.selectable_label(categoria == c, c.nome()).clicked() {
                    b.categoria = Some(c);
                }
            }
        });
        let nome = b.nome.trim().to_string();
        let existe = st.blocos.iter().any(|x| x.nome.eq_ignore_ascii_case(&nome));
        let pronto = tem_desenho && !nome.is_empty() && !existe;
        let botao = egui::Button::new(RichText::new("+ Criar novo bloco elétrico").strong().color(Color32::WHITE))
            .fill(Tokens::get().accent)
            .min_size(vec2(210.0, 26.0));
        let dica = if existe {
            "Já existe um bloco com esse nome: use Editar na lista"
        } else {
            "Abre o editor de blocos já com a família e os atributos prontos"
        };
        if ui
            .add_enabled(pronto, botao)
            .on_hover_text(dica)
            .on_disabled_hover_text(if nome.is_empty() { "Digite o nome do bloco" } else { dica })
            .clicked()
        {
            acoes.push(Acao::Rodar("eletrica.bootstrap", json!({})));
            acoes.push(Acao::Rodar("bedit", json!({ "name": nome, "create": true })));
            acoes.push(Acao::Rodar("eletrica.familia.definir", json!({ "categoria": categoria.codigo() })));
            acoes.push(Acao::Rodar("eletrica.familia.atributos", json!({})));
            acoes.push(Acao::Fechar(Qual::Blocos));
            b.nome.clear();
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for f in FiltroPontos::TODOS {
            if ui.selectable_label(b.filtro == f, f.nome()).clicked() {
                b.filtro = f;
            }
        }
    });
    let colunas: [(&str, f32, bool); 5] =
        [("Bloco", 170.0, false), ("Categoria", 190.0, false), ("Caixa", 90.0, false), ("No desenho", 70.0, true), ("", 130.0, false)];
    egui::ScrollArea::vertical().id_salt("ele-blocos-lista").auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("ele-blocos-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
            cabecalho(ui, t, &colunas);
            for x in st.blocos.iter().filter(|x| b.filtro.aceita(x.tipo)) {
                celula(ui, colunas[0].1, x.nome.as_str(), false);
                let categoria = x.familia.as_ref().map_or_else(|| format!("{} (sem família)", x.tipo.nome()), |f| f.categoria.nome().to_string());
                celula(ui, colunas[1].1, categoria, false);
                celula(ui, colunas[2].1, x.familia.as_ref().map_or("-", |f| f.caixa.nome()), false);
                celula(ui, colunas[3].1, numero(x.inseridos.to_string()), true);
                ui.horizontal(|ui| {
                    if ui.small_button("Editar").on_hover_text("Abre o editor de blocos, com a família na paleta").clicked() {
                        acoes.push(Acao::Rodar("bedit", json!({ "name": x.nome })));
                        acoes.push(Acao::Fechar(Qual::Blocos));
                    }
                    if ui.small_button("Inserir").clicked() {
                        acoes.push(Acao::Rodar("eletrica.ponto.modelo", json!({ "bloco": x.nome })));
                        acoes.push(Acao::Iniciar("eleponto"));
                        acoes.push(Acao::Fechar(Qual::Blocos));
                    }
                });
                ui.end_row();
            }
        });
    });
}

/// A classe e o fator de demanda de um ponto de carga, no inspetor de atributos.
fn campos_de_demanda(ui: &mut egui::Ui, p: &PontoEletrico, acoes: &mut Vec<Acao>) {
    let h = p.handle.hex();
    egui::Grid::new("ele-atributos-demanda").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
        ui.label("Classe de Demanda");
        let atual = p.tipo_carga_efetivo();
        egui::ComboBox::from_id_salt("ele-ponto-classe").selected_text(atual.nome_classe()).width(340.0).show_ui(ui, |ui| {
            if ui.selectable_label(false, "Automática (a da família)").clicked() {
                acoes.push(Acao::Rodar("eletrica.ponto.demanda", json!({ "pontos": [h], "classe": null })));
            }
            for c in ele::demanda::TipoCarga::CLASSES {
                if ui.selectable_label(atual == c, c.nome_classe()).clicked() {
                    acoes.push(Acao::Rodar("eletrica.ponto.demanda", json!({ "pontos": [h], "classe": c.codigo() })));
                }
            }
        });
        ui.end_row();
        ui.label("Fator de Demanda (0,00 a 1,00)");
        ui.horizontal(|ui| {
            let mut automatico = p.fator_demanda.is_none();
            if ui.checkbox(&mut automatico, "Automático, pela tabela").changed() {
                acoes.push(Acao::Rodar(
                    "eletrica.ponto.demanda",
                    json!({ "pontos": [h], "fator": if automatico { Value::Null } else { json!(1.0) } }),
                ));
            }
            if let Some(mut f) = p.fator_demanda
                && ui.add(campo(&mut f, 0.0..=1.0, 0.01, 2, "")).changed()
            {
                acoes.push(Acao::Rodar("eletrica.ponto.demanda", json!({ "pontos": [h], "fator": f })));
            }
        });
        ui.end_row();
    });
}

/// O inspetor de atributos do bloco selecionado.
fn janela_atributos(ui: &mut egui::Ui, app: &CadApp, t: &Tokens, acoes: &mut Vec<Acao>) {
    let Ok(doc) = app.session.doc() else {
        ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
        return;
    };
    let Some((h, lista)) = app.session.selection().into_iter().find_map(|h| ele::editar::atributos_de(doc, h).map(|l| (h, l))) else {
        ui.label(pequeno("Selecione um bloco no desenho para ver os atributos dele.", t.text_dim));
        return;
    };
    let nome = match doc.model.get(h).map(|e| &e.kind) {
        Some(cadcraft_doc::EntityKind::Insert(i)) => i.block.clone(),
        _ => String::new(),
    };
    ui.label(RichText::new(format!("{nome} #{}", h.hex())).strong());
    if let Some(p) = ele::ponto::ler_ponto(doc, h).filter(|p| p.tipo.e_carga()) {
        campos_de_demanda(ui, &p, acoes);
        ui.separator();
    }
    if lista.is_empty() {
        ui.label(pequeno("Este bloco não tem atributos.", t.text_dim));
        return;
    }
    egui::ScrollArea::vertical().id_salt("ele-atributos").auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("ele-atributos-tabela").num_columns(5).striped(true).spacing(vec2(8.0, 3.0)).show(ui, |ui| {
            cabecalho(
                ui,
                t,
                &[("Atributo", 110.0, false), ("Valor", 150.0, false), ("Visível", 44.0, false), ("X", 70.0, false), ("Y", 70.0, false)],
            );
            for a in &lista {
                celula(ui, 110.0, a.tag.as_str(), false);
                if a.constante {
                    celula(ui, 150.0, pequeno(format!("{} (da definição)", a.valor), t.text_dim), false);
                    celula(ui, 44.0, "", false);
                    celula(ui, 70.0, "", false);
                    celula(ui, 70.0, "", false);
                    ui.end_row();
                    continue;
                }
                let enviar = |acoes: &mut Vec<Acao>, campo: &str, valor: Value| {
                    let mut p = serde_json::Map::new();
                    p.insert("ponto".into(), json!(h.hex()));
                    p.insert("tag".into(), json!(a.tag));
                    p.insert(campo.into(), valor);
                    acoes.push(Acao::Rodar("eletrica.atributo.definir", Value::Object(p)));
                };
                // O texto em edição fica na memória da janela até sair do campo.
                let chave = egui::Id::new(("ele-atributo", h, a.tag.as_str(), a.valor.as_str()));
                let mut texto: String = ui.ctx().data_mut(|d| d.get_temp(chave)).unwrap_or_else(|| a.valor.clone());
                let r = ui.add_sized(vec2(150.0, LINHA), egui::TextEdit::singleline(&mut texto));
                if r.lost_focus() && texto != a.valor {
                    enviar(acoes, "valor", json!(texto));
                }
                ui.ctx().data_mut(|d| d.insert_temp(chave, texto));
                let mut visivel = a.visivel;
                if ui.checkbox(&mut visivel, "").changed() {
                    enviar(acoes, "visivel", json!(visivel));
                }
                let (mut x, mut y) = (a.x, a.y);
                let mudou_x = ui.add_sized(vec2(70.0, LINHA), campo(&mut x, -1e9..=1e9, 0.01, 3, "")).changed();
                let mudou_y = ui.add_sized(vec2(70.0, LINHA), campo(&mut y, -1e9..=1e9, 0.01, 3, "")).changed();
                if mudou_x || mudou_y {
                    enviar(acoes, "posicao", json!([x, y]));
                }
                ui.end_row();
            }
        });
    });
    ui.label(pequeno("A altura segue a legenda do projeto: vazio volta a ela, e outro valor aparece como h=1,45m.", t.text_dim));
}

/// Uma coluna da tabela: título, largura e se o conteúdo é número (à direita).
type Coluna = (&'static str, f32, bool);

/// As opções de fases travadas para um circuito de `n` fases.
fn opcoes_de_fases(n: u8) -> &'static [&'static str] {
    match n {
        1 => &["R", "S", "T"],
        2 => &["RS", "ST", "RT"],
        _ => &["RST"],
    }
}

/// Numa célula de tabela: um conteúdo com largura exata.
fn na_celula(ui: &mut egui::Ui, largura: f32, conteudo: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(vec2(largura, LINHA), Layout::left_to_right(Align::Center), |ui| {
        ui.set_width(largura);
        ui.spacing_mut().item_spacing.x = 2.0;
        conteudo(ui);
    });
}

/// A escolha do tipo de cabo de um circuito.
fn escolher_cabo(ui: &mut egui::Ui, q: Handle, id: &str, l: &ele::quadro::Linha, largura: f32, completo: bool, acoes: &mut Vec<Acao>) {
    let nome = if completo { l.tipo_cabo.nome() } else { l.tipo_cabo.abreviado() };
    let texto = if l.isolacao_propria { RichText::new(nome).strong() } else { RichText::new(nome) };
    egui::ComboBox::from_id_salt(("ele-cabo", id, completo)).selected_text(texto).width(largura).show_ui(ui, |ui| {
        if ui.selectable_label(!l.isolacao_propria, "Padrão do projeto").clicked() && l.isolacao_propria {
            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "cabo": null })));
        }
        for c in TipoCabo::TODOS {
            if ui.selectable_label(l.isolacao_propria && l.tipo_cabo == c, c.nome()).clicked() {
                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "cabo": c.codigo() })));
            }
        }
    });
}

/// A seção e o disjuntor escolhíveis, com o cadeado.
fn escolher_secao_e_disjuntor(ui: &mut egui::Ui, q: Handle, id: &String, l: &ele::quadro::Linha, largura: (f32, f32), acoes: &mut Vec<Acao>) {
    let e = &l.escolha;
    let mut sob = l.sobrescrita;
    let antes = sob;
    let secao_ruim = sob.secao.is_some_and(|v| l.automatico.secao.is_some_and(|a| v < a - 1e-9));
    let disj_ruim = sob.disjuntor.is_some_and(|d| d < e.ib - 1e-9 || e.iz.is_some_and(|iz| d > iz + 1e-9));
    na_celula(ui, largura.0, |ui| {
        let dica = if secao_ruim {
            "Abaixo do mínimo da NBR 5410 para este circuito. Clique para voltar ao automático"
        } else {
            "Fechado: valor escolhido à mão, fora do recálculo. Clique para alternar"
        };
        if cadeado(ui, sob.secao.is_some(), secao_ruim, dica).clicked() {
            sob.secao = if sob.secao.is_some() { None } else { e.secao };
        }
        escolher(ui, ("ele-secao", id), &mut sob.secao, l.automatico.secao, &SECOES, largura.0 - 20.0, secao_ruim);
    });
    na_celula(ui, largura.1, |ui| {
        let dica = if disj_ruim {
            "Fora de Ib ≤ In ≤ Iz (NBR 5410, 5.3.4). Clique para voltar ao automático"
        } else {
            "Fechado: valor escolhido à mão, fora do recálculo. Clique para alternar"
        };
        if cadeado(ui, sob.disjuntor.is_some(), disj_ruim, dica).clicked() {
            sob.disjuntor = if sob.disjuntor.is_some() { None } else { e.disjuntor };
        }
        escolher(ui, ("ele-disj", id), &mut sob.disjuntor, l.automatico.disjuntor, &DISJUNTORES, largura.1 - 20.0, disj_ruim);
    });
    if sob != antes {
        acoes
            .push(Acao::Rodar("eletrica.sobrescrever", json!({ "quadro": q.hex(), "circuito": id, "secao": sob.secao, "disjuntor": sob.disjuntor })));
    }
}

/// O resumo do quadro de cargas: circuitos, fases, grupos DR e o último cálculo. A engenharia de cada
/// circuito fica na ficha.
#[allow(clippy::too_many_arguments)]
fn quadro_de_cargas(
    ui: &mut egui::Ui,
    t: &Tokens,
    analise: &ele::Analise,
    s: &Setup,
    r: &ele::ResumoQuadro,
    c: &mut Cargas,
    destaque: &mut Option<(Handle, String)>,
    acoes: &mut Vec<Acao>,
) {
    let q = r.quadro;
    let alimentacao = match r.fases_quadro {
        1 => format!("{} V monofásico", fixo(r.tensao_fn, 0)),
        2 => format!("{}/{} V bifásico", fixo(r.tensao_ff, 0), fixo(r.tensao_fn, 0)),
        _ => format!("{}/{} V trifásico", fixo(r.tensao_ff, 0), fixo(r.tensao_fn, 0)),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("PAINEL: {}", r.tag)).strong().size(16.0));
        ui.add_space(12.0);
        ui.label(format!("Alimentação: {alimentacao}"));
    });
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let proximo = ele::circuito::proximo_circuito(&analise.pontos, q);
        ui.add(egui::TextEdit::singleline(&mut c.nova_descricao).hint_text(format!("Descrição do circuito {proximo}")).desired_width(200.0));
        if ui.button("Novo circuito").on_hover_text("Cria o circuito vazio; depois use \"+ Pontos\" na linha dele").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.criar", json!({ "quadro": q.hex(), "descricao": c.nova_descricao.trim() })));
            c.nova_descricao.clear();
        }
        ui.separator();
        if ui
            .button("Equilibrar fases")
            .on_hover_text("Destrava as fases de todos os circuitos: o balanceamento automático vale para todos")
            .clicked()
        {
            acoes.push(Acao::RodarComStatus("eletrica.quadro.equilibrar", json!({ "quadro": q.hex() }), |v| {
                format!("Fases equilibradas: desequilíbrio de {} %", fixo(v["desequilibrio"].as_f64().unwrap_or(0.0), 1))
            }));
        }
        if ui.button("Dimensionar projeto...").on_hover_text("Premissas do cálculo e Processar cálculo (ELEDIMENSIONAR)").clicked() {
            acoes.push(Acao::Rodar("ui.eletrica.dimensionar", Value::Null));
        }
        ui.separator();
        if ui.button("Tags de fiação").on_hover_text("Põe a simbologia de fiação nos eletrodutos deste quadro (ELEFIACAO)").clicked() {
            acoes.push(Acao::Rodar("elefiacao", json!({ "quadro": q.hex() })));
        }
        if ui.button("Tabela no desenho").on_hover_text("Clique no ponto da planta onde vai a tabela (ELEQUADRO_TABELA)").clicked() {
            acoes.push(Acao::Iniciar("elequadro_tabela"));
        }
        if ui.button("Copiar resumo").on_hover_text("Copia o resumo técnico em texto").clicked() {
            ui.ctx().copy_text(ele::quadro::texto(r, s));
        }
    });
    ui.add_space(4.0);

    let pendentes = r.linhas.iter().filter(|l| l.estado_calculo() != ele::quadro::EstadoCalculo::EmDia).count();
    if pendentes > 0 {
        ui.label(RichText::new(format!("{pendentes} circuito(s) sem cálculo ou com o projeto alterado: use Dimensionar projeto.")).color(COR_AVISO));
    }
    let fases = usize::from(r.fases_quadro.clamp(1, 3));
    let mut colunas: Vec<Coluna> = vec![
        ("Circ.", 34.0, false),
        ("", 262.0, false),
        ("Descrição", 150.0, false),
        ("Esquema", 56.0, false),
        ("Pontos", 44.0, true),
        ("Potência (VA)", 80.0, true),
        ("Potência (W)", 74.0, true),
        ("Ib (A)", 52.0, true),
    ];
    for nome in ["Fase R (VA)", "Fase S (VA)", "Fase T (VA)"].into_iter().take(fases) {
        colunas.push((nome, 66.0, true));
    }
    /// A primeira coluna de carga por fase.
    const F0: usize = 8;
    let fim = F0 + fases;
    colunas.extend_from_slice(&[("Fases", 78.0, false), ("Grupo DR", 76.0, false), ("Cabo / disjuntor", 106.0, false), ("Situação", 92.0, false)]);
    let largura = |i: usize| colunas.get(i).map_or(60.0, |c| c.1);
    let todos: Vec<(String, String)> = r
        .linhas
        .iter()
        .map(|l| (l.circuito.id.clone(), l.descricao.clone()))
        .chain(r.vazios.iter().map(|v| (v.id.clone(), v.descricao.clone())))
        .collect();

    egui::Grid::new("ele-cargas-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for l in &r.linhas {
            let k = &l.circuito;
            let id = &k.id;
            let destacado = destaque.as_ref().is_some_and(|(dq, dc)| *dq == q && dc.eq_ignore_ascii_case(id));
            // O número liga o destaque no desenho.
            let rotulo = if destacado { RichText::new(id.as_str()).strong().color(COR_DESTAQUE_PONTO) } else { RichText::new(id.as_str()) };
            let link = ui.allocate_ui_with_layout(vec2(largura(0), LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(rotulo)).inner;
            if link.on_hover_text(format!("{} · clique para destacar no desenho", k.natureza.nome())).clicked() {
                *destaque = if destacado { None } else { Some((q, id.clone())) };
            }
            let lugares: Vec<[f64; 2]> = analise.pontos.iter().filter(|p| k.pontos.contains(&p.handle)).map(|p| p.posicao).collect();
            acoes_do_circuito(ui, c, q, id, &k.pontos, &lugares, &todos, destaque, acoes);
            campo_descricao(ui, c, q, id, &l.descricao, largura(2), acoes);
            celula(ui, largura(3), l.esquema.codigo(), false);
            celula(ui, largura(4), numero(k.pontos.len().to_string()), true);
            celula(ui, largura(5), numero(fixo(k.potencia, 0)), true);
            celula(ui, largura(6), numero(fixo(l.potencia_w, 0)), true);
            celula(ui, largura(7), numero(fixo(l.escolha.ib, 2)), true);
            for f in 0..fases {
                let v = l.carga_fase.get(f).copied().unwrap_or(0.0);
                celula(ui, largura(F0 + f), numero(if v > 0.0 { fixo(v, 0) } else { String::new() }), true);
            }
            // Fases: balanceadas pelo motor, a não ser que travadas.
            na_celula(ui, largura(fim), |ui| {
                let dica = "Fechado: as fases ficam fora do balanceamento automático. Clique para alternar";
                let atuais = l.fases_texto().replace('+', "");
                if cadeado(ui, l.fases_travadas, false, dica).clicked() {
                    let f = if l.fases_travadas { Value::Null } else { json!(atuais) };
                    acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "fases": f })));
                }
                egui::ComboBox::from_id_salt(("ele-fases", id.as_str())).selected_text(atuais.as_str()).width(largura(fim) - 20.0).show_ui(
                    ui,
                    |ui| {
                        for o in opcoes_de_fases(l.esquema.fases()) {
                            if ui.selectable_label(atuais == *o, *o).clicked() {
                                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "fases": o })));
                            }
                        }
                    },
                );
            });
            // Grupo de DR.
            na_celula(ui, largura(fim + 1), |ui| {
                let atual = l.grupo_dr.as_deref().unwrap_or("-");
                egui::ComboBox::from_id_salt(("ele-dr", id.as_str())).selected_text(atual).width(largura(fim + 1) - 4.0).show_ui(ui, |ui| {
                    if ui.selectable_label(l.grupo_dr.is_none(), "-").clicked() {
                        acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "grupoDr": null })));
                    }
                    for g in ["IDR1", "IDR2", "IDR3", "IDR4", "IDR5", "IDR6"] {
                        if ui.selectable_label(atual == g, g).clicked() {
                            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "grupoDr": g })));
                        }
                    }
                });
            });
            // O que o último cálculo gravou.
            let resumo = l
                .calculado
                .map_or("-".to_string(), |d| format!("#{} · {} A", d.secao.map_or("?".to_string(), br), d.disjuntor.map_or("?".to_string(), br)));
            celula(ui, largura(fim + 2), numero(resumo), false);
            let mut avisos = l.escolha.avisos.clone();
            avisos.extend(k.avisos.iter().cloned());
            avisos.sort();
            avisos.dedup();
            match l.estado_calculo() {
                ele::quadro::EstadoCalculo::Pendente => {
                    celula(ui, largura(fim + 3), pequeno("A dimensionar", t.text_dim), false);
                }
                ele::quadro::EstadoCalculo::Desatualizado => {
                    celula(ui, largura(fim + 3), RichText::new("Desatualizado").color(COR_AVISO), false)
                        .on_hover_text("O projeto mudou depois do último cálculo");
                }
                ele::quadro::EstadoCalculo::EmDia if avisos.is_empty() => {
                    celula(ui, largura(fim + 3), "OK", false);
                }
                ele::quadro::EstadoCalculo::EmDia => {
                    celula(ui, largura(fim + 3), RichText::new(format!("{} aviso(s)", avisos.len())).color(COR_AVISO), false)
                        .on_hover_text(avisos.join("\n"));
                }
            }
            ui.end_row();
        }
        // Circuitos criados ainda sem pontos.
        for v in &r.vazios {
            celula(ui, largura(0), v.id.as_str(), false);
            acoes_do_circuito(ui, c, q, &v.id, &[], &[], &todos, destaque, acoes);
            campo_descricao(ui, c, q, &v.id, &v.descricao, largura(2), acoes);
            celula(ui, largura(3), pequeno("vazio", t.text_dim), false);
            for i in 4..=fim + 3 {
                celula(ui, largura(i), "", false);
            }
            ui.end_row();
        }
        // Linha de total.
        celula(ui, largura(0), "", false);
        celula(ui, largura(1), "", false);
        celula(ui, largura(2), RichText::new("TOTAL").strong(), false);
        celula(ui, largura(3), "", false);
        celula(ui, largura(4), "", false);
        celula(ui, largura(5), numero(fixo(r.potencia_total, 0)).strong(), true);
        celula(ui, largura(6), numero(fixo(r.linhas.iter().map(|l| l.potencia_w).sum::<f64>(), 0)).strong(), true);
        celula(ui, largura(7), "", false);
        for f in 0..fases {
            celula(ui, largura(F0 + f), numero(fixo(r.carga_por_fase.get(f).copied().unwrap_or(0.0), 0)).strong(), true);
        }
        for i in fim..=fim + 3 {
            celula(ui, largura(i), "", false);
        }
        ui.end_row();
    });
    if r.linhas.is_empty() && r.vazios.is_empty() {
        ui.label(pequeno("Nenhum circuito neste quadro: crie um acima ou use Criar circuito na paleta.", t.text_dim));
    }
    ui.add_space(4.0);
    egui::CollapsingHeader::new("Demanda, alimentador e totais").id_salt("ele-cargas-totais").show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width(440.0);
                ui.label(pequeno(format!("{} · {}", r.demanda.tabela, r.demanda.edificacao), t.text_dim));
                egui::Grid::new("ele-demanda").num_columns(4).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                    cabecalho(
                        ui,
                        t,
                        &[("Tipo de carga", 130.0, false), ("Instalada (VA)", 90.0, true), ("Fator", 60.0, true), ("Demandada (VA)", 100.0, true)],
                    );
                    for l in &r.demanda.linhas {
                        celula(ui, 130.0, l.tipo.nome(), false).on_hover_text(&l.regra);
                        celula(ui, 90.0, numero(fixo(l.instalada, 0)), true);
                        celula(ui, 60.0, numero(format!("{} %", fixo(l.fator * 100.0, 1))), true).on_hover_text(&l.regra);
                        celula(ui, 100.0, numero(fixo(l.demandada, 0)), true);
                        ui.end_row();
                    }
                });
                ui.add(egui::Label::new(pequeno(format!("Fonte: {}", r.demanda.fonte), t.text_dim)).wrap());
            });
            ui.add_space(20.0);
            ui.vertical(|ui| {
                egui::Grid::new("ele-totais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                    let linha = |ui: &mut egui::Ui, nome: &str, valor: String| {
                        ui.label(nome);
                        celula(ui, 100.0, numero(valor), true);
                        ui.end_row();
                    };
                    linha(ui, "Potência instalada", format!("{} VA", fixo(r.demanda.instalada, 0)));
                    linha(ui, "Potência demandada", format!("{} VA", fixo(r.demanda.demandada, 0)));
                    linha(ui, "Corrente demandada", format!("{} A", fixo(r.corrente_demandada, 2)));
                    if r.fases_quadro > 1 {
                        linha(ui, "Desequilíbrio", format!("{} %", fixo(r.desequilibrio, 1)));
                    }
                });
            });
            ui.add_space(20.0);
            ui.vertical(|ui| {
                let a = &r.alimentador;
                egui::Grid::new("ele-alimentador").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                    let linha = |ui: &mut egui::Ui, nome: &str, valor: String| {
                        ui.label(nome);
                        celula(ui, 100.0, numero(valor), true);
                        ui.end_row();
                    };
                    let ou = |v: Option<f64>, u: &str| v.map_or("acima da tabela".to_string(), |x| format!("{} {u}", br(x)));
                    linha(ui, "Cabo por fase", ou(a.secao, "mm²"));
                    linha(ui, "Disjuntor geral", ou(a.disjuntor, "A"));
                    linha(ui, "DR (30 mA)", ou(a.dr, "A"));
                    linha(ui, "DPS (Uc mínimo)", format!("{} V", fixo(a.dps_uc, 0)));
                });
                ui.label(pequeno("Sugestões a conferir: DPS classe II, In ≥ 5 kA (NBR 5410, 6.3.5).", t.text_dim));
            });
        });
    });
    lista_avisos(ui, "ele-avisos-resumo", &r.avisos);
}

/// A ficha de um circuito: a engenharia dele, com as escolhas à mão. É uma prévia até processar.
fn detalhes_do_circuito(ui: &mut egui::Ui, s: &Setup, q: Handle, l: &ele::quadro::Linha, acoes: &mut Vec<Acao>) {
    let id_texto = l.circuito.id.clone();
    let e = &l.escolha;
    let estimado = l.fora_da_rede > 0;
    let til = if estimado { "~" } else { "" };
    if l.estado_calculo() != ele::quadro::EstadoCalculo::EmDia {
        ui.label(RichText::new("Prévia: o quadro, as tags e as tabelas mudam ao processar o cálculo.").color(COR_AVISO));
    }
    let secoes = |ui: &mut egui::Ui, nome: &str, valor: String| {
        ui.label(nome);
        ui.label(numero(valor));
        ui.end_row();
    };
    if !l.descricao.is_empty() {
        ui.label(RichText::new(l.descricao.as_str()).strong());
    }
    egui::Grid::new("ele-detalhe-escolhas").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
        ui.label("Tipo de cabo");
        escolher_cabo(ui, q, &id_texto, l, 300.0, true, acoes);
        ui.end_row();
        ui.label("Esquema");
        egui::ComboBox::from_id_salt(("ele-esquema", id_texto.as_str())).selected_text(l.esquema.codigo()).width(120.0).show_ui(ui, |ui| {
            if ui.selectable_label(false, "Automático").clicked() {
                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id_texto, "esquema": null })));
            }
            for es in Esquema::TODOS {
                if ui.selectable_label(l.esquema == es, es.codigo()).clicked() {
                    acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id_texto, "esquema": es.codigo() })));
                }
            }
        });
        ui.end_row();
        ui.label("Tensão");
        egui::ComboBox::from_id_salt(("ele-tensao", id_texto.as_str())).selected_text(format!("{} V", fixo(l.tensao, 0))).width(120.0).show_ui(
            ui,
            |ui| {
                if ui.selectable_label(false, "Dos pontos").clicked() {
                    acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id_texto, "tensao": null })));
                }
                for v in [127.0, 220.0, 380.0] {
                    if ui.selectable_label((l.tensao - v).abs() < 1e-6, format!("{} V", fixo(v, 0))).clicked() {
                        acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id_texto, "tensao": v })));
                    }
                }
            },
        );
        ui.end_row();
        ui.label("Seção e disjuntor");
        ui.horizontal(|ui| escolher_secao_e_disjuntor(ui, q, &id_texto, l, (110.0, 110.0), acoes));
        ui.end_row();
    });
    ui.separator();
    egui::Grid::new("ele-detalhe-calculo").num_columns(2).spacing(vec2(12.0, 3.0)).show(ui, |ui| {
        secoes(ui, "Método de instalação", "B1, condutores em eletroduto".to_string());
        secoes(ui, "Temperatura e FCT", format!("{} °C · {}", fixo(s.temperatura_ambiente, 0), fixo(l.fct, 2)));
        secoes(ui, "Agrupamento e FCA", format!("{} circuito(s) · {}", l.agrupados, fixo(l.fca, 2)));
        secoes(ui, "Potência", format!("{} VA · {} W · FP {}", fixo(l.circuito.potencia, 0), fixo(l.potencia_w, 0), fixo(l.fp, 2)));
        secoes(ui, "Corrente de projeto (Ib)", format!("{} A", fixo(e.ib, 2)));
        secoes(ui, "Corrente corrigida (I'b)", format!("{} A", fixo(e.ib_corrigida, 2)));
        secoes(ui, "Capacidade (Iz)", e.iz.map_or("-".to_string(), |v| format!("{} A", fixo(v, 1))));
        let origem = if estimado { "estimada por Manhattan 3D" } else { "real, pelos eletrodutos" };
        secoes(ui, "Distância até a carga", l.comprimento.map_or("-".to_string(), |v| format!("{til}{} m, {origem}", fixo(v, 1))));
        secoes(ui, "Queda de tensão", e.queda.map_or("-".to_string(), |v| format!("{til}{} % (limite {} %)", fixo(v, 2), br(l.queda_admissivel))));
        secoes(ui, "Fases", l.fases_texto());
        secoes(ui, "Indicação", l.indicacao.clone());
    });
    let mut avisos = l.automatico.avisos.clone();
    avisos.extend(e.avisos.iter().cloned());
    avisos.extend(l.circuito.avisos.iter().cloned());
    avisos.sort();
    avisos.dedup();
    for a in &avisos {
        ui.label(pequeno(a.as_str(), COR_AVISO));
    }
}

/// A lista de materiais.
fn lista_de_materiais(ui: &mut egui::Ui, t: &Tokens, m: &ele::materiais::Materiais) {
    titulo(ui, t, "Cabos, com a folga do projeto");
    if m.cabos.is_empty() {
        ui.label(pequeno("Nenhum circuito dimensionado.", t.text_dim));
    }
    egui::Grid::new("ele-mat-cabos").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        for c in &m.cabos {
            celula(ui, 70.0, numero(format!("{} mm²", br(c.secao))), true);
            celula(ui, 300.0, c.tipo.nome(), false);
            celula(ui, 70.0, numero(format!("{} m", fixo(c.metros.ceil(), 0))), true);
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    titulo(ui, t, "Eletrodutos");
    if m.eletrodutos.is_empty() {
        ui.label(pequeno("Nenhum eletroduto ligado.", t.text_dim));
    }
    egui::Grid::new("ele-mat-dutos").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        for e in &m.eletrodutos {
            celula(ui, 300.0, e.tipo.nome(), false);
            celula(ui, 70.0, numero(e.diametro.map_or("sem Ø".to_string(), |d| format!("Ø{}", br(d)))), true)
                .on_hover_text("Sem Ø: nenhum circuito passa no trecho");
            celula(ui, 70.0, numero(format!("{} m", fixo(e.metros, 1))), true);
            ui.end_row();
        }
    });
    if m.eletrodutos_soltos > 0 {
        ui.label(pequeno(format!("{} eletroduto(s) solto(s) fora da metragem.", m.eletrodutos_soltos), COR_AVISO));
    }
    ui.add_space(6.0);
    let contados = |ui: &mut egui::Ui, id: &str, nome: &str, lista: &[ele::materiais::ItemContado]| {
        titulo(ui, t, nome);
        if lista.is_empty() {
            ui.label(pequeno("Nenhum.", t.text_dim));
        }
        egui::Grid::new(id).num_columns(2).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
            for i in lista {
                celula(ui, 200.0, i.nome.as_str(), false);
                celula(ui, 50.0, numero(i.quantidade.to_string()), true);
                ui.end_row();
            }
        });
        ui.add_space(6.0);
    };
    contados(ui, "ele-mat-caixas", "Caixas", &m.caixas);
    if m.pontos_sem_caixa > 0 {
        ui.label(pequeno(format!("{} ponto(s) de blocos sem família: a caixa deles não entra na conta.", m.pontos_sem_caixa), COR_AVISO));
    }
    contados(ui, "ele-mat-pontos", "Pontos", &m.pontos);
    contados(ui, "ele-mat-disj", "Disjuntores", &m.disjuntores);
    if !m.grupos_dr.is_empty() {
        contados(ui, "ele-mat-dr", "Circuitos por grupo de DR", &m.grupos_dr);
    }
    lista_avisos(ui, "ele-mat-avisos", &m.avisos);
}

/// Descrição editável: grava ao sair do campo.
fn campo_descricao(ui: &mut egui::Ui, c: &mut Cargas, q: Handle, id: &str, gravada: &str, largura: f32, acoes: &mut Vec<Acao>) {
    let i = match c.descricoes.iter().position(|(k, _)| k == id) {
        Some(i) => i,
        None => {
            c.descricoes.push((id.to_string(), gravada.to_string()));
            c.descricoes.len() - 1
        }
    };
    if let Some((_, texto)) = c.descricoes.get_mut(i) {
        let r = ui.add_sized(vec2(largura, LINHA), egui::TextEdit::singleline(texto).hint_text("sem descrição"));
        if r.lost_focus() && texto.trim() != gravada {
            acoes.push(Acao::Rodar("eletrica.circuito.descrever", json!({ "quadro": q.hex(), "circuito": id, "descricao": texto.trim() })));
        }
    }
}

/// Os botões de um circuito (adicionar e remover pontos, localizar) e o menu "..." (selecionar,
/// juntar, excluir), com a confirmação da exclusão na própria linha.
#[allow(clippy::too_many_arguments)]
fn acoes_do_circuito(
    ui: &mut egui::Ui,
    c: &mut Cargas,
    q: Handle,
    id: &str,
    pontos: &[Handle],
    lugares: &[[f64; 2]],
    todos: &[(String, String)],
    destaque: &mut Option<(Handle, String)>,
    acoes: &mut Vec<Acao>,
) {
    ui.horizontal(|ui| {
        if c.excluir.as_deref() == Some(id) {
            if ui.small_button(RichText::new("Excluir?").color(COR_AVISO)).on_hover_text("Os pontos ficam sem circuito (Ctrl+Z desfaz)").clicked() {
                acoes.push(Acao::Rodar("eletrica.circuito.excluir", json!({ "quadro": q.hex(), "circuito": id })));
                c.excluir = None;
            }
            if ui.small_button("Não").clicked() {
                c.excluir = None;
            }
            return;
        }
        if ui.small_button("Detalhes").on_hover_text("Abre a ficha de engenharia deste circuito").clicked() {
            acoes.push(Acao::Ficha(q, id.to_string()));
        }
        if ui.small_button("+ Pontos").on_hover_text("Adicionar pontos: clique neles no desenho; Enter termina").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.alvo", json!({ "quadro": q.hex(), "circuito": id })));
            acoes.push(Acao::Iniciar("elecircuito"));
        }
        if ui
            .add_enabled(!pontos.is_empty(), egui::Button::new("- Ponto").small())
            .on_hover_text("Remover ponto: clique nos que saem do circuito; Enter termina")
            .clicked()
        {
            acoes.push(Acao::Iniciar("eleremoverponto"));
        }
        if ui
            .add_enabled(!lugares.is_empty(), egui::Button::new("Localizar").small())
            .on_hover_text("Destaca o circuito e enquadra na tela")
            .clicked()
        {
            *destaque = Some((q, id.to_string()));
            acoes.push(Acao::Localizar(lugares.to_vec()));
        }
        ui.menu_button("...", |ui| {
            if ui.add_enabled(!pontos.is_empty(), egui::Button::new("Selecionar pontos")).clicked() {
                acoes.push(Acao::Selecionar(pontos.to_vec()));
                ui.close();
            }
            ui.menu_button("Juntar em", |ui| {
                for (outro, descricao) in todos.iter().filter(|(o, _)| o != id) {
                    if ui.button(format!("{outro}  {descricao}")).clicked() {
                        acoes.push(Acao::Rodar("eletrica.circuito.juntar", json!({ "quadro": q.hex(), "de": id, "para": outro })));
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button("Excluir circuito").clicked() {
                c.excluir = Some(id.to_string());
                ui.close();
            }
        });
    });
}

/// Uma caixa de escolha com "Auto" (a pré-seleção) e os valores da tabela.
fn escolher(ui: &mut egui::Ui, id: (&str, &String), valor: &mut Option<f64>, automatico: Option<f64>, opcoes: &[f64], largura: f32, violacao: bool) {
    let auto = automatico.map_or("-".to_string(), br);
    let texto = match valor {
        Some(v) if violacao => RichText::new(br(*v)).strong().color(COR_VIOLACAO),
        Some(v) => RichText::new(br(*v)).strong(),
        None => RichText::new(auto.clone()),
    };
    egui::ComboBox::from_id_salt(id).selected_text(texto).width(largura).show_ui(ui, |ui| {
        if ui.selectable_label(valor.is_none(), format!("Auto ({auto})")).clicked() {
            *valor = None;
        }
        for o in opcoes {
            if ui.selectable_label(*valor == Some(*o), br(*o)).clicked() {
                *valor = Some(*o);
            }
        }
    });
}

/// A aba Eletrodutos: cada trecho com a instalação, os circuitos e o diâmetro, que pode ser travado.
fn aba_lista_eletrodutos(ui: &mut egui::Ui, app: &CadApp, t: &Tokens, st: &mut Estado, s: &Setup, acoes: &mut Vec<Acao>) {
    let e = st.eletrodutos(app, s);
    let analise = st.analise.clone();
    if e.trechos.is_empty() && e.soltos.is_empty() {
        ui.label(pequeno("Nenhum eletroduto: use Inserir eletroduto na paleta.", t.text_dim));
        return;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} trecho(s) ligados, {} solto(s)", e.trechos.len(), e.soltos.len())).strong());
        if ui.button("Tags em todos").on_hover_text("Põe a tag de fiação em todos os eletrodutos (ELEFIACAO)").clicked() {
            acoes.push(Acao::Rodar("elefiacao", json!({})));
        }
    });
    let nome = |h: Handle| analise.ponto(h).map_or_else(|| h.hex(), PontoEletrico::rotulo);
    let colunas: [(&str, f32, bool); 9] = [
        ("De", 130.0, false),
        ("Para", 130.0, false),
        ("Tipo", 74.0, false),
        ("Cota", 92.0, false),
        ("Comprimento (m)", 104.0, true),
        ("Circuitos", 120.0, false),
        ("Diâmetro", 112.0, false),
        ("Ocupação", 70.0, true),
        ("", 70.0, false),
    ];
    egui::Grid::new("ele-eletrodutos-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for tr in &e.trechos {
            let h = tr.handle;
            celula(ui, colunas[0].1, nome(tr.de), false);
            celula(ui, colunas[1].1, nome(tr.para), false);
            ui.allocate_ui_with_layout(vec2(colunas[2].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
                egui::ComboBox::from_id_salt(("ele-trecho-tipo", h)).selected_text(tr.tipo.abreviado()).width(colunas[2].1 - 4.0).show_ui(ui, |ui| {
                    for ti in TipoEletroduto::TODOS {
                        if ui.selectable_label(tr.tipo == ti, ti.nome()).clicked() {
                            acoes.push(Acao::Rodar("eletrica.eletroduto.definir", json!({ "eletroduto": h.hex(), "tipo": ti.codigo() })));
                        }
                    }
                });
            });
            ui.allocate_ui_with_layout(vec2(colunas[3].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
                egui::ComboBox::from_id_salt(("ele-trecho-cota", h)).selected_text(tr.instalacao.nome()).width(colunas[3].1 - 4.0).show_ui(
                    ui,
                    |ui| {
                        for i in Instalacao::TODAS {
                            if ui.selectable_label(tr.instalacao == i, i.nome()).clicked() {
                                acoes.push(Acao::Rodar("eletrica.eletroduto.definir", json!({ "eletroduto": h.hex(), "instalacao": i.codigo() })));
                            }
                        }
                    },
                );
            });
            celula(ui, colunas[4].1, numero(fixo(tr.comprimento, 2)), true);
            let circuitos: Vec<&str> = tr.circuitos.iter().map(|c| c.circuito.as_str()).collect();
            celula(ui, colunas[5].1, circuitos.join(", "), false);
            let excedida = tr.ocupacao_excedida();
            ui.allocate_ui_with_layout(vec2(colunas[6].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let atual = tr.eletroduto.as_ref().map(|d| d.nominal);
                let dica = if excedida {
                    "Ocupação acima do limite da NBR 5410 (6.2.11.1.6). Clique para voltar ao automático"
                } else {
                    "Fechado: diâmetro escolhido à mão. Clique para alternar"
                };
                if cadeado(ui, tr.diametro_travado, excedida, dica).clicked() {
                    let d = if tr.diametro_travado { Value::Null } else { json!(atual) };
                    if !d.is_null() || tr.diametro_travado {
                        acoes.push(Acao::Rodar("eletrica.eletroduto.definir", json!({ "eletroduto": h.hex(), "diametro": d })));
                    }
                }
                let texto = atual.map_or("-".to_string(), |d| format!("Ø{}", br(d)));
                let texto = if excedida { RichText::new(texto).color(COR_VIOLACAO) } else { RichText::new(texto) };
                egui::ComboBox::from_id_salt(("ele-trecho-diam", h)).selected_text(texto).width(colunas[6].1 - 22.0).show_ui(ui, |ui| {
                    for d in &s.eletrodutos {
                        if ui.selectable_label(atual == Some(d.nominal), format!("Ø{}", br(d.nominal))).clicked() {
                            acoes.push(Acao::Rodar("eletrica.eletroduto.definir", json!({ "eletroduto": h.hex(), "diametro": d.nominal })));
                        }
                    }
                });
            });
            let ocupacao = tr.ocupacao.map_or("-".to_string(), |o| format!("{} %", fixo(o * 100.0, 0)));
            let ocupacao = if excedida { numero(ocupacao).color(COR_VIOLACAO) } else { numero(ocupacao) };
            celula(ui, colunas[7].1, ocupacao, true);
            if ui.small_button("Localizar").clicked() {
                acoes.push(Acao::Localizar(vec![tr.meio]));
            }
            ui.end_row();
        }
    });
    if !e.soltos.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Soltos: não ligam dois blocos, ficam fora do cálculo").color(COR_AVISO));
        for x in &e.soltos {
            ui.horizontal(|ui| {
                if ui.small_button("Localizar").clicked() {
                    acoes.push(Acao::Localizar(vec![x.meio]));
                }
                ui.label(pequeno(format!("{}: {}", x.eletroduto.hex(), x.motivo), t.text));
            });
        }
    }
    ui.label(pequeno(
        "Grafo estrito: um eletroduto liga exatamente dois blocos. Cruzamentos e derivações sem caixa não conectam; clique no meio de um trecho com Inserir eletroduto para pôr a caixa.",
        t.text_dim,
    ));
}

/// A aba Conferência: a lista da inspeção, os pontos e os circuitos.
fn aba_conferencia(ui: &mut egui::Ui, t: &Tokens, analise: &ele::Analise, itens: &[Item], acoes: &mut Vec<Acao>) {
    let problemas: Vec<&Item> = itens.iter().filter(|i| i.nivel != Nivel::Verde).collect();
    titulo(ui, t, "Inspeção");
    if problemas.is_empty() {
        ui.label(pequeno("Nenhum problema encontrado.", t.text_dim));
    }
    for i in problemas.iter().take(200) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 5.0, cor_do_nivel(i.nivel));
            if ui.small_button("Localizar").clicked() {
                acoes.push(Acao::Localizar(vec![i.pos]));
            }
            ui.label(i.motivo.as_str());
        });
    }
    if problemas.len() > 200 {
        ui.label(pequeno(format!("... e mais {}.", problemas.len() - 200), t.text_dim));
    }
    ui.separator();
    secao_pontos(ui, t, analise);
    ui.separator();
    secao_circuitos(ui, t, analise, acoes);
}

/// Os filtros da aba Pontos: o que aparece em cada um.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FiltroPontos {
    #[default]
    Todos,
    Tomadas,
    Iluminacao,
    Interruptores,
    Dados,
    Quadros,
    Caixas,
}

impl FiltroPontos {
    const TODOS: [FiltroPontos; 7] = [Self::Todos, Self::Tomadas, Self::Iluminacao, Self::Interruptores, Self::Dados, Self::Quadros, Self::Caixas];
    fn nome(self) -> &'static str {
        match self {
            Self::Todos => "Todos",
            Self::Tomadas => "Tomadas",
            Self::Iluminacao => "Iluminação",
            Self::Interruptores => "Interruptores",
            Self::Dados => "Dados/Telecom",
            Self::Quadros => "Quadros/Racks",
            Self::Caixas => "Caixas",
        }
    }
    fn aceita(self, tipo: TipoPonto) -> bool {
        match self {
            Self::Todos => true,
            Self::Tomadas => matches!(tipo, TipoPonto::Tomada | TipoPonto::TomadaEspecifica),
            Self::Iluminacao => tipo == TipoPonto::Iluminacao,
            Self::Interruptores => tipo == TipoPonto::Interruptor,
            Self::Dados => tipo == TipoPonto::Dados,
            Self::Quadros => matches!(tipo, TipoPonto::Quadro | TipoPonto::Rack),
            Self::Caixas => tipo == TipoPonto::Caixa,
        }
    }
}

/// A janela de pontos: os pontos do desenho por categoria, com altura, seleção e exclusão. Inserir é
/// com a mini-barra (ELEPONTOS).
fn lista_de_pontos(ui: &mut egui::Ui, t: &Tokens, analise: &ele::Analise, s: &Setup, p: &mut Pontos, acoes: &mut Vec<Acao>) {
    ui.horizontal_wrapped(|ui| {
        for f in FiltroPontos::TODOS {
            let n = analise.pontos.iter().filter(|q| f.aceita(q.tipo)).count();
            if ui.selectable_label(p.filtro == f, format!("{} ({n})", f.nome())).clicked() {
                p.filtro = f;
                p.excluir = None;
            }
        }
        ui.add_space(12.0);
        ui.add(egui::TextEdit::singleline(&mut p.busca).hint_text("Buscar bloco ou TAG").desired_width(170.0));
    });
    ui.separator();
    let busca = p.busca.trim().to_uppercase();
    let combina = |texto: &str| busca.is_empty() || texto.to_uppercase().contains(&busca);
    let filtro = p.filtro;
    let lista: Vec<&PontoEletrico> =
        analise.pontos.iter().filter(|q| filtro.aceita(q.tipo) && (combina(&q.bloco) || q.tag.as_deref().is_some_and(combina))).collect();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Pontos no desenho ({})", lista.len())).strong());
        if !lista.is_empty() {
            if ui.small_button("Selecionar todos").clicked() {
                acoes.push(Acao::Selecionar(lista.iter().map(|q| q.handle).collect()));
            }
            if p.excluir == Some(None) {
                if ui.small_button(RichText::new(format!("Excluir os {}?", lista.len())).color(COR_AVISO)).on_hover_text("Ctrl+Z desfaz").clicked() {
                    let hs: Vec<String> = lista.iter().map(|q| q.handle.hex()).collect();
                    acoes.push(Acao::Rodar("eletrica.ponto.excluir", json!({ "pontos": hs })));
                    p.excluir = None;
                }
                if ui.small_button("Não").clicked() {
                    p.excluir = None;
                }
            } else if ui.small_button("Excluir todos").clicked() {
                p.excluir = Some(None);
            }
        }
    });
    let colunas: [(&str, f32, bool); 7] = [
        ("Ponto", 150.0, false),
        ("Bloco", 130.0, false),
        ("Tipo", 90.0, false),
        ("Circuito", 56.0, false),
        ("Potência (VA)", 84.0, true),
        ("Altura", 64.0, false),
        ("", 90.0, false),
    ];
    egui::ScrollArea::vertical().id_salt("ele-pontos-lista").auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("ele-pontos-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
            cabecalho(ui, t, &colunas);
            for q in &lista {
                let link =
                    ui.allocate_ui_with_layout(vec2(colunas[0].1, LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(q.rotulo())).inner;
                if link.on_hover_text("Selecionar no desenho").clicked() {
                    acoes.push(Acao::Selecionar(vec![q.handle]));
                }
                celula(ui, colunas[1].1, q.bloco.as_str(), false);
                celula(ui, colunas[2].1, q.tipo.nome(), false);
                celula(ui, colunas[3].1, q.circuito.as_deref().unwrap_or("-"), false);
                celula(ui, colunas[4].1, numero(q.potencia.map_or("-".to_string(), |v| fixo(v, 0))), true);
                let mut altura = q.altura.unwrap_or_else(|| q.altura_legenda(s));
                let r = ui.add_sized(vec2(colunas[5].1, LINHA), campo(&mut altura, 0.0..=50.0, 0.01, 2, ""));
                if r.on_hover_text(format!("Legenda do projeto: {} m", fixo(q.altura_legenda(s), 2))).changed() {
                    acoes.push(Acao::Rodar("eletrica.ponto.altura", json!({ "pontos": [q.handle.hex()], "altura": altura })));
                }
                ui.horizontal(|ui| {
                    if p.excluir == Some(Some(q.handle)) {
                        if ui.small_button(RichText::new("Excluir?").color(COR_AVISO)).clicked() {
                            acoes.push(Acao::Rodar("eletrica.ponto.excluir", json!({ "pontos": [q.handle.hex()] })));
                            p.excluir = None;
                        }
                        if ui.small_button("Não").clicked() {
                            p.excluir = None;
                        }
                    } else if ui.small_button("Excluir").clicked() {
                        p.excluir = Some(Some(q.handle));
                    }
                });
                ui.end_row();
            }
        });
    });
}

/// O painel "Propriedades da Família", na paleta, enquanto o editor de blocos está aberto.
fn propriedades_da_familia(ui: &mut egui::Ui, t: &Tokens, nome: &str, familia: Option<&ele::familia::Familia>, acoes: &mut Vec<Acao>) {
    use ele::familia::{CaixaFisica, Familia, FamilyCategory, RefAltura, SystemDomain};
    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
    ui.label(RichText::new("Propriedades da Família").strong());
    ui.label(pequeno(format!("Bloco: {nome}"), t.text_dim));
    ui.add_space(2.0);
    let enviar = |acoes: &mut Vec<Acao>, campo: &str, valor: Value| {
        let mut p = serde_json::Map::new();
        p.insert("bloco".into(), json!(nome));
        p.insert(campo.into(), valor);
        acoes.push(Acao::Rodar("eletrica.familia.definir", Value::Object(p)));
    };
    let f = familia.cloned().unwrap_or_else(|| Familia::da_categoria(FamilyCategory::Nenhum));
    let largura = ui.available_width().min(240.0);
    let linha = |ui: &mut egui::Ui, nome: &str| {
        ui.label(pequeno(nome, t.text_dim));
    };
    linha(ui, "Categoria");
    egui::ComboBox::from_id_salt("ele-familia-categoria")
        .selected_text(if familia.is_some() { f.categoria.nome() } else { "Sem classificação" })
        .width(largura)
        .show_ui(ui, |ui| {
            for c in FamilyCategory::TODAS {
                if ui.selectable_label(familia.is_some() && f.categoria == c, c.nome()).clicked() {
                    enviar(acoes, "categoria", json!(c.codigo()));
                }
            }
        });
    if familia.is_none() {
        ui.label(pequeno("Escolha a categoria para o bloco virar uma família.", t.text_dim));
        return;
    }
    linha(ui, "Domínio");
    egui::ComboBox::from_id_salt("ele-familia-dominio").selected_text(f.dominio.nome()).width(largura).show_ui(ui, |ui| {
        for d in SystemDomain::TODOS {
            if ui.selectable_label(f.dominio == d, d.nome()).clicked() {
                enviar(acoes, "dominio", json!(d.codigo()));
            }
        }
    });
    linha(ui, "Referência de altura padrão");
    egui::ComboBox::from_id_salt("ele-familia-altura").selected_text(f.ref_altura.nome()).width(largura).show_ui(ui, |ui| {
        for r in RefAltura::TODAS {
            if ui.selectable_label(f.ref_altura == r, r.nome()).clicked() {
                enviar(acoes, "refAltura", json!(r.codigo()));
            }
        }
    });
    if f.ref_altura == RefAltura::Fixa {
        linha(ui, "Cota fixa");
        let mut cota = f.cota_fixa;
        if ui.add(campo(&mut cota, 0.0..=50.0, 0.01, 2, " m")).changed() {
            enviar(acoes, "cotaFixa", json!(cota));
        }
    }
    linha(ui, "Caixa física associada");
    egui::ComboBox::from_id_salt("ele-familia-caixa").selected_text(f.caixa.nome()).width(largura).show_ui(ui, |ui| {
        for c in CaixaFisica::TODAS {
            if ui.selectable_label(f.caixa == c, c.nome()).clicked() {
                enviar(acoes, "caixa", json!(c.codigo()));
            }
        }
    });
    linha(ui, "Potência padrão (0: a do atributo)");
    let mut potencia = f.potencia.unwrap_or(0.0);
    if ui.add(campo(&mut potencia, 0.0..=100_000.0, 10.0, 0, " VA")).changed() {
        enviar(acoes, "potencia", if potencia > 0.0 { json!(potencia) } else { Value::Null });
    }
    linha(ui, "Fator de potência (0: o do atributo)");
    let mut fp = f.fp.unwrap_or(0.0);
    if ui.add(campo(&mut fp, 0.0..=1.0, 0.01, 2, "")).changed() {
        enviar(acoes, "fp", if fp > 0.0 { json!(fp) } else { Value::Null });
    }
    linha(ui, "Nº de teclas/pontos");
    let mut teclas = f.teclas;
    if ui.add(egui::DragValue::new(&mut teclas).range(1..=3)).changed() {
        enviar(acoes, "teclas", json!(teclas));
    }
    linha(ui, "Classe de Demanda");
    let classe = f.classe_demanda.map_or("Automática (do tipo de ponto)", ele::demanda::TipoCarga::nome_classe);
    egui::ComboBox::from_id_salt("ele-familia-classe").selected_text(classe).width(largura).show_ui(ui, |ui| {
        if ui.selectable_label(f.classe_demanda.is_none(), "Automática (do tipo de ponto)").clicked() {
            enviar(acoes, "classeDemanda", Value::Null);
        }
        for c in ele::demanda::TipoCarga::CLASSES {
            if ui.selectable_label(f.classe_demanda == Some(c), c.nome_classe()).clicked() {
                enviar(acoes, "classeDemanda", json!(c.codigo()));
            }
        }
    });
    linha(ui, "Fator de Demanda (0,00 a 1,00)");
    let mut automatico = f.fator_demanda.is_none();
    if ui.checkbox(&mut automatico, "Automático, pela tabela da concessionária").changed() {
        enviar(acoes, "fatorDemanda", if automatico { Value::Null } else { json!(1.0) });
    }
    if let Some(mut fator) = f.fator_demanda
        && ui.add(campo(&mut fator, 0.0..=1.0, 0.01, 2, "")).changed()
    {
        enviar(acoes, "fatorDemanda", json!(fator));
    }
    ui.separator();
    ui.menu_button("Inserir geometria base", |ui| {
        for (forma, rotulo) in ele::biblioteca::GEOMETRIAS {
            if ui.button(rotulo).clicked() {
                acoes.push(Acao::Rodar("eletrica.familia.geometria", json!({ "forma": forma })));
                ui.close();
            }
        }
    });
    if ferramenta(ui, "Definir ponto de conexão", true, "Clique no ponto que vira a origem do bloco: inserção e chegada dos eletrodutos").clicked()
    {
        acoes.push(Acao::Iniciar("elefamilia_conector"));
    }
    if ferramenta(ui, "Gerar atributos padrão", true, "Cria os atributos da categoria que ainda faltam, como CIRCUITO, COMANDO e ALTURA").clicked() {
        acoes.push(Acao::Rodar("eletrica.familia.atributos", json!({ "bloco": nome })));
    }
    ui.label(pequeno("A família vale na hora; a geometria e os atributos entram no bloco ao salvar (BSAVE).", t.text_dim));
}

fn janela_setup(ctx: &egui::Context, t: &Tokens, tem_desenho: bool, acoes: &mut Vec<Acao>) {
    let Some(mut r) = trava(&JANELAS).setup.clone() else { return };
    let mut fechar = false;
    egui::Window::new("Free Elétrica · Configurações")
        .id(egui::Id::new("ele-janela-setup"))
        .title_bar(false)
        .frame(moldura_de_janela(ctx))
        .resizable(false)
        .default_pos(egui::pos2(420.0, 120.0))
        .default_width(620.0)
        .show(ctx, |ui| {
            fechar = cabecalho_de_janela(ui, "Configurações");
            ui.horizontal(|ui| {
                for aba in Aba::TODAS {
                    if ui.selectable_label(r.aba == aba, aba.nome()).clicked() {
                        r.aba = aba;
                    }
                }
            });
            ui.separator();
            match r.aba {
                Aba::Criterios => aba_criterios(ui, t, &mut r.setup),
                Aba::Alturas => aba_alturas(ui, t, &mut r.setup),
                Aba::Geral => aba_geral(ui, t, &mut r.setup, tem_desenho, acoes),
                Aba::Condutores => aba_condutores(ui, t, &mut r.setup),
                Aba::Eletrodutos => aba_eletrodutos(ui, t, &mut r.setup),
                Aba::Agrupamento => aba_agrupamento(ui, t),
                Aba::Demanda => aba_demanda(ui, t, &mut r),
                Aba::Telecom => aba_telecom(ui, t, &mut r),
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    definir_setup(r.setup.clone());
                    fechar = true;
                }
                if ui.button("Cancelar").clicked() {
                    fechar = true;
                }
            });
        });
    let mut j = trava(&JANELAS);
    if !fechar {
        j.setup = Some(r);
    } else {
        crate::eletrica::fechar(&mut j, Qual::Setup);
    }
}

/// Critérios elétricos do projeto.
fn aba_criterios(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-criterios").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Sistema de distribuição");
        egui::ComboBox::from_id_salt("ele-sistema").selected_text(s.sistema.nome()).width(230.0).show_ui(ui, |ui| {
            for sistema in Sistema::TODOS {
                ui.selectable_value(&mut s.sistema, sistema, sistema.nome());
            }
        });
        ui.end_row();
        ui.label("Tensões");
        let ff = s.sistema.tensao_ff().map_or_else(|| "-".to_string(), |v| format!("{} V", br(v)));
        ui.label(format!("F-N {} V · F-F {ff} · {} fase(s)", br(s.sistema.tensao_fn()), s.sistema.fases()));
        ui.end_row();
        ui.label("Temperatura ambiente");
        ui.add(campo(&mut s.temperatura_ambiente, 10.0..=60.0, 1.0, 0, " °C"));
        ui.end_row();
        ui.label("FCT (tabela 40)");
        ui.label(fixo(s.fct(), 2));
        ui.end_row();
        ui.label("Tipo de cabo padrão");
        egui::ComboBox::from_id_salt("ele-cabo-padrao").selected_text(s.cabo_padrao.nome()).width(280.0).show_ui(ui, |ui| {
            for c in TipoCabo::TODOS {
                ui.selectable_value(&mut s.cabo_padrao, c, c.nome());
            }
        });
        ui.end_row();
        ui.label("Queda de tensão: iluminação");
        ui.add(campo(&mut s.queda_iluminacao, 0.5..=10.0, 0.1, 1, " %"));
        ui.end_row();
        ui.label("Queda de tensão: força");
        ui.add(campo(&mut s.queda_admissivel, 0.5..=10.0, 0.1, 1, " %"));
        ui.end_row();
        ui.label("Queda de tensão: alimentador");
        ui.add(campo(&mut s.queda_alimentador, 0.1..=10.0, 0.1, 1, " %"));
        ui.end_row();
        ui.label("Folga de cabos");
        let mut folga = (s.fator_materiais - 1.0) * 100.0;
        if ui
            .add(campo(&mut folga, 0.0..=100.0, 1.0, 0, " %"))
            .on_hover_text("Soma aos comprimentos de cabo no levantamento (perdas, emendas, sobras)")
            .changed()
        {
            s.fator_materiais = 1.0 + folga / 100.0;
        }
        ui.end_row();
    });
    ui.label(pequeno(
        "O cabo padrão vale para os circuitos que não escolheram o seu, no Quadro de cargas. Tensões e fases valem quando o bloco não informa TENSAO ou o quadro não informa NUMERO_FASES. A queda do alimentador ainda não entra no cálculo: chega na etapa de dimensionamento.",
        t.text_dim,
    ));
}

/// As alturas padrão do projeto: a legenda que as famílias de bloco seguem.
fn aba_alturas(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-alturas").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Pé-direito (teto)");
        ui.add(campo(&mut s.pe_direito, 2.0..=10.0, 0.01, 2, " m"));
        ui.end_row();
        ui.label("Altura alta padrão");
        ui.add(campo(&mut s.altura_alta, 0.0..=10.0, 0.01, 2, " m"));
        ui.end_row();
        ui.label("Altura média padrão");
        ui.add(campo(&mut s.altura_media, 0.0..=10.0, 0.01, 2, " m"));
        ui.end_row();
        ui.label("Altura baixa padrão");
        ui.add(campo(&mut s.altura_baixa, 0.0..=10.0, 0.01, 2, " m"));
        ui.end_row();
        ui.label("Piso");
        ui.label("0,00 m");
        ui.end_row();
    });
    ui.label(pequeno(
        "Um ponto que segue a legenda não escreve a altura na planta, e muda junto com ela. Um ponto com outra altura mostra h=1,45m e não muda.",
        t.text_dim,
    ));
}

fn aba_geral(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup, tem_desenho: bool, acoes: &mut Vec<Acao>) {
    if ui
        .add_enabled(tem_desenho, egui::Button::new("Inicializar projeto no DXF atual"))
        .on_hover_text("Cria as camadas e os blocos que faltam; as ferramentas fazem isso sozinhas na primeira vez")
        .clicked()
    {
        acoes.push(Acao::Rodar("eletrica.bootstrap", json!({})));
    }
    ui.add_space(4.0);
    egui::Grid::new("ele-setup-geral").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Unidade do desenho");
        egui::ComboBox::from_id_salt("ele-unidade").selected_text(s.unidade.nome()).width(230.0).show_ui(ui, |ui| {
            for u in Unidade::TODAS {
                ui.selectable_value(&mut s.unidade, u, u.nome());
            }
        });
        ui.end_row();
        ui.label("Camadas dos eletrodutos");
        ui.add(egui::TextEdit::singleline(&mut s.camadas_eletroduto).desired_width(230.0))
            .on_hover_text("Separadas por vírgula; * e ? valem como curingas");
        ui.end_row();
        ui.label("Eletrodutos no piso");
        ui.add(egui::TextEdit::singleline(&mut s.camadas_piso).desired_width(230.0))
            .on_hover_text("Entre as camadas de eletroduto, as que correm no piso; as demais correm no teto");
        ui.end_row();
        ui.label("Raio de captura");
        ui.add(campo(&mut s.raio_captura, 0.01..=5.0, 0.01, 2, " m"))
            .on_hover_text("Distância máxima entre um bloco e a ponta do eletroduto que chega nele, e dos cliques nos blocos e eletrodutos");
        ui.end_row();
    });
    ui.label(pequeno("Os eletrodutos correm no teto, no piso ou na parede; cada ponta desce ou sobe até a altura do bloco.", t.text_dim));
}

fn aba_condutores(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-condutores").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Material / método");
        ui.label("Cobre · B1: condutores isolados em eletroduto");
        ui.end_row();
        for (f, nome) in ["Cor da fase R", "Cor da fase S", "Cor da fase T"].into_iter().enumerate() {
            ui.label(nome);
            if let Some(atual) = s.cores_fase.get_mut(f) {
                egui::ComboBox::from_id_salt(("ele-cor", f)).selected_text(atual.nome()).width(120.0).show_ui(ui, |ui| {
                    for c in CorCabo::TODAS {
                        ui.selectable_value(atual, c, c.nome());
                    }
                });
            }
            ui.end_row();
        }
    });
    ui.label("Omitir nas indicações e no resumo as seções:");
    ui.horizontal_wrapped(|ui| {
        for secao in SECOES.iter().copied().take(6) {
            let mut on = s.omitida(secao);
            if ui.checkbox(&mut on, format!("{} mm²", br(secao))).changed() {
                if on {
                    s.secoes_omitidas.push(secao);
                } else {
                    s.secoes_omitidas.retain(|x| (x - secao).abs() > 1e-6);
                }
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Diâmetro externo dos condutores").strong());
        if ui.small_button("Valores típicos").on_hover_text("Volta à tabela de fábrica").clicked() {
            s.diametros_condutor = diametros_padrao();
        }
    });
    egui::ScrollArea::vertical().id_salt("ele-diametros").max_height(170.0).show(ui, |ui| {
        egui::Grid::new("ele-setup-diametros").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
            cabecalho(ui, t, &[("Seção (mm²)", 80.0, true), ("PVC 450/750 V (mm)", 130.0, false), ("EPR/XLPE 0,6/1 kV (mm)", 150.0, false)]);
            for d in &mut s.diametros_condutor {
                let DiametroCondutor { secao, pvc, epr_xlpe } = d;
                celula(ui, 80.0, numero(br(*secao)), true);
                ui.add(campo(pvc, 0.0..=100.0, 0.05, 1, ""));
                ui.add(campo(epr_xlpe, 0.0..=100.0, 0.05, 1, ""));
                ui.end_row();
            }
        });
    });
    ui.label(pequeno(
        "Valores típicos de catálogo; confira com o fabricante do cabo usado. Neutro azul-claro e proteção verde-amarelo são fixos pela NBR 5410.",
        t.text_dim,
    ));
}

fn aba_eletrodutos(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-eletrodutos").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Tipo de eletroduto");
        ui.add(egui::TextEdit::singleline(&mut s.tipo_eletroduto).desired_width(260.0));
        ui.end_row();
        ui.label("Diâmetro mínimo");
        let minimo = s.eletroduto_minimo;
        egui::ComboBox::from_id_salt("ele-minimo").selected_text(format!("{} mm", br(minimo))).width(120.0).show_ui(ui, |ui| {
            for e in &s.eletrodutos {
                if ui.selectable_label((e.nominal - minimo).abs() < 1e-6, format!("{} mm ({})", br(e.nominal), e.polegadas)).clicked() {
                    s.eletroduto_minimo = e.nominal;
                }
            }
        });
        ui.end_row();
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Tamanhos").strong());
        if ui.small_button("Valores típicos").on_hover_text("Volta à tabela de fábrica").clicked() {
            s.eletrodutos = eletrodutos_padrao();
        }
    });
    egui::Grid::new("ele-setup-tamanhos").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &[("Nominal (mm)", 90.0, true), ("Polegadas", 80.0, false), ("Diâmetro interno (mm)", 140.0, false)]);
        for e in &mut s.eletrodutos {
            let TamanhoEletroduto { nominal, polegadas, interno } = e;
            celula(ui, 90.0, numero(br(*nominal)), true);
            celula(ui, 80.0, polegadas.as_str(), false);
            ui.add(campo(interno, 0.0..=500.0, 0.1, 1, ""));
            ui.end_row();
        }
    });
    ui.label(pequeno(
        "As tags de fiação escolhem, em cada trecho, o menor eletroduto a partir do mínimo que comporta os condutores: ocupação máxima de 53 % com um condutor, 31 % com dois e 40 % com três ou mais (NBR 5410, 6.2.11.1.6). Diâmetros internos típicos de catálogo; confira com o fabricante.",
        t.text_dim,
    ));
}

fn aba_agrupamento(ui: &mut egui::Ui, t: &Tokens) {
    egui::Grid::new("ele-setup-agrupamento").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("FCA (tabela 42)");
        ui.label("automático");
        ui.end_row();
    });
    ui.label(pequeno(
        "Ao rotear um quadro, cada circuito usa o fator do trecho de eletroduto mais cheio por onde passa (número de circuitos daquele quadro no trecho). Pontos fora da rede ficam com FCA 1,00.",
        t.text_dim,
    ));
}

/// O texto de uma faixa de quantidade: "1", "2 a 5", "6 ou mais".
fn faixa_de_quantidade(de: u32, ate: u32) -> String {
    match ate {
        u32::MAX => format!("{de} ou mais"),
        a if a == de => a.to_string(),
        a => format!("{de} a {a}"),
    }
}

fn aba_demanda(ui: &mut egui::Ui, t: &Tokens, r: &mut Rascunho) {
    if r.tabela_editada.is_some() {
        editor_de_tabela(ui, t, r);
        return;
    }
    let s = &mut r.setup;
    let tabela = s.tabela_demanda().clone();
    let todas: Vec<(String, String)> = s.tabelas_demanda().iter().map(|t| (t.id.clone(), t.nome.clone())).collect();
    egui::Grid::new("ele-setup-demanda").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Concessionária");
        egui::ComboBox::from_id_salt("ele-concessionaria").selected_text(tabela.nome.as_str()).width(320.0).show_ui(ui, |ui| {
            for (id, nome) in &todas {
                if ui.selectable_label(*id == tabela.id, nome.as_str()).clicked() {
                    s.concessionaria = id.clone();
                }
            }
        });
        ui.end_row();
        ui.label("Tipo de edificação");
        let atual = tabela.edificacao(&s.edificacao).nome.clone();
        egui::ComboBox::from_id_salt("ele-edificacao").selected_text(atual.as_str()).width(320.0).show_ui(ui, |ui| {
            for e in &tabela.edificacoes {
                if ui.selectable_label(e.nome == atual, e.nome.as_str()).clicked() {
                    s.edificacao = e.nome.clone();
                }
            }
        });
        ui.end_row();
    });
    let mut abrir_editor = None;
    let mut excluir = false;
    ui.horizontal(|ui| {
        if ui
            .button("+ Criar / Editar Tabela de Concessionária")
            .on_hover_text("Uma tabela própria abre para edição; com uma de fábrica escolhida, começa uma nova")
            .clicked()
        {
            abrir_editor = Some(if tabela.propria { tabela.clone() } else { ele::demanda::TabelaDemanda::em_branco("Nova concessionária") });
        }
        if tabela.propria && ui.button("Excluir tabela").clicked() {
            excluir = true;
        }
    });
    ui.add_space(4.0);
    let ed = tabela.edificacao(&s.edificacao).clone();
    ui.label(RichText::new("Iluminação e tomadas de uso geral").strong());
    if ed.modo == ele::demanda::ModoFaixas::TotalNaFaixa {
        ui.label(pequeno("O fator da faixa em que o total cai vale para toda a potência.", t.text_dim));
    }
    let mut inicio = 0.0;
    for f in &ed.faixas {
        let faixa = match f.ate_kw {
            Some(ate) if inicio == 0.0 => format!("até {} kVA", br(ate)),
            Some(ate) => format!("de {} a {} kVA", br(inicio), br(ate)),
            None if inicio == 0.0 => "toda a potência".to_string(),
            None => format!("acima de {} kVA", br(inicio)),
        };
        ui.label(format!("    {faixa}: {} %", br((f.fator * 1000.0).round() / 10.0)));
        inicio = f.ate_kw.unwrap_or(inicio);
    }
    let lista = |ui: &mut egui::Ui, nome: &str, tab: &[ele::demanda::PorQuantidade]| {
        if tab.is_empty() {
            ui.label(format!("{nome}: sem tabela, 100 %"));
            return;
        }
        let mut de = 1u32;
        let partes: Vec<String> = tab
            .iter()
            .map(|q| {
                let n = faixa_de_quantidade(de, q.ate);
                de = q.ate.saturating_add(1);
                format!("{n}: {}", fixo(q.fator, 2))
            })
            .collect();
        ui.add(egui::Label::new(format!("{nome}: {}", partes.join(" · "))).wrap());
    };
    egui::CollapsingHeader::new("Fatores por número de aparelhos").id_salt("ele-demanda-aparelhos").show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            ui.set_max_width(580.0);
            if !tabela.aquecimento.is_empty() {
                lista(ui, "Aquecimento", &tabela.aquecimento);
            }
            lista(ui, "Ar condicionado", &tabela.ar_condicionado);
            lista(ui, "Motores", &tabela.motores);
            lista(ui, "Bombas d'água", &tabela.bombas);
            if tabela.aparelhos.is_empty() {
                ui.label("TUE geral: sem tabela, 100 %");
            } else {
                ui.label(RichText::new("TUE geral").strong());
                egui::Grid::new("ele-demanda-tab4").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                    let lim = br(tabela.limite_aparelho_kw);
                    cabecalho(ui, t, &[("Aparelhos", 80.0, true), ("Até o limite", 90.0, true), ("Acima", 90.0, true)]);
                    let mut de = 1u32;
                    for q in &tabela.aparelhos {
                        celula(ui, 80.0, numero(faixa_de_quantidade(de, q.ate)), true);
                        celula(ui, 90.0, numero(fixo(q.ate_limite, 2)), true).on_hover_text(format!("potência individual até {lim} kW"));
                        celula(ui, 90.0, numero(fixo(q.acima_limite, 2)), true);
                        ui.end_row();
                        de = q.ate.saturating_add(1);
                    }
                });
            }
        });
    });
    ui.add_space(4.0);
    ui.set_max_width(600.0);
    ui.add(egui::Label::new(pequeno(format!("Fonte: {}", tabela.fonte), t.text_dim)).wrap());
    ui.add(
        egui::Label::new(pequeno(
            "A classe de demanda vem da família do bloco ou do Inspetor de atributos. Um ponto com fator próprio sai da contagem da tabela. Confira a revisão da norma em vigor na sua área de concessão. CPFL, Neoenergia, Copel, Celesc e Enel SP/RJ/CE não têm preset porque as tabelas não puderam ser conferidas nas normas públicas: cadastre a sua em + Criar / Editar.",
            t.text_dim,
        ))
        .wrap(),
    );
    if excluir {
        r.setup.remover_tabela(&tabela.id);
    }
    if let Some(e) = abrir_editor {
        r.tabela_editada = Some(e);
        r.erro_tabela = None;
    }
}

/// As linhas de uma lista por número de aparelhos, com a última valendo para mais.
fn editor_de_quantidades(ui: &mut egui::Ui, t: &Tokens, id: &str, nome: &str, lista: &mut Vec<ele::demanda::PorQuantidade>) {
    egui::CollapsingHeader::new(format!("{nome} ({} linha(s))", lista.len())).id_salt((id, "cabeca")).show(ui, |ui| {
        let mut sair = None;
        egui::Grid::new((id, "grade")).num_columns(3).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
            cabecalho(ui, t, &[("Até (aparelhos)", 100.0, false), ("Fator", 70.0, false), ("", 60.0, false)]);
            for (i, q) in lista.iter_mut().enumerate() {
                if q.ate == u32::MAX {
                    celula(ui, 100.0, "ou mais", false);
                } else {
                    ui.add(egui::DragValue::new(&mut q.ate).range(1..=100_000));
                }
                ui.add(campo(&mut q.fator, 0.0..=1.0, 0.01, 2, ""));
                if ui.small_button("Excluir").clicked() {
                    sair = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = sair {
            lista.remove(i);
        }
        ui.horizontal(|ui| {
            let aberta = lista.last().is_none_or(|q| q.ate != u32::MAX);
            if ui.add_enabled(aberta, egui::Button::new("+ Linha").small()).clicked() {
                let ate = lista.last().map_or(1, |q| q.ate.saturating_add(1));
                let fator = lista.last().map_or(1.0, |q| q.fator);
                lista.push(ele::demanda::PorQuantidade { ate, fator });
            }
            if ui.add_enabled(aberta, egui::Button::new("+ E acima").small()).clicked() {
                let fator = lista.last().map_or(1.0, |q| q.fator);
                lista.push(ele::demanda::PorQuantidade { ate: u32::MAX, fator });
            }
        });
    });
}

/// O cadastro de uma tabela de concessionária: fica no rascunho até o OK das configurações.
fn editor_de_tabela(ui: &mut egui::Ui, t: &Tokens, r: &mut Rascunho) {
    use ele::demanda::{Edificacao, Faixa, ModoFaixas, PorQuantidadeDupla, TabelaDemanda};
    let Some(e) = r.tabela_editada.as_mut() else { return };
    ui.label(RichText::new("Tabela de concessionária").strong());
    egui::ScrollArea::vertical().id_salt("ele-editor-tabela").max_height(460.0).show(ui, |ui| {
        ui.set_min_width(580.0);
        egui::Grid::new("ele-editor-tabela-dados").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
            let rotulo = ui.label("Nome da tabela");
            ui.add(egui::TextEdit::singleline(&mut e.nome).desired_width(360.0)).labelled_by(rotulo.id);
            ui.end_row();
            let rotulo = ui.label("Norma e revisão");
            ui.add(egui::TextEdit::singleline(&mut e.fonte).desired_width(360.0)).labelled_by(rotulo.id);
            ui.end_row();
            ui.label("Copiar de");
            egui::ComboBox::from_id_salt("ele-editor-copiar").selected_text("Escolha uma tabela").width(360.0).show_ui(ui, |ui| {
                for base in ele::demanda::tabelas() {
                    if ui.selectable_label(false, base.nome.as_str()).clicked() {
                        let (id, nome, fonte) = (e.id.clone(), e.nome.clone(), e.fonte.clone());
                        *e = TabelaDemanda { id, nome, fonte, propria: true, ..base.clone() };
                    }
                }
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Iluminação + TUG, por faixa de potência").strong());
        let mut sair_edificacao = None;
        for (i, ed) in e.edificacoes.iter_mut().enumerate() {
            egui::CollapsingHeader::new(if ed.nome.is_empty() { "Edificação sem nome" } else { ed.nome.as_str() })
                .id_salt(("ele-editor-ed", i))
                .default_open(i == 0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Nome");
                        ui.add(egui::TextEdit::singleline(&mut ed.nome).desired_width(220.0));
                        egui::ComboBox::from_id_salt(("ele-editor-modo", i))
                            .selected_text(if ed.modo == ModoFaixas::Acumulada {
                                "Fator por faixa acumulada"
                            } else {
                                "Fator sobre toda a potência"
                            })
                            .width(190.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut ed.modo, ModoFaixas::Acumulada, "Fator por faixa acumulada");
                                ui.selectable_value(&mut ed.modo, ModoFaixas::TotalNaFaixa, "Fator sobre toda a potência");
                            });
                        if ui.small_button("Excluir").clicked() {
                            sair_edificacao = Some(i);
                        }
                    });
                    let mut sair = None;
                    egui::Grid::new(("ele-editor-faixas", i)).num_columns(3).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                        cabecalho(ui, t, &[("Até (kVA)", 100.0, false), ("Fator", 70.0, false), ("", 60.0, false)]);
                        for (k, f) in ed.faixas.iter_mut().enumerate() {
                            match f.ate_kw.as_mut() {
                                Some(a) => {
                                    ui.add(campo(a, 0.0..=1_000_000.0, 0.5, 1, ""));
                                }
                                None => {
                                    celula(ui, 100.0, "acima", false);
                                }
                            }
                            ui.add(campo(&mut f.fator, 0.0..=1.0, 0.01, 2, ""));
                            if ui.small_button("Excluir").clicked() {
                                sair = Some(k);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(k) = sair {
                        ed.faixas.remove(k);
                    }
                    ui.horizontal(|ui| {
                        let aberta = ed.faixas.last().is_none_or(|f| f.ate_kw.is_some());
                        if ui.add_enabled(aberta, egui::Button::new("+ Faixa").small()).clicked() {
                            let ate = ed.faixas.last().and_then(|f| f.ate_kw).unwrap_or(0.0) + 10.0;
                            let fator = ed.faixas.last().map_or(1.0, |f| f.fator);
                            ed.faixas.push(Faixa { ate_kw: Some(ate), fator });
                        }
                        if ui.add_enabled(aberta, egui::Button::new("+ E acima").small()).clicked() {
                            let fator = ed.faixas.last().map_or(1.0, |f| f.fator);
                            ed.faixas.push(Faixa { ate_kw: None, fator });
                        }
                    });
                });
        }
        if let Some(i) = sair_edificacao {
            e.edificacoes.remove(i);
        }
        if ui.small_button("+ Edificação").clicked() {
            e.edificacoes.push(Edificacao {
                nome: format!("Edificação {}", e.edificacoes.len() + 1),
                modo: ModoFaixas::Acumulada,
                faixas: vec![Faixa { ate_kw: None, fator: 1.0 }],
            });
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Por número de aparelhos (lista vazia: 100 %)").strong());
        editor_de_quantidades(ui, t, "ele-editor-aquecimento", "Aquecimento / Eletrotérmico", &mut e.aquecimento);
        editor_de_quantidades(ui, t, "ele-editor-ar", "Ar-Condicionado / Climatização", &mut e.ar_condicionado);
        editor_de_quantidades(ui, t, "ele-editor-motores", "Motores", &mut e.motores);
        editor_de_quantidades(ui, t, "ele-editor-bombas", "Bombas d'água", &mut e.bombas);
        egui::CollapsingHeader::new(format!("TUE Geral ({} linha(s))", e.aparelhos.len())).id_salt("ele-editor-tue").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Limite da potência individual");
                ui.add(campo(&mut e.limite_aparelho_kw, 0.0..=1_000.0, 0.1, 1, " kW"));
            });
            let mut sair = None;
            egui::Grid::new("ele-editor-tue-grade").num_columns(4).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                cabecalho(ui, t, &[("Até (aparelhos)", 100.0, false), ("Até o limite", 80.0, false), ("Acima", 70.0, false), ("", 60.0, false)]);
                for (i, q) in e.aparelhos.iter_mut().enumerate() {
                    if q.ate == u32::MAX {
                        celula(ui, 100.0, "ou mais", false);
                    } else {
                        ui.add(egui::DragValue::new(&mut q.ate).range(1..=100_000));
                    }
                    ui.add(campo(&mut q.ate_limite, 0.0..=1.0, 0.01, 2, ""));
                    ui.add(campo(&mut q.acima_limite, 0.0..=1.0, 0.01, 2, ""));
                    if ui.small_button("Excluir").clicked() {
                        sair = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = sair {
                e.aparelhos.remove(i);
            }
            ui.horizontal(|ui| {
                let aberta = e.aparelhos.last().is_none_or(|q| q.ate != u32::MAX);
                let ultimo = e.aparelhos.last().copied().unwrap_or(PorQuantidadeDupla { ate: 0, ate_limite: 1.0, acima_limite: 1.0 });
                if ui.add_enabled(aberta, egui::Button::new("+ Linha").small()).clicked() {
                    e.aparelhos.push(PorQuantidadeDupla { ate: ultimo.ate.saturating_add(1), ..ultimo });
                }
                if ui.add_enabled(aberta, egui::Button::new("+ E acima").small()).clicked() {
                    e.aparelhos.push(PorQuantidadeDupla { ate: u32::MAX, ..ultimo });
                }
            });
        });
    });
    ui.separator();
    let (mut salvar, mut voltar) = (false, false);
    ui.horizontal(|ui| {
        salvar = ui.button("Salvar tabela").clicked();
        voltar = ui.button("Voltar sem salvar").clicked();
        if let Some(m) = &r.erro_tabela {
            ui.label(pequeno(m.as_str(), COR_AVISO));
        }
    });
    if voltar {
        r.tabela_editada = None;
        r.erro_tabela = None;
    } else if salvar {
        let tabela = r.tabela_editada.clone().unwrap_or_default();
        match r.setup.salvar_tabela(tabela) {
            Ok(id) => {
                r.setup.concessionaria = id;
                r.setup.normalizar();
                r.tabela_editada = None;
                r.erro_tabela = None;
            }
            Err(m) => r.erro_tabela = Some(m),
        }
    }
}

fn aba_telecom(ui: &mut egui::Ui, t: &Tokens, r: &mut Rascunho) {
    let mut remover: Option<String> = None;
    egui::ScrollArea::vertical().id_salt("ele-telecom-lista").max_height(220.0).show(ui, |ui| {
        egui::Grid::new("ele-telecom").num_columns(6).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
            cabecalho(
                ui,
                t,
                &[
                    ("Cabo", 130.0, false),
                    ("Tipo", 80.0, false),
                    ("Qtd.", 70.0, false),
                    ("Diâmetro (mm)", 90.0, false),
                    ("Observação", 100.0, false),
                    ("", 60.0, false),
                ],
            );
            for c in &mut r.setup.cabos_telecom {
                celula(ui, 130.0, c.nome.as_str(), false);
                celula(ui, 80.0, c.categoria.nome(), false);
                celula(ui, 70.0, format!("{} {}", c.quantidade, c.categoria.unidade()), false);
                ui.add(campo(&mut c.diametro, 0.0..=100.0, 0.05, 1, ""));
                celula(ui, 100.0, c.observacao.as_str(), false);
                if c.padrao {
                    celula(ui, 60.0, pequeno("padrão", t.text_dim), false);
                } else if ui.small_button("Remover").clicked() {
                    remover = Some(c.nome.clone());
                }
                ui.end_row();
            }
        });
    });
    if let Some(n) = remover {
        r.setup.remover_cabo(&n);
    }
    ui.separator();
    ui.label(RichText::new("Cadastrar novo cabo").strong());
    egui::Grid::new("ele-telecom-novo").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
        ui.label("Nome");
        ui.add(egui::TextEdit::singleline(&mut r.novo_cabo.nome).hint_text("Ex.: UTP Cat 6A").desired_width(200.0));
        ui.end_row();
        ui.label("Tipo");
        egui::ComboBox::from_id_salt("ele-telecom-tipo").selected_text(r.novo_cabo.categoria.nome()).show_ui(ui, |ui| {
            for c in CategoriaTelecom::TODAS {
                ui.selectable_value(&mut r.novo_cabo.categoria, c, c.nome());
            }
        });
        ui.end_row();
        ui.label(format!("Quantidade ({})", r.novo_cabo.categoria.unidade()));
        ui.add(egui::DragValue::new(&mut r.novo_cabo.quantidade).range(1..=10_000));
        ui.end_row();
        ui.label("Diâmetro externo");
        ui.add(campo(&mut r.novo_cabo.diametro, 0.0..=100.0, 0.05, 1, " mm"))
            .on_hover_text("Obrigatório: entra no cálculo de ocupação dos eletrodutos");
        ui.end_row();
        ui.label("Observação");
        ui.add(egui::TextEdit::singleline(&mut r.novo_cabo.observacao).desired_width(200.0));
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if ui.button("Cadastrar").clicked() {
            match r.setup.cadastrar_cabo(r.novo_cabo.clone()) {
                Ok(()) => {
                    r.erro = None;
                    r.novo_cabo.nome.clear();
                    r.novo_cabo.observacao.clear();
                    r.novo_cabo.diametro = 0.0;
                }
                Err(e) => r.erro = Some(e),
            }
        }
        if let Some(e) = &r.erro {
            ui.label(pequeno(e.as_str(), COR_AVISO));
        }
    });
    ui.label(pequeno(
        "Diâmetros típicos de catálogo; confira com o fabricante. O catálogo é gravado com as configurações ao clicar em OK.",
        t.text_dim,
    ));
}

fn lista_avisos(ui: &mut egui::Ui, id_salt: &str, avisos: &[String]) {
    if avisos.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(RichText::new(format!("{} aviso(s)", avisos.len())).color(COR_AVISO)).id_salt(id_salt).show(ui, |ui| {
        for a in avisos {
            ui.label(pequeno(a.as_str(), COR_AVISO));
        }
    });
}

fn titulo(ui: &mut egui::Ui, t: &Tokens, texto: &str) {
    ui.label(RichText::new(texto).strong().color(t.text));
}

fn secao_pontos(ui: &mut egui::Ui, t: &Tokens, a: &ele::Analise) {
    titulo(ui, t, "Pontos elétricos");
    if a.pontos.is_empty() {
        ui.label(pequeno("Nenhum bloco com atributos elétricos (TIPO_PONTO, POTENCIA, CIRCUITO...) no desenho.", t.text_dim));
        return;
    }
    let tipos = [
        TipoPonto::Quadro,
        TipoPonto::Iluminacao,
        TipoPonto::Tomada,
        TipoPonto::TomadaEspecifica,
        TipoPonto::Interruptor,
        TipoPonto::Caixa,
        TipoPonto::Outro,
    ];
    egui::Grid::new("ele-pontos").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        for tipo in tipos {
            let n = a.contar(tipo);
            if n > 0 {
                ui.label(tipo.nome());
                celula(ui, 40.0, numero(n.to_string()), true);
                ui.end_row();
            }
        }
    });
    let avisos: Vec<String> = a.avisos_pontos().into_iter().map(|(p, w)| format!("{p}: {w}")).collect();
    lista_avisos(ui, "ele-avisos-pontos", &avisos);
}

fn secao_circuitos(ui: &mut egui::Ui, t: &Tokens, a: &ele::Analise, acoes: &mut Vec<Acao>) {
    titulo(ui, t, "Circuitos");
    if a.circuitos.is_empty() {
        ui.label(pequeno("Nenhum circuito: use Criar circuito ou o Quadro de cargas.", t.text_dim));
        return;
    }
    let tag_quadro = |h: Option<Handle>| h.and_then(|h| a.ponto(h)).map_or("?".to_string(), |q| q.rotulo());
    let colunas: [(&str, f32, bool); 6] =
        [("Quadro", 70.0, false), ("Circ.", 40.0, false), ("Tipo", 80.0, false), ("Pontos", 50.0, true), ("VA", 60.0, true), ("V", 40.0, true)];
    egui::Grid::new("ele-circuitos").num_columns(colunas.len()).striped(true).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for c in &a.circuitos {
            celula(ui, 70.0, tag_quadro(c.quadro), false);
            let link = ui.allocate_ui_with_layout(vec2(40.0, LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(c.id.as_str())).inner;
            if link.on_hover_text("Selecionar os pontos deste circuito").clicked() {
                acoes.push(Acao::Selecionar(c.pontos.clone()));
            }
            celula(ui, 80.0, c.natureza.nome(), false);
            celula(ui, 50.0, numero(c.pontos.len().to_string()), true);
            celula(ui, 60.0, numero(fixo(c.potencia, 0)), true);
            celula(ui, 40.0, numero(c.tensao.map_or_else(|| "?".to_string(), |v| fixo(v, 0))), true);
            ui.end_row();
        }
    });
    let avisos: Vec<String> = a.circuitos.iter().flat_map(|c| c.avisos.iter().map(move |w| format!("Circuito {}: {w}", c.id))).collect();
    lista_avisos(ui, "ele-avisos-circuitos", &avisos);
}

#[cfg(test)]
mod tests {
    use super::{PALETA_ELETRICA, PALETA_TELECOM, chave_da_secao, fixo};

    #[test]
    fn a_paleta_tem_as_secoes_pedidas_sem_numeros_e_sem_rotulos_repetidos() {
        let nomes: Vec<&str> = PALETA_ELETRICA.iter().map(|s| s.nome).collect();
        assert_eq!(
            nomes,
            ["Pontos e Famílias", "Circuitos e Comandos", "Quadros e Dimensionamento", "Eletrodutos", "Documentação e Tabelas", "Projeto e Inspeção"]
        );
        for paleta in [&PALETA_ELETRICA[..], &PALETA_TELECOM[..]] {
            let mut rotulos: Vec<&str> = paleta.iter().flat_map(|s| s.ferramentas.iter().map(|f| f.rotulo)).collect();
            let total = rotulos.len();
            rotulos.sort_unstable();
            rotulos.dedup();
            assert_eq!(rotulos.len(), total);
        }
        assert_eq!(chave_da_secao("Eletrodutos"), "eletrica/Eletrodutos");
    }

    #[test]
    fn zero_nao_tem_sinal() {
        assert_eq!(fixo(-0.0, 0), "0");
        assert_eq!(fixo(-0.001, 2), "0,00");
        assert_eq!(fixo(-1.5, 1), "-1,5");
        assert_eq!(fixo(2.5, 2), "2,50");
    }
}
