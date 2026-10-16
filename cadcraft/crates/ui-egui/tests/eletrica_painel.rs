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
    // Pelo canal de controle, o resultado sai em JSON.
    let q = app.run("eletrica.dimensionar", json!({})).unwrap();
    assert_eq!(q["quadro"], "QD-01");
    assert_eq!(q["potenciaTotal"], 700.0);
    // 600 VA / 127 V = 4,7 A: 2,5 mm² e disjuntor de 6 A.
    assert_eq!(q["linhas"][1]["dimensionamento"]["secao"], 2.5);
    assert_eq!(q["linhas"][1]["dimensionamento"]["disjuntor"], 6.0);
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    assert_eq!(a["pontos"].as_array().unwrap().len(), 3);

    app.ui.in_window_menu = false;
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h.get_by_label("Pontos elétricos");
    // Na contagem de pontos e no tipo do circuito 1.
    assert_eq!(h.get_all_by_label("Iluminação").count(), 2);
    h.get_by_label("Circuitos");
    assert!(h.query_by_label("Quadro de cargas (QD-01)").is_none());
    h.get_by_label("Dimensionar quadro").click();
    h.run_steps(3);
    h.get_by_label("Quadro de cargas (QD-01)");
    h.get_by_label("700 VA");
    // Clicar no circuito seleciona os pontos dele no desenho.
    h.get_all_by_label("2").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.selection().len(), 1);
}
