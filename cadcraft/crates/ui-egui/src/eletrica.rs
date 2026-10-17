//! CRAFT ELE: a paleta Elétrica e as suas janelas.
//!
//! A paleta só tem ferramentas. Criar circuito, Comandos e luminárias e Ajustar fiação são
//! comandos do motor (ELECIRCUITO, ELECOMANDO, ELEFIACAO) que pedem as seleções no desenho; a
//! interface não calcula nada que o motor não exponha também como comando. Os dados do projeto aparecem nas
//! janelas: Configurações (o setup, com a aba de telecom), Conferência (pontos e circuitos) e
//! Rotear fiação (a tabela de resumo de um quadro, com a pré-seleção automática de seção e
//! disjuntor que o projetista pode sobrescrever).
//!
//! Os textos ficam em português: o CRAFT ELE é um addon para projetos brasileiros e não passa
//! pelo catálogo de traduções do CADCraft. Só o nome do painel, o item de menu e os nomes dos
//! comandos passam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cadcraft_doc::Handle;
use cadcraft_ele::nbr5410::{DISJUNTORES, SECOES};
use cadcraft_ele::ponto::br;
use cadcraft_ele::{self as ele, CaboTelecom, CategoriaTelecom, CorCabo, Setup, Sistema, TipoPonto, Unidade};
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

static JANELAS: Mutex<Janelas> = Mutex::new(Janelas { setup: None, conferencia: false, rotear: None });
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Aba {
    Geral,
    Condutores,
    Agrupamento,
    Telecom,
}

impl Aba {
    const TODAS: [Aba; 4] = [Aba::Geral, Aba::Condutores, Aba::Agrupamento, Aba::Telecom];
    fn nome(self) -> &'static str {
        match self {
            Aba::Geral => "Geral",
            Aba::Condutores => "Condutores",
            Aba::Agrupamento => "Agrupamento",
            Aba::Telecom => "Telecom",
        }
    }
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
/// Digitados: ELETRICA (paleta), ELECONFIG, ELECONFERIR e ELEROTEAR (janelas). Todo o resto é
/// comando do motor (`crates/engine/src/cmd/eletrica.rs`): ELECIRCUITO, ELECOMANDO, ELEFIACAO e
/// os `eletrica.*` em JSON.
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
        _ => None,
    }
}

/// A tabela de Rotear fiação age pelo comando do motor, como a linha de comando faria.
fn sobrescrever(app: &mut CadApp, quadro: Handle, circuito: &str, s: ele::Sobrescrita) -> Result<Value, String> {
    app.run("eletrica.sobrescrever", json!({ "quadro": quadro.hex(), "circuito": circuito, "secao": s.secao, "disjuntor": s.disjuntor }))
}

fn pequeno(texto: impl Into<String>, cor: Color32) -> RichText {
    RichText::new(texto).font(crate::theme::small()).color(cor)
}

/// Campo numérico com vírgula decimal ("2,80 m"); aceita digitar com vírgula ou ponto.
fn campo<'a>(valor: &'a mut f64, faixa: std::ops::RangeInclusive<f64>, passo: f64, casas: usize, sufixo: &'static str) -> egui::DragValue<'a> {
    egui::DragValue::new(valor)
        .range(faixa)
        .speed(passo)
        .custom_formatter(move |n, _| format!("{}{sufixo}", format!("{n:.casas$}").replace('.', ",")))
        .custom_parser(ele::ponto::numero)
}

/// Etiqueta com o fundo na cor do cabo e o texto em contraste (o preto aparece no tema escuro).
fn etiqueta_cor(texto: &str, c: CorCabo) -> RichText {
    let [r, g, b] = c.rgb();
    let claro = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b) > 140.0;
    RichText::new(format!(" {texto} ")).background_color(Color32::from_rgb(r, g, b)).color(if claro { Color32::BLACK } else { Color32::WHITE })
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
            if ferramenta(ui, "Configurações...", true, "Setup do projeto: sistema, temperatura, pé-direito, cores das fases, telecom (ELECONFIG)")
                .clicked()
            {
                comando = Some("ui.eletrica.setup");
            }
            grupo(ui, &t, "Circuitos");
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
            if ferramenta(
                ui,
                "Ajustar fiação",
                tem_desenho,
                "Desenha as marcas de fiação (fase, neutro, retorno, proteção) sobre os eletrodutos (ELEFIACAO)",
            )
            .clicked()
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
    let (setup_aberta, mut conferencia, rotear) = {
        let j = trava(&JANELAS);
        (j.setup.is_some(), j.conferencia, j.rotear)
    };
    let t = Tokens::get();
    if setup_aberta {
        janela_setup(ctx, &t);
    }
    if !conferencia && rotear.is_none() {
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
    let mut pedidos: Vec<(Handle, String, ele::Sobrescrita)> = Vec::new();
    let rotear = rotear.map(|mut r| {
        let mut aberta = true;
        let s = setup();
        let quadros: Vec<(Handle, String)> = st.analise.quadros().iter().map(|q| (q.handle, q.rotulo())).collect();
        if r.quadro.is_none_or(|h| !quadros.iter().any(|(q, _)| *q == h)) {
            r.quadro = quadros.first().map(|(h, _)| *h);
            r.roteado = false;
        }
        let resumo = match (r.roteado, r.quadro) {
            (true, Some(q)) => st.resumo(app, &s, q),
            _ => None,
        };
        egui::Window::new("CRAFT ELE · Rotear fiação")
            .id(egui::Id::new("ele-janela-rotear"))
            .open(&mut aberta)
            .default_pos(egui::pos2(300.0, 120.0))
            .default_width(940.0)
            .show(ctx, |ui| {
                if !tem_desenho {
                    ui.label(pequeno("Nenhum desenho aberto.", t.text_dim));
                    return;
                }
                if quadros.is_empty() {
                    ui.label(pequeno("Nenhum quadro no desenho (bloco com TIPO_PONTO = QUADRO).", t.text_dim));
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label("Quadro");
                    let atual = quadros.iter().find(|(h, _)| Some(*h) == r.quadro).map_or(String::new(), |(_, n)| n.clone());
                    egui::ComboBox::from_id_salt("ele-rotear-quadro").selected_text(atual).width(200.0).show_ui(ui, |ui| {
                        for (h, nome) in &quadros {
                            if ui.selectable_label(r.quadro == Some(*h), nome).clicked() {
                                r.quadro = Some(*h);
                                r.roteado = false;
                            }
                        }
                    });
                    if ui.button("Rotear").clicked() {
                        r.roteado = true;
                    }
                    if let Some(res) = resumo.as_deref()
                        && let Ok(res) = res
                        && ui.button("Copiar resumo").on_hover_text("Copia o resumo técnico em texto").clicked()
                    {
                        ui.ctx().copy_text(ele::quadro::texto(res, &s));
                    }
                });
                match resumo.as_deref() {
                    None => {
                        ui.label(pequeno("Escolha o quadro e clique em Rotear.", t.text_dim));
                    }
                    Some(Err(e)) => {
                        ui.label(pequeno(e.as_str(), COR_AVISO));
                    }
                    Some(Ok(res)) => {
                        egui::ScrollArea::both().max_height(600.0).show(ui, |ui| tabela_resumo(ui, &t, &s, res, &mut selecionar, &mut pedidos));
                    }
                }
            });
        // Rotear de novo quando o quadro muda é pedido pelo botão; o resumo já roteado se refaz sozinho.
        aberta.then_some(r)
    });
    {
        let mut j = trava(&JANELAS);
        j.conferencia = conferencia;
        j.rotear = rotear.flatten();
    }
    for (q, c, s) in pedidos {
        if let Err(e) = sobrescrever(app, q, &c, s) {
            app.set_status(format!("CRAFT ELE: {e}"));
        }
    }
    if let Some(h) = selecionar {
        app.session.set_selection(h);
    }
    ctx.data_mut(|d| d.insert_temp(id_estado(), st));
}

fn tabela_resumo(
    ui: &mut egui::Ui,
    t: &Tokens,
    s: &Setup,
    r: &ele::ResumoQuadro,
    selecionar: &mut Option<Vec<Handle>>,
    pedidos: &mut Vec<(Handle, String, ele::Sobrescrita)>,
) {
    ui.label(RichText::new(format!("Resumo da fiação: {}", r.tag)).strong().color(t.text));
    ui.label(pequeno(
        format!(
            "{} · {} °C, FCT {} · FCA pelo agrupamento nos eletrodutos · queda admissível {} % · {} trecho(s) de eletroduto · 1 unidade = {} m",
            s.sistema.nome(),
            br(s.temperatura_ambiente),
            br(s.fct()),
            br(s.queda_admissivel),
            r.trechos_eletroduto,
            br(r.metros_por_unidade),
        ),
        t.text_dim,
    ));
    egui::Grid::new("ele-resumo").num_columns(13).striped(true).spacing(vec2(10.0, 3.0)).show(ui, |ui| {
        for h in ["Circ.", "Tipo", "VA", "Fases", "Ib (A)", "FCT", "FCA", "L (m)", "ΔV (%)", "Seção (mm²)", "Disj. (A)", "Iz (A)", "Fiação"] {
            ui.label(pequeno(h, t.text_dim));
        }
        ui.end_row();
        for l in &r.linhas {
            let e = &l.escolha;
            let c = &l.circuito;
            let mut dica = l.automatico.avisos.clone();
            dica.extend(e.avisos.iter().cloned());
            if ui.link(c.id.as_str()).on_hover_text("Selecionar os pontos deste circuito").clicked() {
                *selecionar = Some(c.pontos.clone());
            }
            ui.label(c.natureza.nome());
            ui.label(br(c.potencia));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, f) in l.fases.iter().enumerate() {
                    if i > 0 {
                        ui.label("+");
                    }
                    ui.label(etiqueta_cor(ele::quadro::NOMES_FASES[*f], s.cores_fase[*f])).on_hover_text(format!(
                        "Fase {}: {}",
                        ele::quadro::NOMES_FASES[*f],
                        s.cores_fase[*f].nome()
                    ));
                }
            });
            ui.label(br((e.ib * 10.0).round() / 10.0));
            ui.label(br(l.fct));
            ui.label(br(l.fca)).on_hover_text(format!("{} circuito(s) no trecho de eletroduto mais cheio (tabela 42)", l.agrupados));
            let comprimento = l.comprimento.map_or("-".into(), |v| br((v * 10.0).round() / 10.0));
            if l.fora_da_rede > 0 {
                ui.label(RichText::new(format!("{comprimento}*")).color(COR_AVISO))
                    .on_hover_text(format!("{} ponto(s) fora da rede de eletrodutos: comprimento estimado em L", l.fora_da_rede));
            } else {
                ui.label(comprimento);
            }
            let queda = e.queda.map_or("-".into(), |q| format!("{} / {}", br((q * 100.0).round() / 100.0), br(l.queda_admissivel)));
            if e.queda.is_some_and(|q| q > l.queda_admissivel) {
                ui.label(RichText::new(queda).color(COR_AVISO));
            } else {
                ui.label(queda);
            }
            // Seção e disjuntor: a pré-seleção automática, que o projetista pode trocar.
            let mut sob = l.sobrescrita;
            let antes = sob;
            escolher(ui, ("ele-secao", &c.id), &mut sob.secao, l.automatico.secao, &SECOES);
            escolher(ui, ("ele-disj", &c.id), &mut sob.disjuntor, l.automatico.disjuntor, &DISJUNTORES);
            if sob != antes {
                pedidos.push((r.quadro, c.id.clone(), sob));
            }
            let iz = e.iz.map_or("-".into(), |v| br((v * 10.0).round() / 10.0));
            if dica.is_empty() {
                ui.label(iz);
            } else {
                ui.label(RichText::new(format!("{iz} !")).color(COR_AVISO)).on_hover_text(dica.join("\n"));
            }
            ui.label(l.indicacao.as_str());
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.horizontal_top(|ui| {
        egui::Grid::new("ele-resumo-totais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
            ui.label("Potência instalada");
            ui.label(format!("{} VA", br(r.potencia_total)));
            ui.end_row();
            for (f, nome) in ele::quadro::NOMES_FASES.iter().enumerate().take(usize::from(r.fases_quadro)) {
                ui.label(etiqueta_cor(&format!("Fase {nome} ({})", s.cores_fase[f].nome()), s.cores_fase[f]));
                ui.label(format!("{} VA", br(r.carga_por_fase[f].round())));
                ui.end_row();
            }
            if r.fases_quadro > 1 {
                ui.label("Desequilíbrio");
                ui.label(format!("{} %", br((r.desequilibrio * 10.0).round() / 10.0)));
                ui.end_row();
            }
        });
        ui.add_space(24.0);
        egui::Grid::new("ele-resumo-materiais").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
            ui.label(pequeno(format!("Cabos (fator {})", br(s.fator_materiais)), t.text_dim));
            ui.end_row();
            for (secao, m) in &r.materiais {
                ui.label(format!("{} mm²", br(*secao)));
                ui.label(format!("{} m", br(m.ceil())));
                ui.end_row();
            }
        });
    });
    let mut avisos = r.avisos.clone();
    for l in &r.linhas {
        avisos.extend(l.escolha.avisos.iter().map(|w| format!("Circuito {}: {w}", l.circuito.id)));
        avisos.extend(l.circuito.avisos.iter().map(|w| format!("Circuito {}: {w}", l.circuito.id)));
    }
    lista_avisos(ui, "ele-avisos-resumo", &avisos);
    ui.label(pequeno(
        "Escolhas feitas à mão ficam em destaque e são gravadas no quadro (desfazer com Ctrl+Z). \"Auto\" volta à pré-seleção.",
        t.text_dim,
    ));
}

/// Uma caixa de escolha com "Auto" (a pré-seleção) e os valores da tabela.
fn escolher(ui: &mut egui::Ui, id: (&str, &String), valor: &mut Option<f64>, automatico: Option<f64>, opcoes: &[f64]) {
    let auto = automatico.map_or("-".to_string(), br);
    let texto = match valor {
        Some(v) => RichText::new(br(*v)).strong().color(COR_AVISO),
        None => RichText::new(auto.clone()),
    };
    egui::ComboBox::from_id_salt(id).selected_text(texto).width(70.0).show_ui(ui, |ui| {
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
        .default_pos(egui::pos2(480.0, 140.0))
        .default_width(460.0)
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
        "Tensões e fases valem quando o bloco não informa TENSAO ou o quadro não informa NUMERO_FASES. Os eletrodutos correm no pé-direito; cada ponto desce até a sua ALTURA.",
        t.text_dim,
    ));
}

fn aba_condutores(ui: &mut egui::Ui, t: &Tokens, s: &mut Setup) {
    egui::Grid::new("ele-setup-condutores").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label("Material / isolação");
        ui.label("Cobre, PVC 70 °C");
        ui.end_row();
        ui.label("Método de instalação");
        ui.label("B1: condutores isolados em eletroduto");
        ui.end_row();
        ui.label("Temperatura do projeto");
        ui.add(campo(&mut s.temperatura_ambiente, 10.0..=60.0, 1.0, 0, " °C"));
        ui.end_row();
        ui.label("FCT (tabela 40)");
        ui.label(br(s.fct()));
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
            let atual = s.cores_fase[f];
            egui::ComboBox::from_id_salt(("ele-cor", f)).selected_text(etiqueta_cor(atual.nome(), atual)).width(120.0).show_ui(ui, |ui| {
                for c in CorCabo::TODAS {
                    ui.selectable_value(&mut s.cores_fase[f], c, etiqueta_cor(c.nome(), c));
                }
            });
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
    ui.label(pequeno("Neutro azul-claro e proteção verde-amarelo são fixos pela NBR 5410.", t.text_dim));
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
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        egui::Grid::new("ele-telecom").num_columns(5).striped(true).spacing(vec2(12.0, 3.0)).show(ui, |ui| {
            for h in ["Cabo", "Tipo", "Qtd.", "Observação", ""] {
                ui.label(pequeno(h, t.text_dim));
            }
            ui.end_row();
            for c in &r.setup.cabos_telecom {
                ui.label(c.nome.as_str());
                ui.label(c.categoria.nome());
                ui.label(format!("{} {}", c.quantidade, c.categoria.unidade()));
                ui.label(c.observacao.as_str());
                if c.padrao {
                    ui.label(pequeno("padrão", t.text_dim));
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
                }
                Err(e) => r.erro = Some(e),
            }
        }
        if let Some(e) = &r.erro {
            ui.label(pequeno(e.as_str(), COR_AVISO));
        }
    });
    ui.label(pequeno("O catálogo é gravado com as configurações ao clicar em OK.", t.text_dim));
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
        ui.label(pequeno("Nenhum circuito: use Criar circuito ou o atributo CIRCUITO.", t.text_dim));
        return;
    }
    let tag_quadro = |h: Option<Handle>| h.and_then(|h| a.ponto(h)).map_or("?".to_string(), |q| q.rotulo());
    egui::Grid::new("ele-circuitos").num_columns(6).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
        for h in ["Quadro", "Circ.", "Tipo", "Pontos", "VA", "V"] {
            ui.label(pequeno(h, t.text_dim));
        }
        ui.end_row();
        for c in &a.circuitos {
            ui.label(tag_quadro(c.quadro));
            if ui.link(c.id.as_str()).on_hover_text("Selecionar os pontos deste circuito").clicked() {
                *selecionar = Some(c.pontos.clone());
            }
            ui.label(c.natureza.nome());
            ui.label(c.pontos.len().to_string());
            ui.label(br(c.potencia));
            ui.label(c.tensao.map_or_else(|| "?".to_string(), br));
            ui.end_row();
        }
    });
    let avisos: Vec<String> = a.circuitos.iter().flat_map(|c| c.avisos.iter().map(move |w| format!("Circuito {}: {w}", c.id))).collect();
    lista_avisos(ui, "ele-avisos-circuitos", &avisos);
}
