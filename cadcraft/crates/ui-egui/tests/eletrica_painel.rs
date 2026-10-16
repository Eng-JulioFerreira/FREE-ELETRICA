//! CRAFT ELE: a paleta Elétrica (só ferramentas), os comandos ELECIRCUITO e ELECOMANDO no
//! desenho, a janela Rotear fiação com a tabela de resumo e a sobrescrita manual, e o setup.

use std::sync::Arc;

use cadcraft_engine::doc::geom::Vec3;
use cadcraft_engine::doc::{Attrib, Block, Common, Drawing, EntityKind, HAlign, Handle, Insert, Line, Space, Text, VAlign};
use cadcraft_engine::{Input, Session};
use cadcraft_ui_egui::{CadApp, Services, docking::Panel};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

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

fn inserir(d: &mut Drawing, bloco: &str, x: f64, attrs: &[(&str, &str)]) -> Handle {
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
    d.add(&Space::Model, Common::default(), EntityKind::Insert(ins)).unwrap()
}

/// Em metros: quadro em 0, luz em 2, TUG em 4, uma segunda luz e um interruptor sem circuito,
/// e um eletroduto do quadro até 6.
fn planta() -> Drawing {
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    inserir(&mut d, "QD", 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01"), ("TENSAO", "220/127V"), ("NUMERO_FASES", "2"), ("ALTURA", "1,5")]);
    #[rustfmt::skip]
    let luz = [("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a")];
    inserir(&mut d, "CAIXA_LUZ", 2.0, &luz);
    #[rustfmt::skip]
    let tug = [("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "2"), ("ALTURA", "0,3")];
    inserir(&mut d, "TOMADA", 4.0, &tug);
    // Sem circuito: entram pelas ferramentas.
    inserir(&mut d, "CAIXA_LUZ", 5.0, &[("TIPO_PONTO", "ILUM"), ("POTENCIA", "160VA"), ("TENSAO", "127V")]);
    inserir(&mut d, "INTERRUPTOR", 6.0, &[("TIPO_PONTO", "INTERRUPTOR"), ("POTENCIA", "0W")]);
    let c = Common { layer: "ELETRODUTO".into(), ..Default::default() };
    d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(6.0, 0.0, 0.0) })).unwrap();
    d
}

fn hex(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

/// (handle do quadro, da luz sem circuito, do interruptor) pela análise JSON.
fn handles(app: &mut CadApp) -> (String, String, String) {
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    let pontos = a["pontos"].as_array().unwrap();
    let achar = |f: &dyn Fn(&Value) -> bool| hex(&pontos.iter().find(|p| f(p)).unwrap()["handle"]);
    (achar(&|p| p["tipo"] == "quadro"), achar(&|p| p["tipo"] == "iluminacao" && p["circuito"].is_null()), achar(&|p| p["tipo"] == "interruptor"))
}

fn harness(app: CadApp) -> Harness<'static, CadApp> {
    Harness::builder().with_size(vec2(1600.0, 1000.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    )
}

/// Um teste só: as janelas do CRAFT ELE guardam estado global, e dois testes em paralelo
/// mexeriam nele ao mesmo tempo.
#[test]
fn paleta_ferramentas_e_fluxo_completo() {
    let mut app = CadApp::new(Session::default(), Services::default());
    app.session.open_drawing(planta(), "Planta", None);
    app.ui.in_window_menu = false;
    // O layout de fábrica do CADCraft continua o mesmo: só o executável pede a paleta aberta.
    let mut h = harness(app);
    h.run_steps(3);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Eletrica));
    let app = h.state_mut();
    // Digitado, abre a paleta acima das Tool Sets, com altura fixa.
    assert!(app.cmdline("ELETRICA").is_none());
    let local = app.ui.docking.layout.location(&Panel::Eletrica).unwrap();
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedFirst(250.0)));
    assert_eq!(local.anchor, Some(Panel::ToolSets));

    let (qd, luz, int) = handles(app);
    // ELECIRCUITO interativo: quadro, Enter (aceita o próximo número, 3) e a luz.
    app.start("elecircuito");
    app.session.input(Input::Pick(vec![Handle::parse_hex(&qd).unwrap()])).unwrap();
    assert!(app.session.prompt_text().contains("Número do circuito"), "{}", app.session.prompt_text());
    app.session.input(Input::Enter).unwrap();
    app.session.input(Input::Pick(vec![Handle::parse_hex(&luz).unwrap()])).unwrap();
    app.session.input(Input::Enter).unwrap();
    assert!(app.session.running.is_none());
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    let c3 = a["circuitos"].as_array().unwrap().iter().find(|c| c["id"] == "3").expect("circuito 3");
    assert_eq!((c3["potencia"].as_f64(), c3["quadro"].as_str()), (Some(160.0), Some(qd.as_str())));
    // Um passo de desfazer.
    app.session.undo().unwrap();
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    assert!(a["circuitos"].as_array().unwrap().iter().all(|c| c["id"] != "3"));
    app.session.redo().unwrap();

    // ELECOMANDO pelo JSON: o interruptor (sem circuito) ganha a letra livre do circuito da luz e
    // entra no circuito dela.
    let r = app.run("elecomando", json!({ "interruptor": int, "luminarias": [luz] })).unwrap();
    assert_eq!((r["comando"].as_str(), r["circuito"].as_str()), (Some("a"), Some("3")));
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    let sw = a["pontos"].as_array().unwrap().iter().find(|p| p["tipo"] == "interruptor").unwrap();
    assert_eq!((sw["comando"].as_str(), sw["circuito"].as_str()), (Some("a"), Some("3")));

    // Rotear fiação pelo JSON: TUG de 600 VA a 127 V (4,7 A) em 2,5 mm² com 6 A.
    let r = app.run("eletrica.rotear", json!({ "quadro": "QD-01" })).unwrap();
    assert_eq!(r["tag"], "QD-01");
    let c2 = r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "2").unwrap();
    assert_eq!((c2["escolha"]["secao"].as_f64(), c2["escolha"]["disjuntor"].as_f64()), (Some(2.5), Some(6.0)));
    // 4 m de eletroduto + 2,5 m de descida + 1,3 m de subida ao quadro.
    assert!((c2["comprimento"].as_f64().unwrap() - 7.8).abs() < 1e-9);
    // Sobrescrita: disjuntor de 10 A no circuito 2, gravada no quadro e desfeita com Ctrl+Z.
    app.run("eletrica.sobrescrever", json!({ "circuito": "2", "disjuntor": 10 })).unwrap();
    let r = app.run("eletrica.rotear", json!({})).unwrap();
    let c2 = r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "2").unwrap();
    assert_eq!((c2["escolha"]["disjuntor"].as_f64(), c2["sobrescrita"]["disjuntor"].as_f64()), (Some(10.0), Some(10.0)));
    assert!(app.run("eletrica.sobrescrever", json!({ "circuito": "2", "secao": 3 })).is_err(), "seção fora da tabela");
    app.session.undo().unwrap();
    let r = app.run("eletrica.rotear", json!({})).unwrap();
    assert_eq!(r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "2").unwrap()["escolha"]["disjuntor"], 6.0);

    // Setup pelo JSON: a 60 °C o FCT é 0,50.
    let s = app.run("eletrica.setup", json!({ "temperaturaAmbiente": 60, "peDireito": 3.0 })).unwrap();
    assert_eq!((s["temperaturaAmbiente"].as_f64(), s["peDireito"].as_f64()), (Some(60.0), Some(3.0)));
    let r = app.run("eletrica.rotear", json!({})).unwrap();
    assert_eq!(r["linhas"][0]["fct"], 0.5);
    app.run("eletrica.setup", json!({ "temperaturaAmbiente": 30, "peDireito": 2.8 })).unwrap();

    h.run_steps(4);
    // A paleta só tem ferramentas.
    for f in ["Configurações...", "Criar circuito", "Comandos e luminárias", "Conferir pontos e circuitos...", "Rotear fiação..."] {
        h.get_by_label(f);
    }
    assert!(h.query_by_label("Pontos elétricos").is_none());

    // Configurações: a aba de telecom com o catálogo e o cadastro.
    h.get_by_label("Configurações...").click();
    h.run_steps(3);
    h.get_by_label("Sistema de distribuição");
    h.get_by_label("Telecom").click();
    h.run_steps(3);
    h.get_by_label("Fibra óptica N12");
    h.get_by_label("Cadastrar novo cabo");
    h.get_by_label("Cancelar").click();
    h.run_steps(3);
    assert!(h.query_by_label("Cadastrar novo cabo").is_none());

    // Rotear fiação: escolher o quadro e rotear mostra a tabela de resumo.
    h.get_by_label("Rotear fiação...").click();
    h.run_steps(3);
    assert!(h.query_by_label("Resumo da fiação: QD-01").is_none());
    h.get_by_label("Rotear").click();
    h.run_steps(3);
    h.get_by_label("Resumo da fiação: QD-01");
    h.get_by_label("ΔV (%)");
    h.get_by_label("Copiar resumo");
}
