// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. CRAFT ELE: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! CRAFT ELE: a paleta Elétrica e as suas janelas.
//!
//! A paleta só tem ferramentas. Criar circuito, Comandos e luminárias e Ajustar fiação são
//! comandos do motor (ELECIRCUITO, ELECOMANDO, ELEFIACAO) que pedem as seleções no desenho. As
//! janelas mostram os dados do projeto: Configurações (o setup), Quadro de cargas (os circuitos de
//! um quadro: criar, descrever, adicionar pontos, juntar, excluir), Conferência (pontos e
//! circuitos) e Rotear fiação (a tabela de resumo de um quadro). Toda alteração passa por um
//! comando do motor (`app.run`/`app.start`): a interface não decide nada de engenharia.
//!
//! Os textos ficam em português: o CRAFT ELE é um addon para projetos brasileiros e não passa
//! pelo catálogo de traduções do CADCraft. Só o nome do painel, o item de menu e os nomes dos
//! comandos passam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cadcraft_doc::Handle;
use cadcraft_ele::nbr5410::{DISJUNTORES, Isolacao, SECOES};
use cadcraft_ele::ponto::br;
use cadcraft_ele::setup::{DiametroCondutor, TamanhoEletroduto, diametros_padrao, eletrodutos_padrao};
use cadcraft_ele::{self as ele, CaboTelecom, CategoriaTelecom, CorCabo, PontoEletrico, Setup, Sistema, TipoPonto, Unidade};
use craft_ui::docking::{Action, Node};
use craft_ui::layout::SplitSize;
use egui::{Align, Color32, Layout, RichText, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::docking::Panel;
use crate::theme::Tokens;

const COR_AVISO: Color32 = Color32::from_rgb(232, 176, 72);
/// Altura das linhas das tabelas.
const LINHA: f32 = 20.0;

/// Chave das configurações do CRAFT ELE no armazenamento de preferências do CADCraft.
pub const PREFS_KEY: &str = "craftele.setup";

static JANELAS: Mutex<Janelas> = Mutex::new(Janelas { setup: None, conferencia: false, rotear: None, cargas: None });
static ABRIR_AO_INICIAR: AtomicBool = AtomicBool::new(false);

fn trava<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// As janelas abertas.
struct Janelas {
    /// Configurações: o rascunho até o OK.
    setup: Option<Rascunho>,
    conferencia: bool,
    /// Rotear fiação: o quadro escolhido e se já foi roteado.
    rotear: Option<Roteamento>,
    /// Quadro de cargas.
    cargas: Option<Cargas>,
}

#[derive(Clone)]
struct Rascunho {
    setup: Setup,
    aba: Aba,
    novo_cabo: CaboTelecom,
    erro: Option<String>,
}

#[derive(Clone, Copy, Default)]
struct Roteamento {
    quadro: Option<Handle>,
    roteado: bool,
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
    Telecom,
}

impl Aba {
    const TODAS: [Aba; 5] = [Aba::Geral, Aba::Condutores, Aba::Eletrodutos, Aba::Agrupamento, Aba::Telecom];
    fn nome(self) -> &'static str {
        match self {
            Aba::Geral => "Geral",
            Aba::Condutores => "Condutores",
            Aba::Eletrodutos => "Eletrodutos",
            Aba::Agrupamento => "Agrupamento",
            Aba::Telecom => "Telecom",
        }
    }
}

/// O que as janelas pedem para o motor fazer, executado depois de desenhá-las.
enum Acao {
    /// `app.run(comando, parâmetros)`.
    Rodar(&'static str, Value),
    /// `app.start(comando)`: começa um comando interativo.
    Iniciar(&'static str),
}

/// As configurações em uso (guardadas no motor: `cadcraft_ele::setup`).
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
            let mut caminho = Vec::new();
            if let Some(raiz) = &app.ui.docking.layout.root
                && caminho_da_paleta(raiz, &mut caminho)
            {
                let acao = Action::<Panel>::ResizeSplit { path: caminho, size: SplitSize::FixedFirst(330.0) };
                if let Ok(acao) = serde_json::to_value(acao) {
                    let _ = crate::docking::command(app, false, &json!({ "action": acao }));
                }
            }
            return Ok(r);
        }
    }
    Ok(r)
}

/// O caminho (`true` = segundo filho) até a divisão cuja primeira parte é a paleta Elétrica.
fn caminho_da_paleta(no: &Node<Panel>, caminho: &mut Vec<bool>) -> bool {
    let Node::Split { first, second, .. } = no else { return false };
    if matches!(first.as_ref(), Node::Tabs { panels, .. } if panels.contains(&Panel::Eletrica)) {
        return true;
    }
    for (lado, filho) in [(false, first), (true, second)] {
        caminho.push(lado);
        if caminho_da_paleta(filho, caminho) {
            return true;
        }
        caminho.pop();
    }
    false
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

/// Comandos do CRAFT ELE que só existem na interface: abrir a paleta e as janelas. `None`
/// quando `id` não é um deles.
///
/// Digitados: ELETRICA (paleta), ELECONFIG, ELECARGAS, ELECONFERIR e ELEROTEAR (janelas). Todo o
/// resto é comando do motor (`crates/engine/src/cmd/eletrica.rs`).
pub fn route(app: &mut CadApp, id: &str, _params: &Value) -> Option<Result<Value, String>> {
    match id {
        "ui.panel.eletrica" | "eletrica" | "craftele" => Some(open(app)),
        "ui.eletrica.setup" | "eleconfig" => Some(abrir_setup()),
        "ui.eletrica.conferir" | "eleconferir" => {
            trava(&JANELAS).conferencia = true;
            Some(Ok(Value::Null))
        }
        "ui.eletrica.rotear" | "elerotear" => {
            let mut j = trava(&JANELAS);
            if j.rotear.is_none() {
                j.rotear = Some(Roteamento::default());
            }
            Some(Ok(Value::Null))
        }
        "ui.eletrica.cargas" | "elecargas" => {
            let mut j = trava(&JANELAS);
            if j.cargas.is_none() {
                j.cargas = Some(Cargas::default());
            }
            Some(Ok(Value::Null))
        }
        _ => None,
    }
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

/// A isolação gravada no quadro (sem ela, vale a das configurações).
fn isolacao_gravada(q: &PontoEletrico) -> Option<Isolacao> {
    q.extras.iter().find(|(k, _)| k.eq_ignore_ascii_case(ele::quadro::CHAVE_ISOLACAO)).and_then(|(_, v)| Isolacao::de_codigo(v))
}

/// Escolha do quadro e da isolação dele (topo das janelas Quadro de cargas e Rotear fiação).
/// Devolve se o quadro mudou.
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

// ---------------------------------------------------------------- paleta (só ferramentas)

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let tem_desenho = app.session.state().is_ok();
    let mut comando: Option<&str> = None;
    egui::ScrollArea::vertical().id_salt("dock-eletrica").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            grupo(ui, &t, "Projeto");
            if ferramenta(ui, "Configurações...", true, "Setup do projeto: sistema, temperatura, isolação, cabos, eletrodutos, telecom (ELECONFIG)")
                .clicked()
            {
                comando = Some("ui.eletrica.setup");
            }
            grupo(ui, &t, "Circuitos");
            if ferramenta(
                ui,
                "Quadro de cargas...",
                tem_desenho,
                "Circuitos de um quadro: criar, descrever, adicionar pontos, juntar, excluir (ELECARGAS)",
            )
            .clicked()
            {
                comando = Some("ui.eletrica.cargas");
            }
            if ferramenta(ui, "Criar circuito", tem_desenho, "Selecione o quadro e depois os pontos do circuito, um a um (ELECIRCUITO)").clicked() {
                comando = Some("elecircuito");
            }
            if ferramenta(ui, "Comandos e luminárias", tem_desenho, "Selecione o interruptor e depois as luminárias que ele comanda (ELECOMANDO)")
                .clicked()
            {
                comando = Some("elecomando");
            }
            if ferramenta(ui, "Conferir pontos e circuitos...", tem_desenho, "Pontos elétricos, circuitos e atributos que faltam (ELECONFERIR)")
                .clicked()
            {
                comando = Some("ui.eletrica.conferir");
            }
            grupo(ui, &t, "Fiação");
            if ferramenta(ui, "Rotear fiação...", tem_desenho, "Escolha o quadro: roteia pelos eletrodutos e gera o resumo da fiação (ELEROTEAR)")
                .clicked()
            {
                comando = Some("ui.eletrica.rotear");
            }
            if ferramenta(ui, "Ajustar fiação", tem_desenho, "Desenha a simbologia de fiação e o diâmetro dos eletrodutos (ELEFIACAO)").clicked()
            {
                comando = Some("elefiacao");
            }
        });
    });
    if let Some(c) = comando {
        if c.starts_with("ui.") {
            let _ = app.run(c, Value::Null);
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

// ---------------------------------------------------------------- janelas

type ChaveResumo = ((u64, u64), Setup, Handle);

/// O que já foi lido do desenho aberto, refeito a cada edição.
#[derive(Clone, Default)]
struct Estado {
    /// (uid, revisão) do desenho analisado.
    chave: Option<(u64, u64)>,
    analise: Arc<ele::Analise>,
    resumo: Option<(ChaveResumo, Arc<Result<ele::ResumoQuadro, String>>)>,
}

impl Estado {
    fn atualizar(&mut self, app: &CadApp) {
        let chave = app.session.state().ok().map(|st| (st.uid, st.revision));
        if chave == self.chave {
            return;
        }
        self.chave = chave;
        self.analise = Arc::new(app.session.state().map(|st| ele::analisar(&st.doc)).unwrap_or_default());
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
}

fn id_estado() -> egui::Id {
    egui::Id::new("craft-ele-estado")
}

/// Desenha as janelas do CRAFT ELE (chamado a cada quadro, com ou sem a paleta visível).
pub fn janelas(app: &mut CadApp, ctx: &egui::Context) {
    if ABRIR_AO_INICIAR.swap(false, Ordering::Relaxed) {
        let panel = Panel::Eletrica;
        if !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel) {
            let _ = open(app);
        }
    }
    let (setup_aberta, mut conferencia, rotear, cargas) = {
        let j = trava(&JANELAS);
        (j.setup.is_some(), j.conferencia, j.rotear, j.cargas.clone())
    };
    let t = Tokens::get();
    if setup_aberta {
        janela_setup(ctx, &t);
    }
    if !conferencia && rotear.is_none() && cargas.is_none() {
        return;
    }
    let mut st: Estado = ctx.data_mut(|d| d.get_temp(id_estado())).unwrap_or_default();
    st.atualizar(app);
    let tem_desenho = st.chave.is_some();
    let s = setup();
    let mut selecionar: Option<Vec<Handle>> = None;
    let mut acoes: Vec<Acao> = Vec::new();

    if conferencia {
        egui::Window::new("CRAFT ELE · Conferência")
            .id(egui::Id::new("ele-janela-conferencia"))
            .open(&mut conferencia)
            .default_pos(egui::pos2(480.0, 140.0))
            .default_width(460.0)
            .show(ctx, |ui| {
                if !tem_desenho {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                    return;
                }
                egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                    secao_pontos(ui, &t, &st.analise);
                    ui.separator();
                    secao_circuitos(ui, &t, &st.analise, &mut selecionar);
                });
            });
    }

    let cargas = cargas.and_then(|mut c| {
        let mut aberta = true;
        egui::Window::new("CRAFT ELE · Quadro de cargas")
            .id(egui::Id::new("ele-janela-cargas"))
            .open(&mut aberta)
            .default_pos(egui::pos2(260.0, 110.0))
            .default_width(760.0)
            .show(ctx, |ui| {
                if !tem_desenho {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                    return;
                }
                if st.analise.quadros().is_empty() {
                    ui.label(pequeno("Nenhum quadro no desenho (bloco com TIPO_PONTO = QUADRO).", t.text_dim));
                    return;
                }
                quadro_de_cargas(ui, &t, &st.analise, &s, &mut c, &mut selecionar, &mut acoes);
            });
        aberta.then_some(c)
    });

    let rotear = rotear.and_then(|mut r| {
        let mut aberta = true;
        let quadros_vazios = st.analise.quadros().is_empty();
        egui::Window::new("CRAFT ELE · Rotear fiação")
            .id(egui::Id::new("ele-janela-rotear"))
            .open(&mut aberta)
            .default_pos(egui::pos2(240.0, 120.0))
            .default_width(1180.0)
            .show(ctx, |ui| {
                if !tem_desenho {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                    return;
                }
                if quadros_vazios {
                    ui.label(pequeno("Nenhum quadro no desenho (bloco com TIPO_PONTO = QUADRO).", t.text_dim));
                    return;
                }
                let analise = st.analise.clone();
                ui.horizontal(|ui| {
                    if seletor_quadro(ui, "ele-rotear", &analise, &s, &mut r.quadro, &mut acoes) {
                        r.roteado = false;
                    }
                    ui.add_space(8.0);
                    if ui.button("Rotear").clicked() {
                        r.roteado = true;
                    }
                });
                let resumo = match (r.roteado, r.quadro) {
                    (true, Some(q)) => st.resumo(app, &s, q),
                    _ => None,
                };
                match resumo.as_deref() {
                    None => {
                        ui.label(pequeno("Escolha o quadro e clique em Rotear.", t.text_dim));
                    }
                    Some(Err(e)) => {
                        ui.label(pequeno(e.as_str(), COR_AVISO));
                    }
                    Some(Ok(res)) => {
                        ui.separator();
                        egui::ScrollArea::both().max_height(620.0).show(ui, |ui| tabela_resumo(ui, &t, &s, res, &mut selecionar, &mut acoes));
                    }
                }
            });
        aberta.then_some(r)
    });

    {
        let mut j = trava(&JANELAS);
        j.conferencia = conferencia;
        j.rotear = rotear;
        j.cargas = cargas;
    }
    for a in acoes {
        match a {
            Acao::Rodar(id, p) => {
                if let Err(e) = app.run(id, p) {
                    app.set_status(format!("CRAFT ELE: {e}"));
                }
            }
            Acao::Iniciar(id) => app.start(id),
        }
    }
    if let Some(h) = selecionar {
        app.session.set_selection(h);
    }
    ctx.data_mut(|d| d.insert_temp(id_estado(), st));
}

/// A janela Quadro de cargas: os circuitos do quadro e as ações sobre eles.
fn quadro_de_cargas(
    ui: &mut egui::Ui,
    t: &Tokens,
    analise: &ele::Analise,
    s: &Setup,
    c: &mut Cargas,
    selecionar: &mut Option<Vec<Handle>>,
    acoes: &mut Vec<Acao>,
) {
    ui.horizontal(|ui| {
        if seletor_quadro(ui, "ele-cargas", analise, s, &mut c.quadro, acoes) {
            c.descricoes.clear();
            c.excluir = None;
        }
    });
    let Some(q) = c.quadro else { return };
    let circuitos = ele::quadro::circuitos_do_quadro(analise, q);
    ui.separator();

    // Novo circuito.
    ui.horizontal(|ui| {
        let proximo = ele::circuito::proximo_circuito(&analise.pontos, q);
        ui.label(format!("Novo circuito {proximo}:"));
        ui.add(egui::TextEdit::singleline(&mut c.nova_descricao).hint_text("Descrição (ex.: Tomadas da sala)").desired_width(260.0));
        if ui.button("Novo circuito").on_hover_text("Cria o circuito vazio; depois use Adicionar pontos").clicked() {
            acoes.push(Acao::Rodar("eletrica.circuito.criar", json!({ "quadro": q.hex(), "descricao": c.nova_descricao.trim() })));
            c.nova_descricao.clear();
        }
    });
    ui.add_space(4.0);

    if circuitos.is_empty() {
        ui.label(pequeno("Nenhum circuito neste quadro. Crie um acima ou use Criar circuito na paleta.", t.text_dim));
        return;
    }
    let colunas: [(&str, f32, bool); 7] = [
        ("Circ.", 44.0, false),
        ("Descrição", 220.0, false),
        ("Tipo", 80.0, false),
        ("Pontos", 52.0, true),
        ("Potência (VA)", 92.0, true),
        ("Fases", 50.0, true),
        ("", 260.0, false),
    ];
    let mut total = 0.0;
    egui::Grid::new("ele-cargas-tabela").num_columns(colunas.len()).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for item in &circuitos {
            let id = &item.id;
            let pontos = item.circuito.as_ref().map(|k| k.pontos.clone()).unwrap_or_default();
            let link = ui.allocate_ui_with_layout(vec2(colunas[0].1, LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(id.as_str())).inner;
            if link.on_hover_text("Selecionar os pontos deste circuito no desenho").clicked() {
                *selecionar = Some(pontos.clone());
            }
            // Descrição editável: grava ao sair do campo.
            let i = match c.descricoes.iter().position(|(k, _)| k == id) {
                Some(i) => i,
                None => {
                    c.descricoes.push((id.clone(), item.descricao.clone()));
                    c.descricoes.len() - 1
                }
            };
            if let Some((_, texto)) = c.descricoes.get_mut(i) {
                let r = ui.add_sized(vec2(colunas[1].1, LINHA), egui::TextEdit::singleline(texto).hint_text("sem descrição"));
                if r.lost_focus() && *texto != item.descricao {
                    acoes.push(Acao::Rodar("eletrica.circuito.descrever", json!({ "quadro": q.hex(), "circuito": id, "descricao": texto.trim() })));
                }
            }
            match &item.circuito {
                Some(k) => {
                    total += k.potencia;
                    celula(ui, colunas[2].1, k.natureza.nome(), false);
                    celula(ui, colunas[3].1, numero(k.pontos.len().to_string()), true);
                    celula(ui, colunas[4].1, numero(fixo(k.potencia, 0)), true);
                    celula(ui, colunas[5].1, numero(k.fases.to_string()), true);
                }
                None => {
                    celula(ui, colunas[2].1, pequeno("vazio", t.text_dim), false);
                    celula(ui, colunas[3].1, numero("0"), true);
                    celula(ui, colunas[4].1, numero("-"), true);
                    celula(ui, colunas[5].1, numero("-"), true);
                }
            }
            ui.horizontal(|ui| {
                if ui.small_button("Adicionar pontos").on_hover_text("Clique nos pontos no desenho; Enter termina").clicked() {
                    acoes.push(Acao::Rodar("eletrica.circuito.alvo", json!({ "quadro": q.hex(), "circuito": id })));
                    acoes.push(Acao::Iniciar("elecircuito"));
                }
                egui::ComboBox::from_id_salt(("ele-juntar", id.as_str())).selected_text("Juntar em").width(80.0).show_ui(ui, |ui| {
                    for outro in circuitos.iter().filter(|o| o.id != *id) {
                        if ui.selectable_label(false, format!("{} {}", outro.id, outro.descricao)).clicked() {
                            acoes.push(Acao::Rodar("eletrica.circuito.juntar", json!({ "quadro": q.hex(), "de": id, "para": outro.id })));
                        }
                    }
                });
                if c.excluir.as_deref() == Some(id.as_str()) {
                    if ui.small_button(RichText::new("Confirmar exclusão").color(COR_AVISO)).clicked() {
                        acoes.push(Acao::Rodar("eletrica.circuito.excluir", json!({ "quadro": q.hex(), "circuito": id })));
                        c.excluir = None;
                    }
                } else if ui.small_button("Excluir").on_hover_text("Os pontos ficam sem circuito (Ctrl+Z desfaz)").clicked() {
                    c.excluir = Some(id.clone());
                }
            });
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(format!("{} circuito(s) · potência instalada {} VA", circuitos.len(), fixo(total, 0)));
}

fn tabela_resumo(ui: &mut egui::Ui, t: &Tokens, s: &Setup, r: &ele::ResumoQuadro, selecionar: &mut Option<Vec<Handle>>, acoes: &mut Vec<Acao>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Resumo da fiação: {}", r.tag)).strong().color(t.text));
        if ui.button("Copiar resumo").on_hover_text("Copia o resumo técnico em texto").clicked() {
            ui.ctx().copy_text(ele::quadro::texto(r, s));
        }
    });
    ui.label(pequeno(
        format!(
            "{} · condutores {} · {} °C · queda admissível {} % · FCA pelo agrupamento nos eletrodutos ({} trecho(s); 1 unidade = {} m)",
            s.sistema.nome(),
            r.isolacao.nome(),
            br(s.temperatura_ambiente),
            br(s.queda_admissivel),
            r.trechos_eletroduto,
            br(r.metros_por_unidade),
        ),
        t.text_dim,
    ));
    ui.add_space(4.0);
    let colunas: [(&str, f32, bool); 14] = [
        ("Circ.", 40.0, false),
        ("Descrição", 130.0, false),
        ("Tipo", 70.0, false),
        ("Potência (VA)", 84.0, true),
        ("Fases", 56.0, false),
        ("Ib (A)", 50.0, true),
        ("Compr. (m)", 66.0, true),
        ("FCT", 36.0, true),
        ("FCA", 36.0, true),
        ("Seção (mm²)", 84.0, false),
        ("Disjuntor (A)", 84.0, false),
        ("Iz (A)", 50.0, true),
        ("Queda (%)", 64.0, true),
        ("Situação", 70.0, false),
    ];
    egui::Grid::new("ele-resumo").num_columns(colunas.len()).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        cabecalho(ui, t, &colunas);
        for l in &r.linhas {
            let e = &l.escolha;
            let c = &l.circuito;
            let mut avisos = l.automatico.avisos.clone();
            avisos.extend(e.avisos.iter().cloned());
            avisos.extend(c.avisos.iter().cloned());
            let link = ui.allocate_ui_with_layout(vec2(colunas[0].1, LINHA), Layout::left_to_right(Align::Center), |ui| ui.link(c.id.as_str())).inner;
            if link.on_hover_text("Selecionar os pontos deste circuito no desenho").clicked() {
                *selecionar = Some(c.pontos.clone());
            }
            celula(ui, colunas[1].1, if l.descricao.is_empty() { pequeno("-", t.text_dim) } else { RichText::new(&l.descricao) }, false);
            celula(ui, colunas[2].1, c.natureza.nome(), false);
            celula(ui, colunas[3].1, numero(fixo(c.potencia, 0)), true);
            celula(ui, colunas[4].1, l.fases_texto(), false);
            celula(ui, colunas[5].1, numero(fixo(e.ib, 1)), true);
            let comprimento = l.comprimento.map_or("-".to_string(), |v| fixo(v, 1));
            if l.fora_da_rede > 0 {
                celula(ui, colunas[6].1, numero(format!("{comprimento}*")), true)
                    .on_hover_text(format!("{} ponto(s) fora da rede de eletrodutos: comprimento estimado em L", l.fora_da_rede));
            } else {
                celula(ui, colunas[6].1, numero(comprimento), true);
            }
            celula(ui, colunas[7].1, numero(fixo(l.fct, 2)), true);
            celula(ui, colunas[8].1, numero(fixo(l.fca, 2)), true)
                .on_hover_text(format!("{} circuito(s) no trecho mais cheio (tabela 42)", l.agrupados));
            // Seção e disjuntor: a pré-seleção, que o projetista pode trocar.
            let mut sob = l.sobrescrita;
            let antes = sob;
            ui.allocate_ui_with_layout(vec2(colunas[9].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
                escolher(ui, ("ele-secao", &c.id), &mut sob.secao, l.automatico.secao, &SECOES);
            });
            ui.allocate_ui_with_layout(vec2(colunas[10].1, LINHA), Layout::left_to_right(Align::Center), |ui| {
                escolher(ui, ("ele-disj", &c.id), &mut sob.disjuntor, l.automatico.disjuntor, &DISJUNTORES);
            });
            if sob != antes {
                acoes.push(Acao::Rodar(
                    "eletrica.sobrescrever",
                    json!({ "quadro": r.quadro.hex(), "circuito": c.id, "secao": sob.secao, "disjuntor": sob.disjuntor }),
                ));
            }
            celula(ui, colunas[11].1, numero(e.iz.map_or("-".to_string(), |v| fixo(v, 1))), true);
            celula(ui, colunas[12].1, numero(e.queda.map_or("-".to_string(), |q| fixo(q, 2))), true)
                .on_hover_text(format!("Admissível: {} %", br(l.queda_admissivel)));
            if avisos.is_empty() {
                celula(ui, colunas[13].1, "OK", false);
            } else {
                celula(ui, colunas[13].1, RichText::new(format!("{} aviso(s)", avisos.len())).color(COR_AVISO), false)
                    .on_hover_text(avisos.join("\n"));
            }
            ui.end_row();
        }
    });
    ui.label(pequeno(
        "Seção e disjuntor mostram a pré-seleção; escolher outro valor grava a escolha no quadro (Ctrl+Z desfaz) e \"Auto\" volta à pré-seleção. * comprimento estimado.",
        t.text_dim,
    ));
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("Totais").strong());
            egui::Grid::new("ele-resumo-totais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                ui.label("Potência instalada");
                celula(ui, 90.0, numero(format!("{} VA", fixo(r.potencia_total, 0))), true);
                ui.end_row();
                for (f, nome) in ele::quadro::NOMES_FASES.iter().enumerate().take(usize::from(r.fases_quadro)) {
                    ui.label(format!("Fase {nome}"));
                    celula(ui, 90.0, numero(format!("{} VA", fixo(r.carga_por_fase.get(f).copied().unwrap_or(0.0), 0))), true);
                    ui.end_row();
                }
                if r.fases_quadro > 1 {
                    ui.label("Desequilíbrio");
                    celula(ui, 90.0, numero(format!("{} %", fixo(r.desequilibrio, 1))), true);
                    ui.end_row();
                }
            });
        });
        ui.add_space(32.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("Cabos (fator {})", br(s.fator_materiais))).strong());
            egui::Grid::new("ele-resumo-materiais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
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

/// Uma caixa de escolha com "Auto" (a pré-seleção) e os valores da tabela.
fn escolher(ui: &mut egui::Ui, id: (&str, &String), valor: &mut Option<f64>, automatico: Option<f64>, opcoes: &[f64]) {
    let auto = automatico.map_or("-".to_string(), br);
    let texto = match valor {
        Some(v) => RichText::new(format!("{} (à mão)", br(*v))).strong(),
        None => RichText::new(auto.clone()),
    };
    egui::ComboBox::from_id_salt(id).selected_text(texto).width(78.0).show_ui(ui, |ui| {
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

fn janela_setup(ctx: &egui::Context, t: &Tokens) {
    let Some(mut r) = trava(&JANELAS).setup.clone() else { return };
    let mut aberta = true;
    let mut fechar = false;
    egui::Window::new("CRAFT ELE · Configurações")
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
        ui.add(campo(&mut s.raio_captura, 0.01..=5.0, 0.01, 2, " m")).on_hover_text("Distância máxima entre um ponto e o eletroduto que chega nele");
        ui.end_row();
    });
    ui.label(pequeno(
        "Tensões e fases valem quando o bloco não informa TENSAO ou o quadro não informa NUMERO_FASES. Os eletrodutos correm no pé-direito (ou no piso); cada ponto desce ou sobe até a sua ALTURA.",
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
        for (f, nome) in ["Cor da fase A", "Cor da fase B", "Cor da fase C"].into_iter().enumerate() {
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
        "Ajustar fiação escolhe, em cada trecho, o menor eletroduto a partir do mínimo que comporta os condutores: ocupação máxima de 53 % com um condutor, 31 % com dois e 40 % com três ou mais (NBR 5410, 6.2.11.1.6). Diâmetros internos típicos de catálogo; confira com o fabricante.",
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

fn secao_circuitos(ui: &mut egui::Ui, t: &Tokens, a: &ele::Analise, selecionar: &mut Option<Vec<Handle>>) {
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
                *selecionar = Some(c.pontos.clone());
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
