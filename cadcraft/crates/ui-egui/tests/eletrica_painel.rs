// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Free Elétrica: a paleta Elétrica na ordem do fluxo, a barra de contexto sobre o desenho, o painel
//! de baixo com as abas, a inspeção visual e as configurações.

use std::sync::Arc;

use cadcraft_engine::doc::geom::{Vec2, Vec3};
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
/// e eletrodutos ligando os blocos em sequência, do quadro até 6.
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
    // Um bloco de tomada com atributos na definição (como os do template), ainda não inserido.
    let mut tom = Block::new("TOM-X");
    for (tag, valor, constante) in [("TIPO_PONTO", "TUG", true), ("POTENCIA", "100VA", false), ("CIRCUITO", "", false)] {
        let mut a = atributo(tag, valor);
        a.constant = constante;
        tom.entities.push(cadcraft_engine::doc::Entity::new(d.new_handle(), EntityKind::AttDef(a)));
    }
    d.blocks.insert("TOM-X".into(), Arc::new(tom));
    // Grafo estrito: um eletroduto entre cada par de blocos vizinhos.
    for (a, b) in [(0.0, 2.0), (2.0, 4.0), (4.0, 5.0), (5.0, 6.0)] {
        let c = Common { layer: "ELETRODUTO".into(), ..Default::default() };
        d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::new(a, 0.0, 0.0), b: Vec3::new(b, 0.0, 0.0) })).unwrap();
    }
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
    Harness::builder().with_size(vec2(2400.0, 1000.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    )
}

/// Um teste só: as janelas do Free Elétrica guardam estado global, e dois testes em paralelo
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
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedFirst(500.0)));
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
    // A paleta tem as ferramentas na ordem do fluxo.
    for f in [
        "1. Pontos",
        "Inserir pontos",
        "2. Circuitos e comandos",
        "Criar circuito",
        "Comandos e luminárias",
        "3. Quadro de cargas",
        "Quadro de cargas",
        "4. Eletrodutos",
        "Inserir eletroduto",
        "5. Documentação e tags",
        "Tags de fiação",
        "Tabela do quadro",
        "Atualizar tags e tabelas",
        "Inspecionar projeto",
        "Configurações...",
    ] {
        h.get_by_label(f);
    }
    // Nada de planilha flutuando: sem o painel, sem as tabelas.
    assert!(h.query_by_label("Rotear fiação...").is_none());
    assert!(h.query_by_label("Pontos elétricos").is_none());
    assert!(!h.state().ui.docking.layout.contains(&Panel::QuadroCargas));

    // Configurações: isolação e diâmetros dos condutores, eletrodutos, e o catálogo de telecom
    // com o diâmetro do cabo novo.
    h.get_by_label("Configurações...").click();
    h.run_steps(3);
    h.get_by_label("Sistema de distribuição");
    h.get_by_label("Condutores").click();
    h.run_steps(3);
    h.get_by_label("Isolação padrão");
    h.get_by_label("Diâmetro externo dos condutores");
    h.get_by_label("Cor da fase R");
    h.get_by_label("Eletrodutos").click();
    h.run_steps(3);
    h.get_by_label("Diâmetro mínimo");
    h.get_by_label("Demanda").click();
    h.run_steps(3);
    h.get_by_label("Concessionária");
    h.get_by_label("Tipo de edificação");
    h.get_by_label("Telecom").click();
    h.run_steps(3);
    h.get_by_label("Fibra óptica N12");
    h.get_by_label("Cadastrar novo cabo");
    h.get_by_label("Diâmetro externo");
    h.get_by_label("Cancelar").click();
    h.run_steps(3);
    assert!(h.query_by_label("Cadastrar novo cabo").is_none());

    // Quadro de cargas: abre no painel de baixo do desenho, acoplado, com altura fixa.
    h.get_by_label("Quadro de cargas").click();
    h.run_steps(3);
    let local = h.state().ui.docking.layout.location(&Panel::QuadroCargas).unwrap();
    assert_eq!(local.anchor, Some(Panel::Canvas));
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedSecond(340.0)));
    h.get_by_label("PAINEL: QD-01");
    for coluna in [
        "Esquema",
        "Tipo de carga",
        "Potência (VA)",
        "FP",
        "Potência (W)",
        "Ib (A)",
        "I'b (A)",
        "Queda (%)",
        "Isolação",
        "Fases",
        "Fase R (VA)",
        "Situação",
    ] {
        assert!(h.query_all_by_label(coluna).count() >= 1, "coluna {coluna}");
    }
    h.get_by_label("Copiar resumo");
    // Os botões da linha do circuito.
    assert!(h.query_all_by_label("+ Pontos").count() >= 2);
    assert!(h.query_all_by_label("- Ponto").count() >= 2);
    assert!(h.query_all_by_label("Localizar").count() >= 2);
    // Novo circuito cria o próximo número (4) vazio, listado na tabela.
    h.get_by_label("Novo circuito").click();
    h.run_steps(3);
    let cargas = h.state_mut().run("eletrica.cargas", json!({})).unwrap();
    assert!(cargas.as_array().unwrap().iter().any(|c| c["id"] == "4" && c["circuito"].is_null()), "{cargas}");
    h.get_by_label("vazio");
    h.state_mut().session.undo().unwrap();
    h.run_steps(3);
    // ELEROTEAR abre o mesmo painel.
    assert!(h.state_mut().cmdline("ELEROTEAR").is_none());
    h.run_steps(3);
    assert_eq!(h.query_all_by_label("PAINEL: QD-01").count(), 1);

    // Cadeado: travar a seção do primeiro circuito leva ao valor em uso, e a seção automática
    // fica como estava para quem não travou.
    h.query_all_by_label("Automático").next().unwrap().click();
    h.run_steps(3);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"][0]["sobrescrita"]["secao"].is_number(), "{r}");
    h.state_mut().session.undo().unwrap();
    h.run_steps(3);
    // Localizar enquadra o circuito e o destaca, sem travar.
    let antes = h.state().session.state().unwrap().view().center;
    h.query_all_by_label("Localizar").next().unwrap().click();
    h.run_steps(3);
    let depois = h.state().session.state().unwrap().view().center;
    assert_ne!(antes, depois);

    // Recolher e expandir mudam a altura; destacar solta o painel e acoplar devolve.
    h.get_by_label("Recolher").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.docking.layout.location(&Panel::QuadroCargas).unwrap().size, Some(craft_ui::layout::SplitSize::FixedSecond(96.0)));
    h.get_by_label("Expandir").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.docking.layout.location(&Panel::QuadroCargas).unwrap().size, Some(craft_ui::layout::SplitSize::FixedSecond(340.0)));
    h.get_by_label("Destacar").click();
    h.run_steps(3);
    assert!(h.state().ui.docking.layout.floating.iter().any(|g| g.panels.contains(&Panel::QuadroCargas)));
    h.get_by_label("Acoplar").click();
    h.run_steps(3);
    assert!(!h.state().ui.docking.layout.floating.iter().any(|g| g.panels.contains(&Panel::QuadroCargas)));

    // Aba Eletrodutos: os trechos, com diâmetro travável.
    h.get_by_label("Eletrodutos").click();
    h.run_steps(3);
    h.get_by_label("4 trecho(s) ligados, 0 solto(s)");
    h.get_by_label("Ocupação");

    // Aba Pontos: filtrar por tipo, escolher o bloco, inserir com os valores e excluir.
    assert!(h.state_mut().run("ui.eletrica.lista", Value::Null).is_ok());
    h.run_steps(3);
    h.get_by_label("Blocos");
    h.get_by_label("TUG (1)").click();
    h.run_steps(3);
    h.get_by_label("Pontos no desenho (1)");
    h.get_by_label("TOM-X  ·  TUG  ·  0 no desenho").click();
    h.run_steps(3);
    h.get_by_label("POTENCIA");
    h.get_by_label("Inserir no desenho").click();
    h.run_steps(3);
    let app = h.state_mut();
    assert!(app.session.running.is_some(), "ELEPONTO começou");
    app.session.input(Input::Point(Vec2::new(9.0, 9.0))).unwrap();
    app.session.input(Input::Enter).unwrap();
    assert!(app.session.running.is_none());
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    let novo = a["pontos"].as_array().unwrap().iter().find(|p| p["bloco"] == "TOM-X").expect("ponto inserido").clone();
    assert_eq!((novo["potencia"].as_f64(), novo["posicao"][0].as_f64()), (Some(100.0), Some(9.0)));
    h.run_steps(3);
    h.get_by_label("Pontos no desenho (2)");
    // Excluir pede confirmação na própria linha.
    h.query_all_by_label("Excluir").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Excluir?").click();
    h.run_steps(3);
    h.get_by_label("Pontos no desenho (1)");

    // Barra de contexto: só enquanto o comando espera cliques, e muda o modelo sem interrompê-lo.
    assert!(h.query_by_label("Alinhar à parede").is_none());
    h.state_mut().run("eletrica.bootstrap", json!({})).unwrap();
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "TOM-SIM-BAIXA" })).unwrap();
    h.get_by_label("Inserir pontos").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"));
    h.get_by_label("Alinhar à parede").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"));
    let uid = h.state().session.state().unwrap().uid;
    assert!(!free_eletrica::biblioteca::modelo(uid).unwrap().rotacao_auto);
    h.get_by_label("Altura");
    h.get_by_label("Potência");
    h.get_by_label("Girar").click();
    h.run_steps(3);
    assert!(h.state().session.running.is_some());
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Alinhar à parede").is_none());

    h.get_by_label("Inserir eletroduto").click();
    h.run_steps(3);
    h.get_by_label("Cota");
    h.get_by_label("Piso").click();
    h.run_steps(3);
    assert_eq!(free_eletrica::eletroduto::modelo(uid).instalacao, free_eletrica::rota::Instalacao::Piso);
    assert!(h.state().session.running.is_some());
    h.get_by_label("Teto").click();
    h.run_steps(3);
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(3);

    h.get_by_label("Criar circuito").click();
    h.run_steps(3);
    h.get_by_label("Selecione o quadro e informe o número do circuito.");
    let q = Handle::parse_hex(&qd).unwrap();
    let app = h.state_mut();
    app.session.input(Input::Pick(vec![q])).unwrap();
    app.session.input(Input::Text("9".into())).unwrap();
    h.run_steps(3);
    h.get_by_label("Circuito 9 · QD-01");
    h.state_mut().run("eletrica.circuito.definir", json!({ "circuito": "9", "esquema": "F+N" })).unwrap();
    h.run_steps(3);
    h.get_by_label("Tensão");
    assert!(h.state().session.running.is_some(), "a edição não interrompe o comando");
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(3);

    // Inspeção visual: liga e desliga; ELECONFERIR liga e abre a lista.
    h.get_by_label("Inspecionar projeto").click();
    h.run_steps(3);
    h.get_by_label("Inspeção");
    h.get_by_label("Inspecionar projeto").click();
    h.run_steps(3);
    assert!(h.query_by_label("Inspeção").is_none());
    assert!(h.state_mut().cmdline("ELECONFERIR").is_none());
    h.run_steps(3);
    assert!(h.query_all_by_label("Inspeção").count() >= 2, "a legenda e a aba Conferência");
    h.get_by_label("Pontos elétricos");

    // Tab durante ELEPONTO gira; fora dele, segue o caminho do CADCraft.
    assert!(!cadcraft_ui_egui::eletrica::tab(h.state_mut()));
    assert!(h.state_mut().cmdline("ELEPONTOS").is_none());
    h.run_steps(2);
    assert!(cadcraft_ui_egui::eletrica::tab(h.state_mut()));
    // Imagem da barra com a inspeção ligada, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_BARRA") {
        h.run_steps(2);
        h.render().unwrap().save(saida).unwrap();
    }
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(2);
    assert!(h.state_mut().run("ui.eletrica.inspecao", Value::Null).is_ok());
    assert!(h.state_mut().run("ui.eletrica.cargas", Value::Null).is_ok());
    h.run_steps(3);
    // Imagem para conferir a tela, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG") {
        h.render().unwrap().save(saida).unwrap();
    }
}
