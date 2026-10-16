//! CRAFT ELE: a paleta Elétrica e as suas janelas.
//!
//! A paleta só tem ferramentas, como as Tool Sets. Os dados do projeto aparecem nas janelas que
//! as ferramentas abrem: Configurações (o setup do projeto, de onde saem tensões, fases e fatores
//! de correção), Conferência (pontos e circuitos) e Quadro de cargas.
//!
//! Os textos ficam em português: o CRAFT ELE é um addon para projetos brasileiros e não passa
//! pelo catálogo de traduções do CADCraft. Só o nome do painel e o item de menu passam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use cadcraft_doc::{Drawing, Handle};
use cadcraft_ele::{self as ele, Setup, Sistema, TipoPonto};
use craft_ui::docking::{Action, Node};
use craft_ui::layout::SplitSize;
use egui::{Color32, RichText, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::docking::Panel;
use crate::theme::Tokens;

const COR_AVISO: Color32 = Color32::from_rgb(232, 176, 72);

/// Chave das configurações do CRAFT ELE no armazenamento de preferências do CADCraft.
pub const PREFS_KEY: &str = "craftele.setup";

static SETUP: LazyLock<Mutex<Setup>> = LazyLock::new(|| Mutex::new(Setup::default()));
static JANELAS: Mutex<Janelas> = Mutex::new(Janelas { setup: None, conferencia: false, quadro: false });
static ABRIR_AO_INICIAR: AtomicBool = AtomicBool::new(false);

fn trava<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// As janelas abertas. A de configurações guarda o rascunho até o OK.
struct Janelas {
    setup: Option<Rascunho>,
    conferencia: bool,
    quadro: bool,
}

#[derive(Clone)]
struct Rascunho {
    setup: Setup,
    aba: Aba,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Aba {
    Geral,
    Condutores,
    Agrupamento,
}

impl Aba {
    const TODAS: [Aba; 3] = [Aba::Geral, Aba::Condutores, Aba::Agrupamento];
    fn nome(self) -> &'static str {
        match self {
            Aba::Geral => "Geral",
            Aba::Condutores => "Condutores",
            Aba::Agrupamento => "Agrupamento",
        }
    }
}

/// As configurações em uso.
pub fn setup() -> Setup {
    trava(&SETUP).clone()
}

fn definir_setup(mut s: Setup) {
    s.temperatura_ambiente = s.temperatura_ambiente.clamp(10.0, 60.0);
    *trava(&SETUP) = s;
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
                let acao = Action::<Panel>::ResizeSplit { path: caminho, size: SplitSize::FixedFirst(250.0) };
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
    let rascunho = Rascunho { setup: setup(), aba: Aba::Geral };
    trava(&JANELAS).setup = Some(rascunho);
    Ok(Value::Null)
}

/// Comandos do CRAFT ELE. `None` quando `id` não é um deles.
///
/// Digitados: ELETRICA (paleta), ELECONFIG, ELECONFERIR e ELEQUADRO (janelas).
/// JSON, para o canal de controle, o MCP e scripts: `eletrica.analisar`,
/// `eletrica.dimensionar` e `eletrica.setup` (sem parâmetros lê; com `{sistema,
/// temperaturaAmbiente}` grava).
pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    match id {
        "ui.panel.eletrica" | "eletrica" | "craftele" => Some(open(app)),
        "ui.eletrica.setup" | "eleconfig" => Some(abrir_setup()),
        "ui.eletrica.conferir" | "eleconferir" => {
            trava(&JANELAS).conferencia = true;
            Some(Ok(Value::Null))
        }
        "ui.eletrica.quadro" | "elequadro" => {
            trava(&JANELAS).quadro = true;
            Some(Ok(Value::Null))
        }
        "eletrica.analisar" => Some(com_desenho(app, |doc| serde_json::to_value(ele::analisar(doc)).map_err(|e| e.to_string()))),
        "eletrica.dimensionar" => {
            let cfg = config(params);
            Some(com_desenho(app, |doc| serde_json::to_value(ele::dimensionar_quadro(&ele::analisar(doc), &cfg)).map_err(|e| e.to_string())))
        }
        "eletrica.setup" => Some(setup_json(params)),
        _ => None,
    }
}

fn com_desenho(app: &CadApp, f: impl FnOnce(&Drawing) -> Result<Value, String>) -> Result<Value, String> {
    let st = app.session.state().map_err(|e| e.to_string())?;
    f(&st.doc)
}

/// Os parâmetros das configurações, com `{tensaoFn, fct, fca}` de `params` por cima (testes e
/// automação).
fn config(params: &Value) -> ele::Config {
    let mut c = setup().config();
    let num = |k: &str| params.get(k).and_then(Value::as_f64);
    if let Some(v) = num("tensaoFn") {
        c.tensao_fn = v;
    }
    if let Some(v) = num("fct") {
        c.fct = v;
    }
    if let Some(v) = num("fca") {
        c.fca = v;
    }
    c
}

fn setup_json(params: &Value) -> Result<Value, String> {
    let mut v = serde_json::to_value(setup()).map_err(|e| e.to_string())?;
    if let (Some(atual), Some(novo)) = (v.as_object_mut(), params.as_object())
        && !novo.is_empty()
    {
        for (k, valor) in novo {
            atual.insert(k.clone(), valor.clone());
        }
        definir_setup(serde_json::from_value(v).map_err(|e| e.to_string())?);
        v = serde_json::to_value(setup()).map_err(|e| e.to_string())?;
    }
    Ok(v)
}

/// Número com vírgula decimal.
fn br(v: f64, casas: usize) -> String {
    format!("{v:.casas$}").replace('.', ",")
}

fn pequeno(texto: impl Into<String>, cor: Color32) -> RichText {
    RichText::new(texto).font(crate::theme::small()).color(cor)
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
            if ferramenta(ui, "Configurações...", true, "Sistema de distribuição, condutores e temperatura (ELECONFIG)").clicked() {
                comando = Some("ui.eletrica.setup");
            }
            grupo(ui, &t, "Circuitos");
            if ferramenta(ui, "Conferir pontos e circuitos...", tem_desenho, "Pontos elétricos, circuitos e atributos que faltam (ELECONFERIR)")
                .clicked()
            {
                comando = Some("ui.eletrica.conferir");
            }
            grupo(ui, &t, "Quadro");
            if ferramenta(ui, "Dimensionar quadro...", tem_desenho, "Quadro de cargas: corrente, condutor, disjuntor e fases (ELEQUADRO)").clicked() {
                comando = Some("ui.eletrica.quadro");
            }
            grupo(ui, &t, "Fiação");
            ferramenta(ui, "Ajustar fiação", false, "Próxima etapa: precisa do traçado dos eletrodutos ligado aos pontos.");
        });
    });
    if let Some(c) = comando {
        let _ = app.run(c, Value::Null);
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

/// O que já foi lido do desenho aberto, refeito a cada edição.
#[derive(Clone, Default)]
struct Estado {
    /// (uid, revisão) do desenho analisado.
    chave: Option<(u64, u64)>,
    analise: Arc<ele::Analise>,
    /// O quadro e as configurações com que foi calculado.
    quadro: Option<(Setup, Arc<ele::QuadroDeCargas>)>,
}

impl Estado {
    fn atualizar(&mut self, app: &CadApp) {
        let chave = app.session.state().ok().map(|st| (st.uid, st.revision));
        if chave == self.chave {
            return;
        }
        self.chave = chave;
        self.analise = Arc::new(app.session.state().map(|st| ele::analisar(&st.doc)).unwrap_or_default());
        self.quadro = None;
    }

    fn quadro(&mut self, s: &Setup) -> Arc<ele::QuadroDeCargas> {
        if let Some((calculado, q)) = &self.quadro
            && calculado == s
        {
            return q.clone();
        }
        let q = Arc::new(ele::dimensionar_quadro(&self.analise, &s.config()));
        self.quadro = Some((s.clone(), q.clone()));
        q
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
    let (setup_aberta, mut conferencia, mut quadro) = {
        let j = trava(&JANELAS);
        (j.setup.is_some(), j.conferencia, j.quadro)
    };
    if !(setup_aberta || conferencia || quadro) {
        return;
    }
    let t = Tokens::get();
    if setup_aberta {
        janela_setup(ctx, &t);
    }
    if !(conferencia || quadro) {
        return;
    }
    let mut st: Estado = ctx.data_mut(|d| d.get_temp(id_estado())).unwrap_or_default();
    st.atualizar(app);
    let tem_desenho = st.chave.is_some();
    let mut selecionar: Option<Vec<Handle>> = None;
    if conferencia {
        egui::Window::new("CRAFT ELE · Conferência")
            .id(egui::Id::new("ele-janela-conferencia"))
            .open(&mut conferencia)
            .default_pos(egui::pos2(480.0, 140.0))
            .default_width(420.0)
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
    if quadro {
        let s = setup();
        let q = tem_desenho.then(|| st.quadro(&s));
        egui::Window::new("CRAFT ELE · Quadro de cargas")
            .id(egui::Id::new("ele-janela-quadro"))
            .open(&mut quadro)
            .default_pos(egui::pos2(540.0, 180.0))
            .default_width(520.0)
            .show(ctx, |ui| match &q {
                None => {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                }
                Some(q) if q.linhas.is_empty() => {
                    ui.label(pequeno("Nenhum ponto com o atributo CIRCUITO no desenho.", t.text_dim));
                }
                Some(q) => {
                    egui::ScrollArea::both().max_height(560.0).show(ui, |ui| secao_quadro(ui, &t, &s, q, &mut selecionar));
                }
            });
    }
    {
        let mut j = trava(&JANELAS);
        j.conferencia = conferencia;
        j.quadro = quadro;
    }
    if let Some(h) = selecionar {
        app.session.set_selection(h);
    }
    ctx.data_mut(|d| d.insert_temp(id_estado(), st));
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
        .default_pos(egui::pos2(480.0, 140.0))
        .default_width(400.0)
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
                Aba::Agrupamento => aba_agrupamento(ui, t),
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
        ui.label("Tensão fase-neutro");
        ui.label(format!("{} V", br(s.sistema.tensao_fn(), 0)));
        ui.end_row();
        ui.label("Tensão fase-fase");
        ui.label(s.sistema.tensao_ff().map_or_else(|| "-".to_string(), |v| format!("{} V", br(v, 0))));
        ui.end_row();
        ui.label("Fases do quadro");
        ui.label(s.sistema.fases().to_string());
        ui.end_row();
    });
    ui.label(pequeno("Usados quando o bloco não informa TENSAO ou o quadro não informa NUMERO_FASES.", t.text_dim));
}

fn aba_condutores(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-condutores").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Material");
        ui.label("Cobre");
        ui.end_row();
        ui.label("Isolação");
        ui.label("PVC 70 °C");
        ui.end_row();
        ui.label("Método de instalação");
        ui.label("B1: condutores isolados em eletroduto");
        ui.end_row();
        ui.label("Temperatura ambiente");
        ui.add(egui::DragValue::new(&mut s.temperatura_ambiente).range(10.0..=60.0).speed(1.0).suffix(" °C"));
        ui.end_row();
        ui.label("FCT (tabela 40)");
        ui.label(br(s.fct(), 2));
        ui.end_row();
    });
    ui.label(pequeno("Outros materiais, isolações e métodos de instalação virão nas próximas versões.", t.text_dim));
}

fn aba_agrupamento(ui: &mut egui::Ui, t: &Tokens) {
    egui::Grid::new("ele-setup-agrupamento").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("FCA (tabela 42)");
        ui.label("automático");
        ui.end_row();
    });
    ui.label(pequeno(
        "O fator de agrupamento sai do número de circuitos que passam em cada eletroduto. Enquanto o traçado dos eletrodutos não é lido, o cálculo usa FCA = 1,00.",
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
                ui.label(n.to_string());
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
        ui.label(pequeno("Nenhum ponto com o atributo CIRCUITO.", t.text_dim));
        return;
    }
    egui::Grid::new("ele-circuitos").num_columns(5).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        for h in ["Circ.", "Tipo", "Pontos", "VA", "V"] {
            ui.label(pequeno(h, t.text_dim));
        }
        ui.end_row();
        for c in &a.circuitos {
            if ui.link(c.id.as_str()).on_hover_text("Selecionar os pontos deste circuito").clicked() {
                *selecionar = Some(c.pontos.clone());
            }
            ui.label(c.natureza.nome());
            ui.label(c.pontos.len().to_string());
            ui.label(br(c.potencia, 0));
            ui.label(c.tensao.map(|v| br(v, 0)).unwrap_or_else(|| "?".into()));
            ui.end_row();
        }
    });
    let avisos: Vec<String> = a.circuitos.iter().flat_map(|c| c.avisos.iter().map(move |w| format!("Circuito {}: {w}", c.id))).collect();
    lista_avisos(ui, "ele-avisos-circuitos", &avisos);
}

fn secao_quadro(ui: &mut egui::Ui, t: &Tokens, s: &Setup, q: &ele::QuadroDeCargas, selecionar: &mut Option<Vec<Handle>>) {
    let nome = q.quadro.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
    titulo(ui, t, &format!("Quadro de cargas{nome}"));
    ui.label(pequeno(
        format!(
            "{} · {} °C, FCT {} · FCA 1,00 (automático: aguardando os eletrodutos)",
            s.sistema.nome(),
            br(s.temperatura_ambiente, 0),
            br(s.fct(), 2)
        ),
        t.text_dim,
    ));
    egui::Grid::new("ele-quadro").num_columns(7).striped(true).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
        for h in ["Circ.", "VA", "Fases", "Ib (A)", "mm²", "Disj. (A)", "Iz (A)"] {
            ui.label(pequeno(h, t.text_dim));
        }
        ui.end_row();
        for l in &q.linhas {
            let d = &l.dimensionamento;
            if ui.link(l.circuito.id.as_str()).on_hover_text("Selecionar os pontos deste circuito").clicked() {
                *selecionar = Some(l.circuito.pontos.clone());
            }
            ui.label(br(l.circuito.potencia, 0));
            ui.label(l.fases_texto());
            ui.label(br(d.ib, 1));
            ui.label(d.secao.map(|s| br(s, 1)).unwrap_or_else(|| "?".into()));
            ui.label(d.disjuntor.map(|v| br(v, 0)).unwrap_or_else(|| "?".into()));
            ui.label(d.iz.map(|v| br(v, 1)).unwrap_or_else(|| "?".into()));
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    egui::Grid::new("ele-totais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
        ui.label("Potência instalada");
        ui.label(format!("{} VA", br(q.potencia_total, 0)));
        ui.end_row();
        for (f, nome) in ele::quadro::NOMES_FASES.iter().enumerate().take(usize::from(q.fases_quadro)) {
            ui.label(format!("Fase {nome}"));
            ui.label(format!("{} VA", br(q.carga_por_fase[f], 0)));
            ui.end_row();
        }
        if q.fases_quadro > 1 {
            ui.label("Desequilíbrio");
            ui.label(format!("{} %", br(q.desequilibrio, 1)));
            ui.end_row();
        }
    });
    let mut avisos = q.avisos.clone();
    for l in &q.linhas {
        avisos.extend(l.dimensionamento.avisos.iter().map(|w| format!("Circuito {}: {w}", l.circuito.id)));
    }
    lista_avisos(ui, "ele-avisos-quadro", &avisos);
}
