//! CRAFT ELE: o painel Elétrica. Mostra o que `cadcraft_ele` lê do desenho aberto (pontos e
//! circuitos) e, a pedido, o quadro de cargas dimensionado pela NBR 5410.
//!
//! Os textos ficam em português: o CRAFT ELE é um addon para projetos brasileiros e não passa
//! pelo catálogo de traduções do CADCraft. Só o nome do painel e o item de menu passam.

use std::sync::Arc;

use cadcraft_doc::{Drawing, Handle};
use cadcraft_ele::{self as ele, TipoPonto};
use egui::{Color32, RichText, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::docking::Panel;
use crate::theme::Tokens;

const COR_AVISO: Color32 = Color32::from_rgb(232, 176, 72);

#[derive(Clone, Default)]
struct Estado {
    /// (uid, revisão) do desenho analisado: muda a cada edição.
    chave: Option<(u64, u64)>,
    analise: Arc<ele::Analise>,
    cfg: ele::Config,
    /// O quadro foi pedido: é refeito a cada mudança do desenho.
    dimensionar: bool,
    quadro: Option<Arc<ele::QuadroDeCargas>>,
}

impl Estado {
    fn atualizar(&mut self, app: &CadApp) {
        let Ok(st) = app.session.state() else {
            self.chave = None;
            self.analise = Arc::default();
            self.quadro = None;
            return;
        };
        let chave = (st.uid, st.revision);
        if self.chave != Some(chave) {
            self.chave = Some(chave);
            self.analise = Arc::new(ele::analisar(&st.doc));
            self.recalcular();
        }
    }

    fn recalcular(&mut self) {
        self.quadro = self.dimensionar.then(|| Arc::new(ele::dimensionar_quadro(&self.analise, &self.cfg)));
    }
}

fn id() -> egui::Id {
    egui::Id::new("craft-ele-painel")
}

/// Abre o painel (comando ELETRICA, menu Janela).
pub fn open(app: &mut CadApp) -> Result<Value, String> {
    crate::docking::command(app, false, &json!({ "operation": "open", "panel": Panel::Eletrica.id() }))
}

/// Comandos do CRAFT ELE. `None` quando `id` não é um deles.
///
/// `eletrica.analisar` e `eletrica.dimensionar` respondem em JSON, para o canal de controle, o
/// MCP e scripts. `eletrica.dimensionar` aceita `{tensaoFn, fct, fca}`.
pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    match id {
        "ui.panel.eletrica" | "eletrica" | "craftele" => Some(open(app)),
        "eletrica.analisar" => Some(com_desenho(app, |doc| serde_json::to_value(ele::analisar(doc)).map_err(|e| e.to_string()))),
        "eletrica.dimensionar" => {
            let cfg = config(params);
            Some(com_desenho(app, |doc| serde_json::to_value(ele::dimensionar_quadro(&ele::analisar(doc), &cfg)).map_err(|e| e.to_string())))
        }
        _ => None,
    }
}

fn com_desenho(app: &CadApp, f: impl FnOnce(&Drawing) -> Result<Value, String>) -> Result<Value, String> {
    let st = app.session.state().map_err(|e| e.to_string())?;
    f(&st.doc)
}

fn config(params: &Value) -> ele::Config {
    let mut c = ele::Config::default();
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

/// Número com vírgula decimal.
fn br(v: f64, casas: usize) -> String {
    format!("{v:.casas$}").replace('.', ",")
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let mut st: Estado = ui.ctx().data_mut(|d| d.get_temp(id())).unwrap_or_default();
    st.atualizar(app);
    let aberto = app.session.state().is_ok();
    let mut selecionar: Option<Vec<Handle>> = None;
    // Rola nos dois sentidos: o quadro de cargas é mais largo que o painel estreito.
    egui::ScrollArea::both().id_salt("dock-eletrica").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.label(RichText::new("CRAFT ELE · NBR 5410").strong().color(t.accent));
            if !aberto {
                ui.label(RichText::new("Nenhum desenho aberto.").color(t.text_dim));
                return;
            }
            secao_pontos(ui, &t, &st.analise);
            secao_circuitos(ui, &t, &st.analise, &mut selecionar);
            ui.separator();
            secao_acoes(ui, &t, &mut st);
            if let Some(q) = st.quadro.clone() {
                secao_quadro(ui, &t, &q, &mut selecionar);
            }
        });
    });
    if let Some(h) = selecionar {
        app.session.set_selection(h);
    }
    ui.ctx().data_mut(|d| d.insert_temp(id(), st));
}

fn titulo(ui: &mut egui::Ui, t: &Tokens, texto: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(texto).strong().color(t.text));
}

fn pequeno(texto: impl Into<String>, cor: Color32) -> RichText {
    RichText::new(texto).font(crate::theme::small()).color(cor)
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

fn secao_acoes(ui: &mut egui::Ui, t: &Tokens, st: &mut Estado) {
    ui.horizontal_wrapped(|ui| {
        let sem_circuitos = st.analise.circuitos.is_empty();
        let botao = ui.add_enabled(!sem_circuitos, egui::Button::new("Dimensionar quadro"));
        if botao.on_disabled_hover_text("Nenhum circuito no desenho").clicked() {
            st.dimensionar = true;
            st.recalcular();
        }
        ui.add_enabled(false, egui::Button::new("Ajustar fiação"))
            .on_disabled_hover_text("Próxima etapa: precisa do traçado dos eletrodutos ligado aos pontos.");
    });
    egui::CollapsingHeader::new("Parâmetros").id_salt("ele-parametros").show(ui, |ui| {
        let mut mudou = false;
        egui::Grid::new("ele-cfg").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
            ui.label("Tensão F-N padrão (V)");
            mudou |= ui.add(egui::DragValue::new(&mut st.cfg.tensao_fn).range(100.0..=400.0).speed(1.0)).changed();
            ui.end_row();
            ui.label("FCT (temperatura)");
            mudou |= ui.add(egui::DragValue::new(&mut st.cfg.fct).range(0.1..=1.5).speed(0.01).fixed_decimals(2)).changed();
            ui.end_row();
            ui.label("FCA (agrupamento)");
            mudou |= ui.add(egui::DragValue::new(&mut st.cfg.fca).range(0.1..=1.0).speed(0.01).fixed_decimals(2)).changed();
            ui.end_row();
        });
        ui.label(pequeno("Cobre, PVC 70 °C, método B1 (tabela 36). Queda de tensão ainda não é calculada.", t.text_dim));
        if mudou {
            st.recalcular();
        }
    });
}

fn secao_quadro(ui: &mut egui::Ui, t: &Tokens, q: &ele::QuadroDeCargas, selecionar: &mut Option<Vec<Handle>>) {
    let nome = q.quadro.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
    titulo(ui, t, &format!("Quadro de cargas{nome}"));
    egui::Grid::new("ele-quadro").num_columns(7).striped(true).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
        // Iz por último: é a coluna que menos importa quando o painel está estreito.
        for h in ["Circ.", "VA", "Fases", "Ib (A)", "mm²", "Disj. A", "Iz (A)"] {
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
    ui.add_space(2.0);
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
