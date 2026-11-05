// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Free Elétrica: a interface, em quatro zonas, nenhuma bloqueante.
//!
//! 1. A paleta Elétrica (`show`): as ferramentas na ordem do fluxo de trabalho.
//! 2. A barra de contexto (`sobre_canvas`): sobre o desenho, só enquanto ELEPONTO, ELEELETRODUTO ou
//!    ELECIRCUITO esperam cliques. Muda o que o comando usa sem interrompê-lo.
//! 3. O painel de baixo (`painel`): quadro de cargas, eletrodutos, pontos e conferência; pode ser
//!    recolhido, expandido e destacado.
//! 4. A sobreposição (`sobre_canvas`): os halos da inspeção visual e o destaque do circuito da
//!    linha clicada no quadro de cargas.
//!
//! Só Configurações é uma janela, e também não bloqueia. Toda alteração passa por um comando do
//! motor (`app.run`/`app.start`): a interface não decide nada de engenharia.
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
use free_eletrica::nbr5410::{DISJUNTORES, Esquema, Isolacao, SECOES};
use free_eletrica::ponto::br;
use free_eletrica::rota::Instalacao;
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
/// Altura do painel de baixo: aberto e recolhido.
const PAINEL_ABERTO: f32 = 340.0;
const PAINEL_RECOLHIDO: f32 = 96.0;
/// Altura da paleta, com todas as ferramentas à vista.
const ALTURA_PALETA: f32 = 500.0;

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
    aba: AbaPainel,
    cargas: Cargas,
    pontos: Pontos,
    /// Inspeção visual ligada.
    inspecao: bool,
    /// O circuito destacado no desenho: (quadro, circuito).
    destaque: Option<(Handle, String)>,
    recolhido: bool,
}

/// As abas do painel de baixo.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum AbaPainel {
    #[default]
    Cargas,
    Eletrodutos,
    Pontos,
    Conferencia,
}

impl AbaPainel {
    const TODAS: [AbaPainel; 4] = [AbaPainel::Cargas, AbaPainel::Eletrodutos, AbaPainel::Pontos, AbaPainel::Conferencia];
    fn nome(self) -> &'static str {
        match self {
            AbaPainel::Cargas => "Quadro de cargas",
            AbaPainel::Eletrodutos => "Eletrodutos",
            AbaPainel::Pontos => "Pontos",
            AbaPainel::Conferencia => "Conferência",
        }
    }
}

/// A aba Pontos: o filtro por tipo, o bloco escolhido e os valores dos atributos.
#[derive(Clone, Default)]
struct Pontos {
    /// `None`: todos os tipos.
    filtro: Option<TipoPonto>,
    busca: String,
    bloco: Option<String>,
    /// Valores para a inserção: (tag, valor), os padrões do bloco ao escolhê-lo.
    valores: Vec<(String, String)>,
    rotacao: f64,
    /// Ponto com a exclusão pedida, ou `Some(None)` para todos os filtrados (espera a confirmação).
    excluir: Option<Option<Handle>>,
}

#[derive(Clone)]
struct Rascunho {
    setup: Setup,
    aba: Aba,
    novo_cabo: CaboTelecom,
    erro: Option<String>,
}

#[derive(Clone, Default)]
struct Cargas {
    quadro: Option<Handle>,
    /// Descrições em edição: (circuito, texto).
    descricoes: Vec<(String, String)>,
    /// Circuito com a exclusão pedida (espera a confirmação).
    excluir: Option<String>,
    nova_descricao: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Aba {
    Geral,
    Condutores,
    Eletrodutos,
    Agrupamento,
    Demanda,
    Telecom,
}

impl Aba {
    const TODAS: [Aba; 6] = [Aba::Geral, Aba::Condutores, Aba::Eletrodutos, Aba::Agrupamento, Aba::Demanda, Aba::Telecom];
    fn nome(self) -> &'static str {
        match self {
            Aba::Geral => "Geral",
            Aba::Condutores => "Condutores",
            Aba::Eletrodutos => "Eletrodutos",
            Aba::Agrupamento => "Agrupamento",
            Aba::Demanda => "Demanda",
            Aba::Telecom => "Telecom",
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

/// Abre o painel de baixo na aba pedida.
fn abrir_painel(app: &mut CadApp, aba: AbaPainel) -> Result<Value, String> {
    let panel = Panel::QuadroCargas;
    let primeira = !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel);
    let r = crate::docking::command(app, false, &json!({ "operation": "open", "panel": panel.id() }))?;
    if primeira {
        redimensionar(app, panel, SplitSize::FixedSecond(PAINEL_ABERTO));
    }
    let mut j = trava(&JANELAS);
    j.aba = aba;
    if primeira {
        j.recolhido = false;
    }
    Ok(r)
}

fn abrir_setup() -> Result<Value, String> {
    let rascunho = Rascunho {
        setup: setup(),
        aba: Aba::Geral,
        novo_cabo: CaboTelecom { categoria: CategoriaTelecom::Utp, quantidade: 4, ..CaboTelecom::default() },
        erro: None,
    };
    trava(&JANELAS).setup = Some(rascunho);
    Ok(Value::Null)
}

/// ELEPONTOS: prepara o desenho, garante um modelo e começa a inserir, com a barra de contexto.
fn comecar_a_inserir(app: &mut CadApp) -> Result<Value, String> {
    app.run("eletrica.bootstrap", json!({}))?;
    let uid = app.session.state().map_err(|e| e.to_string())?.uid;
    if ele::biblioteca::modelo(uid).is_none() {
        app.run("eletrica.ponto.modelo", json!({ "bloco": ele::biblioteca::BLOCOS_BASE[0] }))?;
    }
    app.start("eleponto");
    Ok(Value::Null)
}

/// Comandos do Free Elétrica que só existem na interface: a paleta, o painel e a inspeção. `None`
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
            Some(abrir_painel(app, AbaPainel::Conferencia))
        }
        "ui.eletrica.inspecao" | "eleinspecao" => {
            let mut j = trava(&JANELAS);
            j.inspecao = !j.inspecao;
            Some(Ok(json!({ "inspecao": j.inspecao })))
        }
        "ui.eletrica.pontos" | "elepontos" => Some(comecar_a_inserir(app)),
        "ui.eletrica.lista" => Some(abrir_painel(app, AbaPainel::Pontos)),
        // Rotear fiação vive dentro do Quadro de cargas; ELEROTEAR abre o quadro de cargas.
        "ui.eletrica.cargas" | "elecargas" | "ui.eletrica.rotear" | "elerotear" => Some(abrir_painel(app, AbaPainel::Cargas)),
        "ui.eletrica.eletrodutos" => Some(abrir_painel(app, AbaPainel::Eletrodutos)),
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
    format!("{v:.casas$}").replace('.', ",")
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

/// A isolação gravada no quadro (sem ela, vale a das configurações).
fn isolacao_gravada(q: &PontoEletrico) -> Option<Isolacao> {
    q.extras.iter().find(|(k, _)| k.eq_ignore_ascii_case(ele::quadro::CHAVE_ISOLACAO)).and_then(|(_, v)| Isolacao::de_codigo(v))
}

/// Escolha do quadro e da isolação dele. Devolve se o quadro mudou.
fn seletor_quadro(ui: &mut egui::Ui, id: &str, analise: &ele::Analise, s: &Setup, quadro: &mut Option<Handle>, acoes: &mut Vec<Acao>) -> bool {
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
    if let Some(q) = quadro.and_then(|h| analise.ponto(h)) {
        ui.add_space(8.0);
        ui.label("Isolação");
        let gravada = isolacao_gravada(q);
        let texto = |i: Option<Isolacao>| match i {
            Some(i) => i.nome().to_string(),
            None => format!("Padrão do projeto ({})", s.isolacao.codigo()),
        };
        egui::ComboBox::from_id_salt((id, "isolacao")).selected_text(texto(gravada)).width(170.0).show_ui(ui, |ui| {
            for opcao in std::iter::once(None).chain(Isolacao::TODAS.into_iter().map(Some)) {
                if ui.selectable_label(gravada == opcao, texto(opcao)).clicked() && gravada != opcao {
                    acoes.push(Acao::Rodar("eletrica.quadro", json!({ "quadro": q.handle.hex(), "isolacao": opcao.map(|i| i.codigo()) })));
                }
            }
        });
    }
    mudou
}

// ---------------------------------------------------------------- zona 1: a paleta

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let tem_desenho = app.session.state().is_ok();
    let mut comando: Option<&str> = None;
    let inspecao = trava(&JANELAS).inspecao;
    egui::ScrollArea::vertical().id_salt("dock-eletrica").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            let mut botao = |ui: &mut egui::Ui, texto: &str, id: &'static str, dica: &str| {
                if ferramenta(ui, texto, tem_desenho, dica).clicked() {
                    comando = Some(id);
                }
            };
            grupo(ui, &t, "1. Pontos");
            botao(ui, "Inserir pontos", "ui.eletrica.pontos", "Clique onde vai cada ponto; a barra sobre o desenho escolhe tipo, altura e potência. Tab gira (ELEPONTOS)");
            grupo(ui, &t, "2. Circuitos e comandos");
            botao(ui, "Criar circuito", "elecircuito", "Selecione o quadro, o número e os pontos do circuito; a barra sobre o desenho edita descrição, esquema e tensão (ELECIRCUITO)");
            botao(ui, "Comandos e luminárias", "elecomando", "Selecione o interruptor e as luminárias que ele comanda (ELECOMANDO)");
            grupo(ui, &t, "3. Quadro de cargas");
            botao(ui, "Quadro de cargas", "ui.eletrica.cargas", "Planilha dos circuitos de um quadro, com balanceamento, alimentador e demanda (ELECARGAS)");
            grupo(ui, &t, "4. Eletrodutos");
            botao(ui, "Inserir eletroduto", "eleeletroduto", "Clique nos blocos em sequência; clicar no meio de um eletroduto insere uma caixa de passagem (ELEELETRODUTO)");
            grupo(ui, &t, "5. Documentação e tags");
            botao(ui, "Tags de fiação", "elefiacao", "Põe a simbologia de fiação e o diâmetro nos eletrodutos; a posição que você der fica guardada (ELEFIACAO)");
            botao(ui, "Tabela do quadro", "elequadro_tabela", "Desenha o quadro de cargas num ponto da planta (ELEQUADRO_TABELA)");
            botao(ui, "Atualizar tags e tabelas", "eleregen", "Refaz o que é gerado sem mudar as posições (ELEREGEN)");
            ui.add_space(6.0);
            let largura = ui.available_width();
            let inspecionar = egui::Button::new("Inspecionar projeto").selected(inspecao).min_size(vec2(largura, 26.0));
            let dica = "Halos no desenho: vermelho falta circuito ou comando, laranja sem caminho até o quadro, amarelo acima do limite, verde válido (ELEINSPECAO)";
            if ui.add_enabled(tem_desenho, inspecionar).on_hover_text(dica).clicked() {
                comando = Some("ui.eletrica.inspecao");
            }
            ui.add_space(6.0);
            if ferramenta(ui, "Configurações...", true, "Setup do projeto: sistema, temperatura, isolação, cabos, eletrodutos, telecom (ELECONFIG)").clicked() {
                comando = Some("ui.eletrica.setup");
            }
        });
    });
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

fn grupo(ui: &mut egui::Ui, t: &Tokens, nome: &str) {
    ui.add_space(4.0);
    ui.label(pequeno(nome, t.text_dim).strong());
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

/// Chamado a cada quadro, com ou sem a paleta visível: a abertura na primeira vez e Configurações.
pub fn janelas(app: &mut CadApp, ctx: &egui::Context) {
    if ABRIR_AO_INICIAR.swap(false, Ordering::Relaxed) {
        let panel = Panel::Eletrica;
        if !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel) {
            let _ = open(app);
        }
    }
    let setup_aberta = trava(&JANELAS).setup.is_some();
    if setup_aberta {
        janela_setup(ctx, &Tokens::get());
    }
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
const TIPOS_BARRA: [(&str, &str); 11] = [
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
                "eleponto" => barra_ponto(ui, uid, st, s, acoes),
                "eleeletroduto" => barra_eletroduto(ui, uid, acoes),
                _ => barra_circuito(ui, uid, st, acoes),
            });
        });
    });
}

fn barra_ponto(ui: &mut egui::Ui, uid: u64, st: &Estado, s: &Setup, acoes: &mut Vec<Acao>) {
    let Some(m) = ele::biblioteca::modelo(uid) else {
        ui.label("Digite o nome do bloco na linha de comando.");
        return;
    };
    let Some(bloco) = st.blocos.iter().find(|b| b.nome.eq_ignore_ascii_case(&m.bloco)) else { return };
    let tem = |tag: &str| bloco.atributos.iter().any(|a| !a.constante && a.tag.eq_ignore_ascii_case(tag));
    let padrao = |tag: &str| bloco.atributos.iter().find(|a| a.tag.eq_ignore_ascii_case(tag)).map_or(String::new(), |a| a.padrao.clone());
    let valor = |tag: &str| m.valores.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map_or_else(|| padrao(tag), |(_, v)| v.clone());
    let enviar = |acoes: &mut Vec<Acao>, nome: &str, rotacao_auto: bool, valores: &[(String, String)]| {
        let mapa: serde_json::Map<String, Value> = valores.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
        acoes
            .push(Acao::Rodar("eletrica.ponto.modelo", json!({ "bloco": nome, "rotacao": m.rotacao, "rotacaoAuto": rotacao_auto, "valores": mapa })));
    };
    let com = |tag: &str, v: String| -> Vec<(String, String)> {
        let mut l: Vec<(String, String)> = m.valores.iter().filter(|(t, _)| !t.eq_ignore_ascii_case(tag)).cloned().collect();
        l.push((tag.to_string(), v));
        l
    };

    ui.label("Tipo");
    let atual = TIPOS_BARRA.iter().find(|(_, b)| b.eq_ignore_ascii_case(&bloco.nome)).map_or(bloco.nome.as_str(), |(n, _)| n);
    egui::ComboBox::from_id_salt("ele-barra-tipo").selected_text(atual).width(130.0).show_ui(ui, |ui| {
        for (nome, b) in TIPOS_BARRA {
            if ui.selectable_label(bloco.nome.eq_ignore_ascii_case(b), nome).clicked() {
                enviar(acoes, b, m.rotacao_auto, &[]);
            }
        }
        // Os blocos próprios do projeto também entram.
        for b in st.blocos.iter().filter(|b| !TIPOS_BARRA.iter().any(|(_, n)| b.nome.eq_ignore_ascii_case(n))) {
            if ui.selectable_label(bloco.nome == b.nome, b.nome.as_str()).clicked() {
                enviar(acoes, &b.nome, m.rotacao_auto, &[]);
            }
        }
    });
    if tem("ALTURA") {
        ui.separator();
        ui.label("Altura");
        let v = valor("ALTURA");
        let numero_atual = ele::ponto::numero(&v);
        let lista = alturas(s);
        let nome_atual = lista
            .iter()
            .find(|(_, a)| numero_atual.is_some_and(|n| (n - a).abs() < 1e-6))
            .map_or(format!("{v} m"), |(n, a)| format!("{n} ({} m)", fixo(*a, 2)));
        egui::ComboBox::from_id_salt("ele-barra-altura").selected_text(nome_atual).width(120.0).show_ui(ui, |ui| {
            for (nome, a) in lista {
                if ui.selectable_label(numero_atual.is_some_and(|n| (n - a).abs() < 1e-6), format!("{nome} ({} m)", fixo(a, 2))).clicked() {
                    enviar(acoes, &bloco.nome, m.rotacao_auto, &com("ALTURA", fixo(a, 2)));
                }
            }
        });
    }
    if tem("POTENCIA") && bloco.tipo != TipoPonto::Interruptor {
        ui.separator();
        ui.label("Potência");
        let mut va = ele::ponto::numero(valor("POTENCIA").trim_end_matches(|c: char| c.is_alphabetic()).trim()).unwrap_or(0.0);
        if ui.add(campo(&mut va, 0.0..=100_000.0, 10.0, 0, " VA")).changed() {
            enviar(acoes, &bloco.nome, m.rotacao_auto, &com("POTENCIA", format!("{}VA", fixo(va, 0))));
        }
    }
    if tem("FP") {
        ui.separator();
        ui.label("FP");
        let mut fp = ele::ponto::numero(&valor("FP")).unwrap_or(1.0);
        if ui.add(campo(&mut fp, 0.1..=1.0, 0.01, 2, "")).changed() {
            enviar(acoes, &bloco.nome, m.rotacao_auto, &com("FP", fixo(fp, 2)));
        }
    }
    ui.separator();
    let mut auto = m.rotacao_auto;
    if ui.checkbox(&mut auto, "Alinhar à parede").changed() {
        enviar(acoes, &bloco.nome, auto, &m.valores);
    }
    if ui.button("Girar").on_hover_text("Soma 90° (Tab)").clicked() {
        acoes.push(Acao::Palavra("Girar"));
    }
}

fn barra_eletroduto(ui: &mut egui::Ui, uid: u64, acoes: &mut Vec<Acao>) {
    let m = ele::eletroduto::modelo(uid);
    ui.label("Cota");
    for i in Instalacao::TODAS {
        if ui.selectable_label(m.instalacao == i, i.nome()).clicked() {
            acoes.push(Acao::Rodar("eletrica.eletroduto.modelo", json!({ "instalacao": i.codigo() })));
        }
    }
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
    if ui.button("Inverter").on_hover_text("Curva para o outro lado").clicked() {
        acoes.push(Acao::Palavra("Inverter"));
    }
    ui.separator();
    ui.label(RichText::new("Clique nos blocos; no meio de um eletroduto entra uma caixa de passagem.").weak());
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

// ---------------------------------------------------------------- zona 3: o painel de baixo

pub fn painel(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let ctx = ui.ctx().clone();
    let mut st = ler_estado(app, &ctx);
    let s = setup();
    let tem_desenho = st.chave.is_some();
    let mut acoes: Vec<Acao> = Vec::new();
    let (mut aba, mut cargas, mut pontos, recolhido, mut destaque) = {
        let j = trava(&JANELAS);
        (j.aba, j.cargas.clone(), j.pontos.clone(), j.recolhido, j.destaque.clone())
    };
    let flutuando = app.ui.docking.layout.floating.iter().any(|g| g.panels.contains(&Panel::QuadroCargas));
    let mut alternar = false;
    let mut destacar = false;
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for a in AbaPainel::TODAS {
                if ui.selectable_label(aba == a, a.nome()).clicked() {
                    aba = a;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button(if flutuando { "Acoplar" } else { "Destacar" })
                    .on_hover_text("Solta o painel numa janela, ou o devolve para baixo do desenho")
                    .clicked()
                {
                    destacar = true;
                }
                if !flutuando && ui.small_button(if recolhido { "Expandir" } else { "Recolher" }).clicked() {
                    alternar = true;
                }
            });
        });
        ui.separator();
        if !tem_desenho {
            ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
            return;
        }
        egui::ScrollArea::both().id_salt("ele-painel").auto_shrink([false, false]).show(ui, |ui| match aba {
            AbaPainel::Cargas => aba_cargas(ui, app, &t, &mut st, &s, &mut cargas, &mut destaque, &mut acoes),
            AbaPainel::Eletrodutos => aba_lista_eletrodutos(ui, app, &t, &mut st, &s, &mut acoes),
            AbaPainel::Pontos => inserir_pontos(ui, &t, &st.analise, &st.blocos, &mut pontos, &mut acoes),
            AbaPainel::Conferencia => {
                let itens = st.inspecao(app, &s);
                aba_conferencia(ui, &t, &st.analise, &itens, &mut acoes);
            }
        });
    });
    {
        let mut j = trava(&JANELAS);
        j.aba = aba;
        j.cargas = cargas;
        j.pontos = pontos;
        j.destaque = destaque;
        if alternar {
            j.recolhido = !j.recolhido;
        }
    }
    guardar(&ctx, st);
    if alternar {
        let altura = if recolhido { PAINEL_ABERTO } else { PAINEL_RECOLHIDO };
        redimensionar(app, Panel::QuadroCargas, SplitSize::FixedSecond(altura));
    }
    if destacar {
        let p = Panel::QuadroCargas.id();
        let c = if flutuando {
            json!({ "operation": "move", "panel": p, "target": Panel::Canvas.id(), "zone": "bottom" })
        } else {
            let r = ui.max_rect();
            json!({ "operation": "float", "panel": p, "rect": [r.left() + 40.0, r.top() - 300.0, r.width().min(1200.0), 520.0] })
        };
        if let Err(e) = crate::docking::command(app, false, &c) {
            app.set_status(format!("Free Elétrica: {e}"));
        } else if flutuando {
            redimensionar(app, Panel::QuadroCargas, SplitSize::FixedSecond(PAINEL_ABERTO));
            trava(&JANELAS).recolhido = false;
        }
    }
    executar(app, acoes);
}

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
        ui.label(pequeno("Nenhum quadro no desenho: insira um Quadro QDG em Inserir pontos.", t.text_dim));
        return;
    }
    let analise = st.analise.clone();
    ui.horizontal(|ui| {
        if seletor_quadro(ui, "ele-cargas", &analise, s, &mut c.quadro, acoes) {
            c.descricoes.clear();
            c.excluir = None;
        }
    });
    let Some(q) = c.quadro else { return };
    let Some(resumo) = st.resumo(app, s, q) else { return };
    match resumo.as_ref() {
        Err(e) => {
            ui.label(pequeno(e.as_str(), COR_AVISO));
        }
        Ok(r) => quadro_de_cargas(ui, t, &analise, s, r, c, destaque, acoes),
    }
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

/// O quadro de cargas: a planilha dos circuitos de um quadro. Cada linha é um circuito, com o que
/// se edita (descrição, esquema, tensão, seção, disjuntor, isolação, fases) e o que se calcula
/// (correntes, queda, potência por fase); embaixo, os totais, o alimentador e a demanda.
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
    // Cabeçalho do painel.
    let alimentacao = match r.fases_quadro {
        1 => format!("{} V monofásico", fixo(r.tensao_fn, 0)),
        2 => format!("{}/{} V bifásico", fixo(r.tensao_ff, 0), fixo(r.tensao_fn, 0)),
        _ => format!("{}/{} V trifásico", fixo(r.tensao_ff, 0), fixo(r.tensao_fn, 0)),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("PAINEL: {}", r.tag)).strong().size(16.0));
        ui.add_space(12.0);
        ui.label(format!("Alimentação: {alimentacao}"));
        ui.add_space(12.0);
        ui.label(pequeno(format!("Demanda: {} · {}", r.demanda.tabela, r.demanda.edificacao), t.text_dim))
            .on_hover_text("Troque a concessionária e o tipo de edificação em Configurações > Demanda");
    });
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let proximo = ele::circuito::proximo_circuito(&analise.pontos, q);
        ui.add(egui::TextEdit::singleline(&mut c.nova_descricao).hint_text(format!("Descrição do circuito {proximo}")).desired_width(240.0));
        if ui.button("Novo circuito").on_hover_text("Cria o circuito vazio; depois use \"+ Pontos\" na linha dele").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.criar", json!({ "quadro": q.hex(), "descricao": c.nova_descricao.trim() })));
            c.nova_descricao.clear();
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

    let fases = usize::from(r.fases_quadro.clamp(1, 3));
    let mut colunas: Vec<Coluna> = vec![
        ("Circ.", 34.0, false),
        ("", 210.0, false),
        ("Descrição", 150.0, false),
        ("Esquema", 74.0, false),
        ("Tipo de carga", 110.0, false),
        ("Potência (VA)", 82.0, true),
        ("FP", 46.0, false),
        ("Potência (W)", 76.0, true),
        ("Tensão (V)", 66.0, false),
        ("Ib (A)", 56.0, true),
        ("I'b (A)", 56.0, true),
        ("Queda (%)", 60.0, true),
        ("Seção (mm²)", 96.0, false),
        ("Disjuntor (A)", 96.0, false),
        ("Isolação", 70.0, false),
        ("Fases", 78.0, false),
    ];
    for nome in ["Fase R (VA)", "Fase S (VA)", "Fase T (VA)"].into_iter().take(fases) {
        colunas.push((nome, 70.0, true));
    }
    colunas.push(("Situação", 64.0, false));
    /// A primeira coluna de carga por fase.
    const F0: usize = 16;
    let largura = |i: usize| colunas.get(i).map_or(60.0, |c| c.1);
    let todos: Vec<(String, String)> = r
        .linhas
        .iter()
        .map(|l| (l.circuito.id.clone(), l.descricao.clone()))
        .chain(r.vazios.iter().map(|v| (v.id.clone(), v.descricao.clone())))
        .collect();
    let celula_com = |ui: &mut egui::Ui, l: f32, conteudo: &mut dyn FnMut(&mut egui::Ui)| {
        ui.allocate_ui_with_layout(vec2(l, LINHA), Layout::left_to_right(Align::Center), |ui| {
            ui.set_width(l);
            conteudo(ui);
        });
    };

    egui::Grid::new("ele-cargas-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for l in &r.linhas {
            let k = &l.circuito;
            let e = &l.escolha;
            let id = &k.id;
            let destacado = destaque.as_ref().is_some_and(|(dq, dc)| *dq == q && dc.eq_ignore_ascii_case(id));
            // O número liga o destaque no desenho.
            let rotulo = if destacado { RichText::new(id.as_str()).strong().color(COR_DESTAQUE_PONTO) } else { RichText::new(id.as_str()) };
            let link = ui.allocate_ui_with_layout(vec2(largura(0), LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(rotulo)).inner;
            if link.on_hover_text(format!("{} ponto(s) · {} · clique para destacar no desenho", k.pontos.len(), k.natureza.nome())).clicked() {
                *destaque = if destacado { None } else { Some((q, id.clone())) };
            }
            let lugares: Vec<[f64; 2]> = analise.pontos.iter().filter(|p| k.pontos.contains(&p.handle)).map(|p| p.posicao).collect();
            acoes_do_circuito(ui, c, q, id, &k.pontos, &lugares, &todos, destaque, acoes);
            campo_descricao(ui, c, q, id, &l.descricao, largura(2), acoes);
            // Esquema.
            celula_com(ui, largura(3), &mut |ui| {
                egui::ComboBox::from_id_salt(("ele-esquema", id.as_str())).selected_text(l.esquema.codigo()).width(largura(3) - 4.0).show_ui(
                    ui,
                    |ui| {
                        if ui.selectable_label(false, "Automático").clicked() {
                            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "esquema": null })));
                        }
                        for es in Esquema::TODOS {
                            if ui.selectable_label(l.esquema == es, es.codigo()).clicked() {
                                acoes.push(Acao::Rodar(
                                    "eletrica.circuito.definir",
                                    json!({ "quadro": q.hex(), "circuito": id, "esquema": es.codigo() }),
                                ));
                            }
                        }
                    },
                );
            });
            // Tipo de carga e FP: o resultado dos atributos TIPO_CARGA e FP dos blocos.
            let tipos = if l.tipos_carga.is_empty() { "-".to_string() } else { l.tipos_texto() };
            celula(ui, largura(4), tipos.as_str(), false).on_hover_text(format!("{tipos}\n(atributo TIPO_CARGA de cada bloco)"));
            celula(ui, largura(5), numero(fixo(k.potencia, 0)), true);
            celula(ui, largura(6), numero(fixo(l.fp, 2)), true).on_hover_text("W / VA do circuito (atributo FP de cada bloco)");
            celula(ui, largura(7), numero(fixo(l.potencia_w, 0)), true);
            // Tensão.
            celula_com(ui, largura(8), &mut |ui| {
                egui::ComboBox::from_id_salt(("ele-tensao", id.as_str())).selected_text(fixo(l.tensao, 0)).width(largura(8) - 4.0).show_ui(
                    ui,
                    |ui| {
                        if ui.selectable_label(false, "Dos pontos").clicked() {
                            acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "tensao": null })));
                        }
                        for v in [127.0, 220.0, 380.0] {
                            if ui.selectable_label((l.tensao - v).abs() < 1e-6, fixo(v, 0)).clicked() {
                                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "tensao": v })));
                            }
                        }
                    },
                );
            });
            celula(ui, largura(9), numero(fixo(e.ib, 2)), true);
            celula(ui, largura(10), numero(fixo(e.ib_corrigida, 2)), true).on_hover_text(format!(
                "Ib / (FCT {} × FCA {})",
                fixo(l.fct, 2),
                fixo(l.fca, 2)
            ));
            // `~`: estimado sem eletroduto até o quadro (Manhattan 3D).
            let estimado = l.fora_da_rede > 0;
            let til = if estimado { "~" } else { "" };
            let queda = e.queda.map_or("-".to_string(), |v| format!("{til}{}", fixo(v, 2)));
            let comprimento = l.comprimento.map_or("-".to_string(), |v| format!("{til}{}", fixo(v, 1)));
            let nota = if estimado {
                format!(" (estimado por Manhattan 3D: {} ponto(s) sem eletroduto até o quadro)", l.fora_da_rede)
            } else {
                String::new()
            };
            let queda_alta = e.queda.is_some_and(|v| v > l.queda_admissivel);
            let queda_txt = if queda_alta { numero(queda).color(COR_VIOLACAO) } else { numero(queda) };
            celula(ui, largura(11), queda_txt, true)
                .on_hover_text(format!("Comprimento {comprimento} m{nota} · admissível {} %", br(l.queda_admissivel)));
            // Seção e disjuntor: a pré-seleção, que o projetista pode travar em outro valor.
            let mut sob = l.sobrescrita;
            let antes = sob;
            let secao_ruim = sob.secao.is_some_and(|v| l.automatico.secao.is_some_and(|a| v < a - 1e-9));
            let disj_ruim = sob.disjuntor.is_some_and(|d| d < e.ib - 1e-9 || e.iz.is_some_and(|iz| d > iz + 1e-9));
            celula_com(ui, largura(12), &mut |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let dica = if secao_ruim {
                    "Abaixo do mínimo da NBR 5410 para este circuito. Clique para voltar ao automático"
                } else {
                    "Fechado: valor escolhido à mão, fora do recálculo. Clique para alternar"
                };
                if cadeado(ui, sob.secao.is_some(), secao_ruim, dica).clicked() {
                    sob.secao = if sob.secao.is_some() { None } else { e.secao };
                }
                escolher(ui, ("ele-secao", id), &mut sob.secao, l.automatico.secao, &SECOES, largura(12) - 20.0, secao_ruim);
            });
            celula_com(ui, largura(13), &mut |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let dica = if disj_ruim {
                    "Fora de Ib ≤ In ≤ Iz (NBR 5410, 5.3.4). Clique para voltar ao automático"
                } else {
                    "Fechado: valor escolhido à mão, fora do recálculo. Clique para alternar"
                };
                if cadeado(ui, sob.disjuntor.is_some(), disj_ruim, dica).clicked() {
                    sob.disjuntor = if sob.disjuntor.is_some() { None } else { e.disjuntor };
                }
                escolher(ui, ("ele-disj", id), &mut sob.disjuntor, l.automatico.disjuntor, &DISJUNTORES, largura(13) - 20.0, disj_ruim);
            });
            if sob != antes {
                acoes.push(Acao::Rodar(
                    "eletrica.sobrescrever",
                    json!({ "quadro": q.hex(), "circuito": id, "secao": sob.secao, "disjuntor": sob.disjuntor }),
                ));
            }
            // Isolação (tipo de cabo) do circuito.
            celula_com(ui, largura(14), &mut |ui| {
                let texto = if l.isolacao_propria { RichText::new(l.isolacao.codigo()).strong() } else { RichText::new(l.isolacao.codigo()) };
                egui::ComboBox::from_id_salt(("ele-isol", id.as_str())).selected_text(texto).width(largura(14) - 4.0).show_ui(ui, |ui| {
                    if ui.selectable_label(!l.isolacao_propria, format!("Do quadro ({})", r.isolacao.codigo())).clicked() && l.isolacao_propria {
                        acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "isolacao": null })));
                    }
                    for i in Isolacao::TODAS {
                        if ui.selectable_label(l.isolacao_propria && l.isolacao == i, i.nome()).clicked() {
                            acoes
                                .push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "isolacao": i.codigo() })));
                        }
                    }
                });
            });
            // Fases: balanceadas pelo motor, a não ser que travadas.
            celula_com(ui, largura(15), &mut |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let dica = "Fechado: as fases ficam fora do balanceamento automático. Clique para alternar";
                if cadeado(ui, l.fases_travadas, false, dica).clicked() {
                    let f = if l.fases_travadas { Value::Null } else { json!(l.fases_texto().replace('+', "")) };
                    acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "fases": f })));
                }
                egui::ComboBox::from_id_salt(("ele-fases", id.as_str()))
                    .selected_text(l.fases_texto().replace('+', ""))
                    .width(largura(15) - 20.0)
                    .show_ui(ui, |ui| {
                        for o in opcoes_de_fases(l.esquema.fases()) {
                            if ui.selectable_label(l.fases_texto().replace('+', "") == *o, *o).clicked() {
                                acoes.push(Acao::Rodar("eletrica.circuito.definir", json!({ "quadro": q.hex(), "circuito": id, "fases": o })));
                            }
                        }
                    });
            });
            for f in 0..fases {
                let v = l.carga_fase.get(f).copied().unwrap_or(0.0);
                celula(ui, largura(F0 + f), numero(if v > 0.0 { fixo(v, 0) } else { String::new() }), true);
            }
            let mut avisos = l.automatico.avisos.clone();
            avisos.extend(e.avisos.iter().cloned());
            avisos.extend(k.avisos.iter().cloned());
            avisos.sort();
            avisos.dedup();
            let detalhes = format!(
                "Iz {} A · FCT {} · FCA {} ({} circuito(s) no trecho mais cheio, tabela 42) · fases {}",
                e.iz.map_or("-".to_string(), |v| fixo(v, 1)),
                fixo(l.fct, 2),
                fixo(l.fca, 2),
                l.agrupados,
                l.fases_texto()
            );
            let situacao = if avisos.is_empty() {
                celula(ui, largura(F0 + fases), "OK", false)
            } else {
                celula(ui, largura(F0 + fases), RichText::new(format!("{} aviso(s)", avisos.len())).color(COR_AVISO), false)
            };
            situacao.on_hover_text(if avisos.is_empty() { detalhes } else { format!("{}\n\n{detalhes}", avisos.join("\n")) });
            ui.end_row();
        }
        // Circuitos criados ainda sem pontos.
        for v in &r.vazios {
            celula(ui, largura(0), v.id.as_str(), false);
            acoes_do_circuito(ui, c, q, &v.id, &[], &[], &todos, destaque, acoes);
            campo_descricao(ui, c, q, &v.id, &v.descricao, largura(2), acoes);
            celula(ui, largura(3), pequeno("vazio", t.text_dim), false);
            for i in 4..F0 + fases + 1 {
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
        celula(ui, largura(6), "", false);
        celula(ui, largura(7), numero(fixo(r.linhas.iter().map(|l| l.potencia_w).sum::<f64>(), 0)).strong(), true);
        for i in 8..F0 {
            celula(ui, largura(i), "", false);
        }
        for f in 0..fases {
            celula(ui, largura(F0 + f), numero(fixo(r.carga_por_fase.get(f).copied().unwrap_or(0.0), 0)).strong(), true);
        }
        celula(ui, largura(F0 + fases), "", false);
        ui.end_row();
    });
    if r.linhas.is_empty() && r.vazios.is_empty() {
        ui.label(pequeno("Nenhum circuito neste quadro: crie um acima ou use Criar circuito na paleta.", t.text_dim));
    }
    ui.label(pequeno(
        "Tipo de carga e FP vêm dos atributos TIPO_CARGA e FP de cada bloco. Cadeado fechado: valor escolhido à mão, fora do recálculo; em vermelho, viola a NBR 5410. ~ indica comprimento estimado, sem eletroduto até o quadro. Ctrl+Z desfaz.",
        t.text_dim,
    ));
    ui.add_space(8.0);

    // Demanda, alimentador e totais.
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(460.0);
            ui.label(RichText::new("Demanda").strong());
            egui::Grid::new("ele-demanda").num_columns(4).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                cabecalho(
                    ui,
                    t,
                    &[("Tipo de carga", 140.0, false), ("Instalada (VA)", 96.0, true), ("Fator", 64.0, true), ("Demandada (VA)", 104.0, true)],
                );
                for l in &r.demanda.linhas {
                    celula(ui, 140.0, l.tipo.nome(), false).on_hover_text(&l.regra);
                    celula(ui, 96.0, numero(fixo(l.instalada, 0)), true);
                    celula(ui, 64.0, numero(format!("{} %", fixo(l.fator * 100.0, 1))), true).on_hover_text(&l.regra);
                    celula(ui, 104.0, numero(fixo(l.demandada, 0)), true);
                    ui.end_row();
                }
            });
            ui.add(egui::Label::new(pequeno(format!("Fonte: {}", r.demanda.fonte), t.text_dim)).wrap());
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("Totais do painel").strong());
            egui::Grid::new("ele-totais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                let linha = |ui: &mut egui::Ui, nome: &str, valor: String| {
                    ui.label(nome);
                    celula(ui, 100.0, numero(valor), true);
                    ui.end_row();
                };
                linha(ui, "Potência instalada", format!("{} VA", fixo(r.demanda.instalada, 0)));
                linha(ui, "Potência demandada", format!("{} VA", fixo(r.demanda.demandada, 0)));
                linha(ui, "Corrente instalada", format!("{} A", fixo(r.corrente_instalada, 2)));
                linha(ui, "Corrente demandada", format!("{} A", fixo(r.corrente_demandada, 2)));
                if r.fases_quadro > 1 {
                    linha(ui, "Desequilíbrio", format!("{} %", fixo(r.desequilibrio, 1)));
                }
            });
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("Alimentador").strong());
            let a = &r.alimentador;
            egui::Grid::new("ele-alimentador").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                let linha = |ui: &mut egui::Ui, nome: &str, valor: String| {
                    ui.label(nome);
                    celula(ui, 100.0, numero(valor), true);
                    ui.end_row();
                };
                let ou = |v: Option<f64>, u: &str| v.map_or("acima da tabela".to_string(), |x| format!("{} {u}", br(x)));
                linha(ui, "Corrente de projeto", format!("{} A", fixo(a.ib, 2)));
                linha(ui, "Cabo por fase", ou(a.secao, "mm²"));
                linha(ui, "Disjuntor geral", ou(a.disjuntor, "A"));
                linha(ui, "DR (30 mA)", ou(a.dr, "A"));
                linha(ui, "DPS (Uc mínimo)", format!("{} V", fixo(a.dps_uc, 0)));
            });
            ui.label(pequeno("Sugestões a conferir: DPS classe II, In ≥ 5 kA (NBR 5410, 6.3.5).", t.text_dim));
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("Cabos (fator {})", br(s.fator_materiais))).strong());
            egui::Grid::new("ele-materiais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                for (secao, m) in &r.materiais {
                    ui.label(format!("{} mm²", br(*secao)));
                    celula(ui, 70.0, numero(format!("{} m", fixo(m.ceil(), 0))), true);
                    ui.end_row();
                }
            });
        });
    });
    lista_avisos(ui, "ele-avisos-resumo", &r.avisos);
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
    let colunas: [(&str, f32, bool); 8] = [
        ("De", 130.0, false),
        ("Para", 130.0, false),
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
                egui::ComboBox::from_id_salt(("ele-trecho-cota", h)).selected_text(tr.instalacao.nome()).width(colunas[2].1 - 4.0).show_ui(
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
            celula(ui, colunas[3].1, numero(fixo(tr.comprimento, 2)), true);
            let circuitos: Vec<&str> = tr.circuitos.iter().map(|c| c.circuito.as_str()).collect();
            celula(ui, colunas[4].1, circuitos.join(", "), false);
            let excedida = tr.ocupacao_excedida();
            ui.allocate_ui_with_layout(vec2(colunas[5].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
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
                egui::ComboBox::from_id_salt(("ele-trecho-diam", h)).selected_text(texto).width(colunas[5].1 - 22.0).show_ui(ui, |ui| {
                    for d in &s.eletrodutos {
                        if ui.selectable_label(atual == Some(d.nominal), format!("Ø{}", br(d.nominal))).clicked() {
                            acoes.push(Acao::Rodar("eletrica.eletroduto.definir", json!({ "eletroduto": h.hex(), "diametro": d.nominal })));
                        }
                    }
                });
            });
            let ocupacao = tr.ocupacao.map_or("-".to_string(), |o| format!("{} %", fixo(o * 100.0, 0)));
            let ocupacao = if excedida { numero(ocupacao).color(COR_VIOLACAO) } else { numero(ocupacao) };
            celula(ui, colunas[6].1, ocupacao, true);
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

/// Os tipos do filtro da aba Pontos, na ordem dos botões.
const TIPOS_FILTRO: [TipoPonto; 7] = [
    TipoPonto::Iluminacao,
    TipoPonto::Tomada,
    TipoPonto::TomadaEspecifica,
    TipoPonto::Interruptor,
    TipoPonto::Quadro,
    TipoPonto::Caixa,
    TipoPonto::Outro,
];

/// A aba Pontos: filtro por tipo, os blocos de ponto do desenho, os valores para a inserção e os
/// pontos já inseridos (selecionar, excluir).
fn inserir_pontos(
    ui: &mut egui::Ui,
    t: &Tokens,
    analise: &ele::Analise,
    blocos: &[ele::biblioteca::BlocoPonto],
    p: &mut Pontos,
    acoes: &mut Vec<Acao>,
) {
    // Filtro por tipo e busca pelo nome.
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(p.filtro.is_none(), format!("Todos ({})", analise.pontos.len())).clicked() {
            p.filtro = None;
            p.excluir = None;
        }
        for tipo in TIPOS_FILTRO {
            let n = analise.contar(tipo);
            let tem_bloco = blocos.iter().any(|b| b.tipo == tipo);
            if (n > 0 || tem_bloco) && ui.selectable_label(p.filtro == Some(tipo), format!("{} ({n})", tipo.nome())).clicked() {
                p.filtro = Some(tipo);
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
    let do_tipo = |tipo: TipoPonto| filtro.is_none_or(|f| f == tipo);

    if blocos.is_empty() {
        ui.horizontal(|ui| {
            ui.label(pequeno("O desenho ainda não tem blocos de ponto elétrico.", t.text_dim));
            if ui
                .button("Preparar desenho")
                .on_hover_text("Cria as camadas e os blocos base; as ferramentas fazem isso sozinhas na primeira vez")
                .clicked()
            {
                acoes.push(Acao::Rodar("eletrica.bootstrap", json!({})));
            }
        });
    }
    if p.bloco.as_ref().is_some_and(|nome| !blocos.iter().any(|b| &b.nome == nome)) {
        p.bloco = None;
        p.valores.clear();
    }

    ui.horizontal_top(|ui| {
        // Altura fixa: o separador vertical entre as colunas não pode crescer o painel.
        ui.set_max_height(250.0);
        // Blocos disponíveis.
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.label(RichText::new("Blocos").strong());
            egui::ScrollArea::vertical().id_salt("ele-pontos-blocos").max_height(220.0).show(ui, |ui| {
                for b in blocos.iter().filter(|b| do_tipo(b.tipo) && combina(&b.nome)) {
                    let texto = format!("{}  ·  {}  ·  {} no desenho", b.nome, b.tipo.nome(), b.inseridos);
                    if ui.selectable_label(p.bloco.as_deref() == Some(b.nome.as_str()), texto).clicked() {
                        p.bloco = Some(b.nome.clone());
                        p.valores = b.atributos.iter().filter(|a| !a.constante).map(|a| (a.tag.clone(), a.padrao.clone())).collect();
                        p.excluir = None;
                    }
                }
            });
        });
        ui.separator();
        // Valores da inserção.
        ui.vertical(|ui| {
            let Some(nome) = p.bloco.clone() else {
                ui.label(pequeno("Escolha um bloco para inserir.", t.text_dim));
                return;
            };
            let prompts: Vec<(String, String)> = blocos
                .iter()
                .find(|b| b.nome == nome)
                .map(|b| b.atributos.iter().map(|a| (a.tag.clone(), a.prompt.clone())).collect())
                .unwrap_or_default();
            ui.label(RichText::new(format!("Inserir {nome}")).strong());
            egui::ScrollArea::vertical().id_salt("ele-pontos-valores-rolagem").max_height(180.0).show(ui, |ui| {
                egui::Grid::new("ele-pontos-valores").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                    for (tag, valor) in &mut p.valores {
                        let dica = prompts
                            .iter()
                            .find(|(t, _)| t == tag)
                            .map(|(_, pr)| pr.clone())
                            .filter(|pr| !pr.is_empty())
                            .unwrap_or_else(|| tag.clone());
                        ui.label(tag.as_str()).on_hover_text(dica);
                        ui.add(egui::TextEdit::singleline(valor).desired_width(170.0));
                        ui.end_row();
                    }
                    ui.label("Rotação");
                    ui.add(campo(&mut p.rotacao, -360.0..=360.0, 1.0, 0, "°"));
                    ui.end_row();
                });
            });
            ui.add_space(4.0);
            if ui.button("Inserir no desenho").on_hover_text("Clique no desenho onde vai cada ponto; Enter termina (ELEPONTO)").clicked() {
                let valores: serde_json::Map<String, Value> = p.valores.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
                acoes.push(Acao::Rodar(
                    "eletrica.ponto.modelo",
                    json!({ "bloco": nome, "rotacao": p.rotacao, "rotacaoAuto": p.rotacao == 0.0, "valores": valores }),
                ));
                acoes.push(Acao::Iniciar("eleponto"));
            }
        });
        ui.separator();
        // Pontos já inseridos, do tipo filtrado.
        ui.vertical(|ui| {
            let lista: Vec<&PontoEletrico> =
                analise.pontos.iter().filter(|q| do_tipo(q.tipo) && (combina(&q.bloco) || q.tag.as_deref().is_some_and(combina))).collect();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Pontos no desenho ({})", lista.len())).strong());
                if !lista.is_empty() {
                    if ui.small_button("Selecionar todos").clicked() {
                        acoes.push(Acao::Selecionar(lista.iter().map(|q| q.handle).collect()));
                    }
                    if p.excluir == Some(None) {
                        if ui
                            .small_button(RichText::new(format!("Excluir os {}?", lista.len())).color(COR_AVISO))
                            .on_hover_text("Ctrl+Z desfaz")
                            .clicked()
                        {
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
            let colunas: [(&str, f32, bool); 6] = [
                ("Ponto", 150.0, false),
                ("Bloco", 150.0, false),
                ("Tipo", 100.0, false),
                ("Circuito", 60.0, false),
                ("Potência (VA)", 90.0, true),
                ("", 90.0, false),
            ];
            egui::ScrollArea::vertical().id_salt("ele-pontos-lista").max_height(190.0).show(ui, |ui| {
                egui::Grid::new("ele-pontos-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                    cabecalho(ui, t, &colunas);
                    for q in &lista {
                        let link = ui
                            .allocate_ui_with_layout(vec2(colunas[0].1, LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(q.rotulo()))
                            .inner;
                        if link.on_hover_text("Selecionar no desenho").clicked() {
                            acoes.push(Acao::Selecionar(vec![q.handle]));
                        }
                        celula(ui, colunas[1].1, q.bloco.as_str(), false);
                        celula(ui, colunas[2].1, q.tipo.nome(), false);
                        celula(ui, colunas[3].1, q.circuito.as_deref().unwrap_or("-"), false);
                        celula(ui, colunas[4].1, numero(q.potencia.map_or("-".to_string(), |v| fixo(v, 0))), true);
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
        });
    });
}

fn janela_setup(ctx: &egui::Context, t: &Tokens) {
    let Some(mut r) = trava(&JANELAS).setup.clone() else { return };
    let mut aberta = true;
    let mut fechar = false;
    egui::Window::new("Free Elétrica · Configurações")
        .id(egui::Id::new("ele-janela-setup"))
        .open(&mut aberta)
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(420.0, 120.0))
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for aba in Aba::TODAS {
                    if ui.selectable_label(r.aba == aba, aba.nome()).clicked() {
                        r.aba = aba;
                    }
                }
            });
            ui.separator();
            match r.aba {
                Aba::Geral => aba_geral(ui, t, &mut r.setup),
                Aba::Condutores => aba_condutores(ui, t, &mut r.setup),
                Aba::Eletrodutos => aba_eletrodutos(ui, t, &mut r.setup),
                Aba::Agrupamento => aba_agrupamento(ui, t),
                Aba::Demanda => aba_demanda(ui, t, &mut r.setup),
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
    trava(&JANELAS).setup = (aberta && !fechar).then_some(r);
}

fn aba_geral(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-geral").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
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
        ui.label("Pé-direito padrão");
        ui.add(campo(&mut s.pe_direito, 2.0..=10.0, 0.01, 2, " m"));
        ui.end_row();
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
    ui.label(pequeno(
        "Tensões e fases valem quando o bloco não informa TENSAO ou o quadro não informa NUMERO_FASES. Os eletrodutos correm no teto (pé-direito), no piso ou na parede; cada ponta desce ou sobe até a ALTURA do bloco.",
        t.text_dim,
    ));
}

fn aba_condutores(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-condutores").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Isolação padrão");
        egui::ComboBox::from_id_salt("ele-isolacao").selected_text(s.isolacao.nome()).width(160.0).show_ui(ui, |ui| {
            for i in Isolacao::TODAS {
                ui.selectable_value(&mut s.isolacao, i, i.nome());
            }
        });
        ui.end_row();
        ui.label("Material / método");
        ui.label("Cobre · B1: condutores isolados em eletroduto");
        ui.end_row();
        ui.label("Temperatura do projeto");
        ui.add(campo(&mut s.temperatura_ambiente, 10.0..=60.0, 1.0, 0, " °C"));
        ui.end_row();
        ui.label("FCT (tabela 40)");
        ui.label(fixo(s.fct(), 2));
        ui.end_row();
        ui.label("Queda de tensão admissível");
        ui.add(campo(&mut s.queda_admissivel, 0.5..=10.0, 0.1, 1, " %"));
        ui.end_row();
        ui.label("Fator para materiais");
        ui.add(campo(&mut s.fator_materiais, 1.0..=2.0, 0.01, 2, ""))
            .on_hover_text("Multiplica os comprimentos de cabo no levantamento (perdas, emendas, sobras)");
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

fn aba_demanda(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    let tabela = s.tabela_demanda();
    egui::Grid::new("ele-setup-demanda").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Concessionária");
        egui::ComboBox::from_id_salt("ele-concessionaria").selected_text(tabela.nome).width(300.0).show_ui(ui, |ui| {
            for tb in ele::demanda::tabelas() {
                if ui.selectable_label(tb.id == tabela.id, tb.nome).clicked() {
                    s.concessionaria = tb.id.to_string();
                }
            }
        });
        ui.end_row();
        ui.label("Tipo de edificação");
        let atual = tabela.edificacao(&s.edificacao).nome;
        egui::ComboBox::from_id_salt("ele-edificacao").selected_text(atual).width(300.0).show_ui(ui, |ui| {
            for e in tabela.edificacoes {
                if ui.selectable_label(e.nome == atual, e.nome).clicked() {
                    s.edificacao = e.nome.to_string();
                }
            }
        });
        ui.end_row();
    });
    ui.add_space(4.0);
    let ed = tabela.edificacao(&s.edificacao);
    ui.label(RichText::new("Iluminação e tomadas de uso geral").strong());
    let mut inicio = 0.0;
    for f in ed.faixas {
        let faixa = match f.ate_kw {
            Some(ate) if inicio == 0.0 => format!("até {} kW", br(ate)),
            Some(ate) => format!("de {} a {} kW", br(inicio), br(ate)),
            None if inicio == 0.0 => "toda a potência".to_string(),
            None => format!("acima de {} kW", br(inicio)),
        };
        ui.label(format!("    {faixa}: {} %", br(f.fator * 100.0)));
        inicio = f.ate_kw.unwrap_or(inicio);
    }
    egui::CollapsingHeader::new("Aquecimento e eletrodomésticos (por número de aparelhos)").id_salt("ele-demanda-aparelhos").show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            egui::Grid::new("ele-demanda-tab4").num_columns(3).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                let lim = br(tabela.limite_aparelho_kw);
                cabecalho(ui, t, &[("Aparelhos", 80.0, true), ("Até 3,5 kW", 90.0, true), ("Acima", 90.0, true)]);
                let mut de = 1u32;
                for q in tabela.aparelhos {
                    let n = match q.ate {
                        u32::MAX => format!("{de} ou mais"),
                        a if a == de => a.to_string(),
                        a => format!("{de} a {a}"),
                    };
                    celula(ui, 80.0, numero(n), true);
                    celula(ui, 90.0, numero(fixo(q.ate_limite, 2)), true).on_hover_text(format!("potência individual até {lim} kW"));
                    celula(ui, 90.0, numero(fixo(q.acima_limite, 2)), true);
                    ui.end_row();
                    de = q.ate.saturating_add(1);
                }
            });
        });
    });
    egui::CollapsingHeader::new("Ar condicionado e bombas").id_salt("ele-demanda-ar").show(ui, |ui| {
        let lista = |ui: &mut egui::Ui, nome: &str, tab: &[ele::demanda::PorQuantidade]| {
            let mut de = 1u32;
            let partes: Vec<String> = tab
                .iter()
                .map(|q| {
                    let n = match q.ate {
                        u32::MAX => format!("{de}+"),
                        a if a == de => a.to_string(),
                        a => format!("{de}-{a}"),
                    };
                    de = q.ate.saturating_add(1);
                    format!("{n}: {}", fixo(q.fator, 2))
                })
                .collect();
            ui.label(format!("{nome}: {}", partes.join(" · ")));
        };
        lista(ui, "Ar condicionado", tabela.ar_condicionado);
        lista(ui, "Bombas d'água", tabela.bombas);
    });
    ui.add_space(4.0);
    ui.label(pequeno(format!("Fonte: {}", tabela.fonte), t.text_dim));
    ui.label(pequeno(
        "O tipo de carga de cada ponto vem do atributo TIPO_CARGA do bloco (ILUM_TUG, AQUECIMENTO, ELETRODOMESTICOS, AR, BOMBA, OUTROS, RESERVA; sem ele, TUE é aquecimento). Outras cargas entram com 100 % e reservas ficam fora. Confira a revisão da norma em vigor na sua área de concessão.",
        t.text_dim,
    ));
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
