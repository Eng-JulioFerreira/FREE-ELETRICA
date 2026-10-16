//! CRAFT ELE: o painel Elétrica abre pelo comando ELETRICA, lê os pontos do desenho aberto e
//! dimensiona o quadro pelo botão e pelo comando JSON.

use std::sync::Arc;

use cadcraft_engine::Session;
use cadcraft_engine::doc::geom::Vec3;
use cadcraft_engine::doc::{Attrib, Block, Common, Drawing, EntityKind, HAlign, Insert, Space, Text, VAlign};
use cadcraft_ui_egui::{CadApp, Services, docking::Panel};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

fn atributo(tag: &str, valor: &str) -> Attrib {
    Attrib {
        tag: tag.into(),
        text: Text {
            insert: Vec3::ZERO,
            align_pt: None,
            height: 0.1,
            value: valor.into(),
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Left,
            valign: VAlign::Baseline,
        },
        invisible: false,
        constant: false,
        prompt: String::new(),
        props: Default::default(),
        multiline: None,
        modes: Default::default(),
    }
}

fn inserir(d: &mut Drawing, bloco: &str, x: f64, attrs: &[(&str, &str)]) {
    d.blocks.entry(bloco.to_string()).or_insert_with(|| Arc::new(Block::new(bloco)));
    let ins = Insert {
        block: bloco.into(),
        insert: Vec3::new(x, 0.0, 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
        rotation: 0.0,
        attribs: attrs.iter().map(|(t, v)| atributo(t, v)).collect(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    };
    d.add(&Space::Model, Common::default(), EntityKind::Insert(ins)).unwrap();
}

fn planta() -> Drawing {
    let mut d = Drawing::new_metric();
    inserir(&mut d, "QD", 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01"), ("TENSAO", "220/127V"), ("NUMERO_FASES", "2")]);
    #[rustfmt::skip]
    let luz = [("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a")];
    inserir(&mut d, "CAIXA_LUZ", 2.0, &luz);
    #[rustfmt::skip]
    let tug = [("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "2")];
    inserir(&mut d, "TOMADA", 4.0, &tug);
    d
}

#[test]
fn eletrica_abre_o_painel_e_dimensiona_o_quadro() {
    let mut app = CadApp::new(Session::default(), Services::default());
    app.session.open_drawing(planta(), "Planta", None);
    // Digitado, abre o painel.
    assert!(app.cmdline("ELETRICA").is_none());
    assert!(app.ui.docking.layout.contains(&Panel::Eletrica));
    // Na primeira vez fica acima das Tool Sets, com altura fixa.
    let local = app.ui.docking.layout.location(&Panel::Eletrica).unwrap();
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedFirst(250.0)));
    assert_eq!(local.anchor, Some(Panel::ToolSets));
    // Pelo canal de controle, o resultado sai em JSON.
    let q = app.run("eletrica.dimensionar", json!({})).unwrap();
    assert_eq!(q["quadro"], "QD-01");
    assert_eq!(q["potenciaTotal"], 700.0);
    // 600 VA / 127 V = 4,7 A: 2,5 mm² e disjuntor de 6 A.
    assert_eq!(q["linhas"][1]["dimensionamento"]["secao"], 2.5);
    assert_eq!(q["linhas"][1]["dimensionamento"]["disjuntor"], 6.0);
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    assert_eq!(a["pontos"].as_array().unwrap().len(), 3);
    // As configurações mudam o cálculo: a 60 °C o FCT é 0,50 e o 2,5 mm² cai para 12 A.
    let s = app.run("eletrica.setup", json!({ "temperaturaAmbiente": 60 })).unwrap();
    assert_eq!(s["temperaturaAmbiente"], 60.0);
    let q = app.run("eletrica.dimensionar", json!({})).unwrap();
    assert_eq!(q["linhas"][1]["dimensionamento"]["iz"], 12.0);
    app.run("eletrica.setup", json!({ "temperaturaAmbiente": 30 })).unwrap();
    assert_eq!(app.run("eletrica.setup", json!({})).unwrap()["sistema"], "bifasico220127");

    app.ui.in_window_menu = false;
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    // A paleta só tem ferramentas: nenhum dado do projeto.
    h.get_by_label("Dimensionar quadro...");
    assert!(h.query_by_label("Pontos elétricos").is_none());
    assert!(h.query_by_label("Quadro de cargas (QD-01)").is_none());

    // Configurações: abas e Cancelar.
    h.get_by_label("Configurações...").click();
    h.run_steps(3);
    h.get_by_label("Sistema de distribuição");
    h.get_by_label("Condutores").click();
    h.run_steps(3);
    h.get_by_label("FCT (tabela 40)");
    h.get_by_label("Cancelar").click();
    h.run_steps(3);
    assert!(h.query_by_label("FCT (tabela 40)").is_none());

    // Conferência: pontos e circuitos; clicar no circuito seleciona os pontos dele.
    h.get_by_label("Conferir pontos e circuitos...").click();
    h.run_steps(3);
    h.get_by_label("Pontos elétricos");
    assert_eq!(h.get_all_by_label("Iluminação").count(), 2);
    h.get_all_by_label("2").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.selection().len(), 1);

    // Quadro de cargas.
    h.get_by_label("Dimensionar quadro...").click();
    h.run_steps(3);
    h.get_by_label("Quadro de cargas (QD-01)");
    h.get_by_label("700 VA");
}

#[test]
fn eletrica_nao_abre_sozinha_sem_o_executavel_pedir() {
    // O layout de fábrica do CADCraft continua o mesmo: só o executável pede a paleta aberta.
    let mut app = CadApp::new(Session::default(), Services::default());
    app.session.open_drawing(planta(), "Planta", None);
    let mut h = Harness::builder().with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(3);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Eletrica));
}
