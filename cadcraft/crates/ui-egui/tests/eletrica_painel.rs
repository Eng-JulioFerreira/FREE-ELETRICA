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
    assert_eq!(h.state().ui.toolset_tab, "Drafting");
    let app = h.state_mut();
    // Digitado, abre a aba Installations das Tool Sets, à esquerda: nenhuma coluna a mais tirando espaço do
    // desenho, e as Tool Sets alargam para as três abas.
    assert!(app.cmdline("ELETRICA").is_none());
    assert_eq!(app.ui.toolset_tab, "Installations");
    assert!(!app.ui.docking.layout.contains(&Panel::Eletrica));
    let local = app.ui.docking.layout.location(&Panel::ToolSets).unwrap();
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedFirst(290.0)));

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
        "Elétrica",
        "Telecom",
        "Pontos e Famílias",
        "Blocos e famílias...",
        "Inspetor de atributos...",
        "Dimensionar projeto...",
        "Inserir pontos",
        "Circuitos e Comandos",
        "Criar circuito",
        "Comandos e luminárias",
        "Quadros e Dimensionamento",
        "Quadro de cargas...",
        "Lista de pontos...",
        "Lista de eletrodutos...",
        "Lista de materiais...",
        "Eletrodutos",
        "Inserir eletroduto",
        "Documentação e Tabelas",
        "Projeto e Inspeção",
        "Tags de fiação",
        "Tabela do quadro",
        "Atualizar tags e tabelas",
        "Inspecionar projeto",
        "Configurações...",
    ] {
        h.get_by_label(f);
    }
    // Imagem da paleta, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_PALETA") {
        h.render().unwrap().save(saida).unwrap();
    }
    // Uma seção recolhe e abre pela seta, e o estado fica junto das Tool Sets.
    h.get_by_label("Eletrodutos").click();
    h.run_steps(3);
    assert!(h.query_by_label("Inserir eletroduto").is_none());
    assert!(h.state().ui.collapsed_groups.iter().any(|g| g == "eletrica/Eletrodutos"));
    h.get_by_label("Eletrodutos").click();
    h.run_steps(3);
    h.get_by_label("Inserir eletroduto");
    // A aba Telecom troca as ferramentas.
    h.get_by_label("Telecom").click();
    h.run_steps(3);
    assert!(h.query_by_label("Criar circuito").is_none());
    for f in ["Pontos e Racks", "Inserir ponto de dados", "Inserir rack", "Lista de pontos de dados...", "Infraestrutura", "Catálogo de cabos..."] {
        h.get_by_label(f);
    }
    h.get_by_label("Inserir ponto de dados").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"));
    assert_eq!(free_eletrica::biblioteca::modelo(h.state().session.state().unwrap().uid).unwrap().bloco, "RJ45");
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(3);
    h.get_by_label("Elétrica").click();
    h.run_steps(3);
    // Nada fica aberto sem ser pedido.
    assert!(h.query_by_label("Rotear fiação...").is_none());
    assert!(h.query_by_label("Pontos elétricos").is_none());
    assert!(h.query_by_label("PAINEL: QD-01").is_none());

    // Configurações: isolação e diâmetros dos condutores, eletrodutos, e o catálogo de telecom
    // com o diâmetro do cabo novo.
    h.get_by_label("Configurações...").click();
    h.run_steps(3);
    // O sistema, o cabo padrão e a folga não ficam mais nas configurações.
    for fora in ["Sistema de distribuição", "Tipo de cabo padrão", "Folga de cabos", "Folga de Cabos (%)"] {
        assert!(h.query_by_label(fora).is_none(), "{fora}");
    }
    h.get_by_label("Queda de tensão: iluminação");
    h.get_by_label("Alturas").click();
    h.run_steps(3);
    h.get_by_label("Pé-direito (teto)");
    h.get_by_label("Altura média padrão");
    h.get_by_label("Geral").click();
    h.run_steps(3);
    h.get_by_label("Inicializar projeto no DXF atual");
    h.get_by_label("Condutores").click();
    h.run_steps(3);
    h.get_by_label("Diâmetro externo dos condutores");
    h.get_by_label("Cor da fase R");
    h.query_all_by_label("Eletrodutos").last().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Diâmetro mínimo");
    // Os perfis: linha, cor, caminhamento e a tabela de diâmetros de cada um.
    for campo in [
        "+ Novo perfil",
        "Eletroduto PVC Flexível - Teto",
        "Eletroduto PEAD - Piso",
        "Tipo de Linha",
        "Cor da linha",
        "Caminhamento 3D",
        "Tabela de Diâmetros Comerciais",
        "Excluir perfil",
        "+ Diâmetro",
    ] {
        assert!(h.query_all_by_label(campo).count() >= 1, "{campo}");
    }
    h.get_by_label("+ Novo perfil").click();
    h.run_steps(3);
    h.get_by_label("Perfil 5");
    h.get_by_label("Demanda / Concessionárias").click();
    h.run_steps(3);
    h.get_by_label("Concessionária");
    h.get_by_label("Tipo de edificação");
    // A tabela própria nasce no rascunho e só vale com o OK das configurações.
    h.get_by_label("+ Criar / Editar Tabela de Concessionária").click();
    h.run_steps(3);
    for campo in ["Nome da tabela", "Norma e revisão", "Copiar de", "+ Faixa", "+ Edificação", "Salvar tabela"] {
        assert!(h.query_all_by_label(campo).count() >= 1, "{campo}");
    }
    h.get_by_label("Voltar sem salvar").click();
    h.run_steps(3);
    h.get_by_label("Concessionária");
    h.get_by_label("Cabos").click();
    h.run_steps(3);
    h.get_by_label("Fibra óptica N12");
    h.get_by_label("Cadastrar novo cabo");
    h.get_by_label("Diâmetro externo");
    // Os dois catálogos, com editar e excluir em cada cabo, inclusive os de fábrica.
    h.get_by_label("Cabos elétricos");
    h.get_by_label("Cadastrar novo cabo elétrico");
    assert!(h.query_all_by_label("Excluir").count() >= 10);
    assert!(h.query_all_by_label("Editar").count() >= 10);
    h.query_all_by_label("Editar").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Editar cabo elétrico");
    h.get_by_label("Salvar cabo elétrico");
    h.get_by_label("Cancelar edição").click();
    h.run_steps(3);
    h.get_by_label("Cadastrar novo cabo elétrico");
    h.get_by_label("Cancelar").click();
    h.run_steps(3);
    assert!(h.query_by_label("Cadastrar novo cabo").is_none());

    // Quadro de cargas: só quando pedido, acoplado embaixo do desenho, entre as Tool Sets e as Camadas.
    assert!(!h.state().ui.docking.layout.contains(&Panel::QuadroCargas));
    h.get_by_label("Quadro de cargas...").click();
    h.run_steps(3);
    let local = h.state().ui.docking.layout.location(&Panel::QuadroCargas).unwrap();
    assert_eq!(local.anchor, Some(Panel::Canvas));
    assert_eq!(local.size, Some(craft_ui::layout::SplitSize::FixedSecond(330.0)));
    h.get_by_label("PAINEL: QD-01");
    // Os dados do quadro ficam num grupo recolhível, e a tabela é o foco; sem abas Resumo e Ficha.
    assert!(h.query_by_label("Sistema do Quadro").is_none());
    assert!(h.query_by_label("Resumo").is_none() && h.query_by_label("Ficha de engenharia").is_none());
    h.get_by_label("Configurações e dados do quadro").click();
    h.run_steps(3);
    for campo in
        ["Sistema do Quadro", "Tensão Nominal", "Tipo de Cabo Padrão do Quadro", "Alimentado por", "Montagem", "Módulos", "Reservas", "Observações"]
    {
        h.get_by_label(campo);
    }
    for coluna in ["Descrição", "Esquema", "Pontos", "Potência (VA)", "Ib (A)", "Fase R (VA)", "Fases", "Grupo DR", "Cabo / disjuntor", "Situação"]
    {
        assert!(h.query_all_by_label(coluna).count() >= 1, "coluna {coluna}");
    }
    for fora in ["Queda (%)", "I'b (A)", "FP", "Isolação", "Tipo de cabo"] {
        assert!(h.query_by_label(fora).is_none(), "{fora} fica na ficha do circuito");
    }
    h.get_by_label("Equilibrar fases");
    h.get_by_label("Copiar resumo");
    // Lançar não dimensiona: nada calculado ainda.
    assert!(h.query_all_by_label("A dimensionar").count() >= 2);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"].as_array().unwrap().iter().all(|l| l["calculado"].is_null()), "{r}");
    // As ações de cada circuito ficam no menu "..." da linha, e não em botões soltos.
    for fora in ["Detalhes", "+ Pontos", "- Ponto", "Localizar"] {
        assert!(h.query_by_label(fora).is_none(), "{fora}");
    }
    assert!(h.query_all_by_label("...").count() >= 2);
    h.query_all_by_label("...").next().unwrap().click();
    h.run_steps(3);
    for item in [
        "Adicionar pontos na planta",
        "Remover ponto da planta",
        "Localizar / Destacar pontos",
        "Mais detalhes e ficha de engenharia...",
        "Excluir circuito",
    ] {
        h.get_by_label(item);
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // Novo circuito cria o próximo número (4) vazio, listado na tabela.
    h.get_by_label("Novo circuito").click();
    h.run_steps(3);
    let cargas = h.state_mut().run("eletrica.cargas", json!({})).unwrap();
    assert!(cargas.as_array().unwrap().iter().any(|c| c["id"] == "4" && c["circuito"].is_null()), "{cargas}");
    h.get_by_label("vazio");
    h.state_mut().session.undo().unwrap();
    h.run_steps(3);
    // O X do canto fecha, como Esc, e o painel sai do layout; ELEROTEAR abre o mesmo quadro.
    h.get_by_label("Fechar").click();
    h.run_steps(3);
    assert!(h.query_by_label("PAINEL: QD-01").is_none());
    assert!(!h.state().ui.docking.layout.contains(&Panel::QuadroCargas));
    assert!(h.state_mut().cmdline("ELEROTEAR").is_none());
    h.run_steps(3);
    assert_eq!(h.query_all_by_label("PAINEL: QD-01").count(), 1);

    // Localizar enquadra o circuito e o destaca, sem travar.
    let antes = h.state().session.state().unwrap().view().center;
    h.query_all_by_label("...").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Localizar / Destacar pontos").click();
    h.run_steps(3);
    assert_ne!(antes, h.state().session.state().unwrap().view().center);
    // Equilibrar destrava as fases.
    h.state_mut().run("eletrica.circuito.definir", json!({ "circuito": "2", "fases": "S" })).unwrap();
    h.run_steps(3);
    h.get_by_label("Equilibrar fases").click();
    h.run_steps(3);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"].as_array().unwrap().iter().all(|l| l["fasesTravadas"] == false), "{r}");

    // Mais detalhes abre a ficha, na mesma janela: a engenharia do circuito, com as travas.
    h.query_all_by_label("...").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Mais detalhes e ficha de engenharia...").click();
    h.run_steps(3);
    for campo in [
        "Método de instalação",
        "Temperatura e FCT",
        "Agrupamento e FCA",
        "Queda de tensão",
        "Capacidade (Iz)",
        "Seção e disjuntor",
        "Tensão",
        "Esquema",
        "Tipo de cabo",
    ] {
        assert!(h.query_all_by_label(campo).count() >= 1, "{campo}");
    }
    h.get_by_label("Prévia: o quadro, as tags e as tabelas mudam ao aplicar o cálculo.");
    // A lista troca de circuito.
    h.get_by_label("Circuito 3").click();
    h.run_steps(3);
    h.get_by_label("Circuito 1").click();
    h.run_steps(3);
    // Cadeado: travar a seção leva ao valor em uso.
    h.query_all_by_label("Automático").next().unwrap().click();
    h.run_steps(3);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"][0]["sobrescrita"]["secao"].is_number(), "{r}");
    h.state_mut().session.undo().unwrap();
    h.run_steps(3);
    h.get_by_label("← Voltar ao quadro").click();
    h.run_steps(3);
    // Na paleta e no resumo.
    assert!(h.query_all_by_label("Dimensionar projeto...").count() >= 2);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label("PAINEL: QD-01").is_none());
    assert!(!h.state().ui.docking.layout.contains(&Panel::QuadroCargas), "Esc fecha o painel acoplado");

    // Dimensionar projeto: esquema, cabo e DR por circuito; Calcular mostra, e só Aplicar grava.
    assert!(h.state_mut().cmdline("ELEDIMENSIONAR").is_none());
    h.run_steps(3);
    h.get_by_label("Compartilhar condutor de proteção (Terra / PE)").click();
    h.run_steps(3);
    h.get_by_label("Gerar / Atualizar Diagrama Unifilar");
    h.get_by_label("Temperatura ambiente (FCT)");
    h.get_by_label("FCA fixo");
    for coluna in ["Circ.", "Esquema", "Tipo de cabo"] {
        assert!(h.query_all_by_label(coluna).count() >= 1, "{coluna}");
    }
    assert!(h.query_by_label("Cabo (mm²)").is_none(), "os resultados só aparecem ao calcular");
    let r = h.state_mut().run("eletrica.setup", json!({})).unwrap();
    assert_eq!(r["compartilharPe"], false, "as premissas só valem ao aplicar");
    // Grupos de DR à vontade: um grupo novo, com o circuito marcado à esquerda.
    h.get_by_label("Circuitos sem DR");
    h.get_by_label("Circuito 1").click();
    h.run_steps(3);
    h.get_by_label("+ Novo Grupo DR").click();
    h.run_steps(3);
    h.get_by_label("IDR1");
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_DR") {
        h.render().unwrap().save(saida).unwrap();
    }
    h.get_by_label("Mover selecionados").click();
    h.run_steps(3);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert_eq!(r["linhas"][0]["grupoDr"], "IDR1", "{r}");
    h.get_by_label("Tirar do DR");
    // Calcular não grava; aparece o resultado, editável, e Aplicar grava.
    h.get_by_label("Calcular Dimensionamento").click();
    h.run_steps(3);
    for coluna in ["Cabo (mm²)", "Disjuntor (A)", "Curva", "Queda (%)"] {
        h.get_by_label(coluna);
    }
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"].as_array().unwrap().iter().all(|l| l["calculado"].is_null()), "{r}");
    // Imagem da janela, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_DIMENSIONAR") {
        h.render().unwrap().save(saida).unwrap();
    }
    h.get_by_label("Aplicar ao Quadro").click();
    h.run_steps(3);
    let st = h.state_mut().run("eletrica.setup", json!({})).unwrap();
    assert_eq!(st["compartilharPe"], true);
    let r = h.state_mut().run("eletrica.rotear", json!({})).unwrap();
    assert!(r["linhas"].as_array().unwrap().iter().all(|l| l["calculado"]["secao"].is_number()), "{r}");
    assert_eq!(free_eletrica::unifilar::unifilares(h.state().session.doc().unwrap()).len(), 1, "o diagrama nasce ao aplicar");
    h.state_mut().run("eletrica.setup", json!({ "compartilharPe": false })).unwrap();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.get_by_label("Quadro de cargas...").click();
    h.run_steps(3);
    assert!(h.query_all(egui_kittest::kittest::by().label_contains("#")).count() >= 2, "o resumo mostra o cálculo gravado");
    assert_eq!(h.query_all_by_label("A dimensionar").count(), 0);
    // Imagem do resumo, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_RESUMO") {
        h.render().unwrap().save(saida).unwrap();
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // Blocos e famílias: o botão de criar pede um nome e cai no editor com a família pronta.
    h.get_by_label("Blocos e famílias...").click();
    h.run_steps(3);
    h.get_by_label("+ Criar novo bloco elétrico");
    assert!(h.query_all_by_label("Editar").count() >= 1);
    assert!(h.query_all_by_label("Inserir").count() >= 1);
    // Imagem da janela de blocos, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_BLOCOS") {
        h.render().unwrap().save(saida).unwrap();
    }
    let campo_nome = || egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput).label("Nome do bloco");
    h.get(campo_nome()).click();
    h.run_steps(2);
    h.get(campo_nome()).type_text("TOMADA-DUPLA");
    h.run_steps(3);
    h.get_by_label("+ Criar novo bloco elétrico").click();
    h.run_steps(3);
    h.get_by_label("Propriedades da Família");
    h.get_by_label("Bloco: TOMADA-DUPLA");
    assert!(h.state().session.doc().unwrap().model.iter().any(|e| matches!(&e.kind, EntityKind::AttDef(a) if a.tag == "CIRCUITO")));
    h.state_mut().run("bclose", json!({ "save": false })).unwrap();
    h.run_steps(3);

    // Inspetor de atributos: sem seleção avisa; com um bloco, edita sem digitar comando.
    h.get_by_label("Inspetor de atributos...").click();
    h.run_steps(3);
    h.get_by_label("Selecione um bloco no desenho para ver os atributos dele.");
    h.state_mut().session.set_selection(vec![Handle::parse_hex(&qd).unwrap()]);
    h.run_steps(3);
    h.get_by_label("TAG");
    h.get_by_label("NUMERO_FASES");
    h.get_by_label("Sistema do Quadro");
    h.get_by_label("Alimentado por");
    // Imagem do inspetor, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_INSPETOR") {
        h.render().unwrap().save(saida).unwrap();
    }
    h.state_mut().session.set_selection(Vec::new());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // Lista de eletrodutos: os trechos, com o tipo.
    h.get_by_label("Lista de eletrodutos...").click();
    h.run_steps(3);
    h.get_by_label("4 trecho(s) ligados, 0 solto(s)");
    h.get_by_label("Ocupação");
    h.get_by_label("Tipo");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // Lista de materiais: cabos por seção e tipo, eletrodutos por tipo e diâmetro.
    h.get_by_label("Lista de materiais...").click();
    h.run_steps(3);
    h.get_by_label("Cabos, com a folga do projeto");
    h.get_by_label("Folga de Cabos (%)");
    assert!(h.query_all_by_label("Eletrodutos").count() >= 2, "a seção da paleta e o título na lista");
    h.get_by_label("Disjuntores");
    assert!(h.query_all_by_label("Cabo flexível PVC 70 °C (450/750 V)").count() >= 1);
    assert!(h.query_all_by_label("PVC rígido (roscável ou soldável)").count() >= 1);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // ELEPONTOS: a mini-barra na hora, sem janela; cada clique insere.
    assert!(h.query_by_label("Quadro pai").is_none());
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "TOM-SIM-BAIXA" })).unwrap();
    assert!(h.state_mut().cmdline("ELEPONTOS").is_none());
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"));
    for campo in ["Categoria", "Bloco", "Potência", "Altura", "Quadro pai"] {
        assert!(h.query_all_by_label(campo).count() >= 1, "{campo}");
    }
    assert!(h.query_by_label("PAINEL: QD-01").is_none());
    // Imagem da mini-barra, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_PONTOS") {
        h.render().unwrap().save(saida).unwrap();
    }
    // A busca filtra a lista de blocos enquanto se digita, pelo nome ou pela legenda; um bloco só já fica escolhido.
    let uid = h.state().session.state().unwrap().uid;
    h.get_by_label("Pesquisar ponto").click();
    h.run_steps(2);
    h.get_by_label("Pesquisar ponto").type_text("arandela");
    h.run_steps(3);
    assert_eq!(free_eletrica::biblioteca::modelo(uid).unwrap().bloco, "ILU-PAR");
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "TOM-SIM-BAIXA" })).unwrap();
    h.run_steps(3);
    let antes = h.state_mut().run("eletrica.analisar", json!({})).unwrap()["pontos"].as_array().unwrap().len();
    // Um clique de mouse de verdade, pelo caminho do desenho: sobre a mini-barra não insere.
    let clicar = |h: &mut Harness<'static, CadApp>, x: f32, y: f32| {
        let pos = egui::pos2(x, y);
        h.event(egui::Event::PointerMoved(pos));
        h.run_steps(2);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            h.run_steps(2);
        }
    };
    let contar = |h: &mut Harness<'static, CadApp>| h.state_mut().run("eletrica.analisar", json!({})).unwrap()["pontos"].as_array().unwrap().len();
    clicar(&mut h, 265.0, 95.0);
    assert_eq!(contar(&mut h), antes, "o clique na mini-barra não pode inserir");
    clicar(&mut h, 1400.0, 500.0);
    assert_eq!(contar(&mut h), antes + 1, "o clique no desenho insere");
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"), "e o comando continua");
    let app = h.state_mut();
    app.session.input(Input::Point(Vec2::new(9.0, 9.0))).unwrap();
    app.session.input(Input::Enter).unwrap();
    assert!(app.session.running.is_none());
    let a = app.run("eletrica.analisar", json!({})).unwrap();
    assert_eq!(a["pontos"].as_array().unwrap().len(), antes + 2);
    assert!(a["pontos"].as_array().unwrap().iter().any(|p| p["posicao"][0].as_f64() == Some(9.0)), "{a}");
    h.run_steps(3);
    assert!(h.query_by_label("Quadro pai").is_none(), "a mini-barra some com o comando");

    // Lista de pontos: filtrar, editar e excluir; inserir é só na mini-barra.
    h.get_by_label("Lista de pontos...").click();
    h.run_steps(3);
    assert!(h.query_by_label("Inserir no desenho").is_none());
    for filtro in ["Todos (", "Iluminação (", "Interruptores (", "Dados/Telecom (", "Quadros/Racks (", "Caixas ("] {
        assert!(h.query_all(egui_kittest::kittest::by().label_contains(filtro)).count() >= 1, "filtro {filtro}");
    }
    h.get_by_label("Tomadas (3)").click();
    h.run_steps(3);
    h.get_by_label("Pontos no desenho (3)");
    // Excluir pede confirmação na própria linha.
    h.query_all_by_label("Excluir").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Excluir?").click();
    h.run_steps(3);
    h.get_by_label("Pontos no desenho (2)");
    // O X também fecha: aqui, Esc.
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // Todo item tem Excluir: eletrodutos, blocos e famílias, circuitos e grupos de DR.
    h.get_by_label("Lista de eletrodutos...").click();
    h.run_steps(3);
    let trechos =
        |h: &mut Harness<'static, CadApp>| h.state_mut().run("eletrica.eletrodutos", json!({})).unwrap()["trechos"].as_array().unwrap().len();
    let antes_trechos = trechos(&mut h);
    h.query_all_by_label("Excluir").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Excluir?").click();
    h.run_steps(3);
    assert_eq!(trechos(&mut h), antes_trechos - 1);
    h.state_mut().session.undo().unwrap();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.state_mut().run("eletrica.bootstrap", json!({})).unwrap();
    h.get_by_label("Blocos e famílias...").click();
    h.run_steps(3);
    h.get_by_label("Caixas").click();
    h.run_steps(3);
    let tem = |h: &mut Harness<'static, CadApp>, n: &str| {
        h.state_mut().run("eletrica.blocos", json!({})).unwrap().as_array().unwrap().iter().any(|b| b["nome"] == n)
    };
    assert!(tem(&mut h, "CX-PASSAGEM") && tem(&mut h, "CX-OCT"));
    // Duas caixas de fábrica, em ordem de nome: a octogonal de teto e a de passagem.
    assert_eq!(h.query_all_by_label("Excluir").count(), 2);
    h.query_all_by_label("Excluir").last().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Excluir?").click();
    h.run_steps(3);
    assert!(!tem(&mut h, "CX-PASSAGEM"));
    h.state_mut().run("eletrica.bootstrap", json!({ "restaurar": true })).unwrap();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);

    // Tags de fiação: a mini-barra escolhe o modo com o comando correndo.
    h.get_by_label("Tags de fiação").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("elefiacao"));
    h.get_by_label("Com linha de chamada").click();
    h.run_steps(3);
    let uid = h.state().session.state().unwrap().uid;
    assert_eq!(free_eletrica::fiacao::modo(uid), free_eletrica::fiacao::ModoTag::Chamada);
    h.get_by_label("Direto no eletroduto").click();
    h.run_steps(3);
    assert_eq!(free_eletrica::fiacao::modo(uid), free_eletrica::fiacao::ModoTag::Direto);
    assert!(h.state().session.running.is_some());
    h.state_mut().session.input(Input::Cancel).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Com linha de chamada").is_none(), "a mini-barra some com o comando");

    // Editor de blocos: a paleta vira as propriedades da família.
    assert!(h.query_by_label("Propriedades da Família").is_none());
    h.state_mut().run("bedit", json!({ "name": "MINHA-TOMADA", "create": true })).unwrap();
    h.run_steps(3);
    h.get_by_label("Propriedades da Família");
    h.get_by_label("Bloco: MINHA-TOMADA");
    assert!(h.query_by_label("Inserir eletroduto").is_none());
    h.state_mut().run("eletrica.familia.definir", json!({ "categoria": "ELECTRICAL_FIXTURE" })).unwrap();
    h.run_steps(3);
    h.get_by_label("Referência de altura padrão");
    h.get_by_label("Caixa física associada");
    h.get_by_label("Classe de Demanda");
    h.get_by_label("Fator de Demanda (0,00 a 1,00)");
    h.get_by_label("Automático, pela tabela da concessionária").click();
    h.run_steps(3);
    let bloco = "MINHA-TOMADA";
    let f = h.state_mut().run("eletrica.familia.ler", json!({ "bloco": bloco })).unwrap();
    assert_eq!(f["fatorDemanda"].as_f64(), Some(1.0));
    h.state_mut().run("eletrica.familia.definir", json!({ "bloco": bloco, "fatorDemanda": null })).unwrap();
    h.run_steps(3);
    // Imagem do painel da família, quando pedida.
    if let Ok(saida) = std::env::var("FREE_ELETRICA_PNG_FAMILIA") {
        h.render().unwrap().save(saida).unwrap();
    }
    h.get_by_label("Inserir geometria base").click();
    h.run_steps(3);
    assert!(h.query_all(egui_kittest::kittest::by().label_contains("hachura")).next().is_none(), "preencher é com a Hachura nativa");
    h.get_by_label("Triângulo de tomada").click();
    h.run_steps(3);
    assert_eq!(h.state().session.doc().unwrap().model.len(), 3);
    h.get_by_label("Gerar atributos padrão").click();
    h.run_steps(3);
    assert!(h.state().session.doc().unwrap().model.iter().any(|e| matches!(&e.kind, EntityKind::AttDef(a) if a.tag == "ALTURA")));
    h.get_by_label("Definir ponto de conexão").click();
    h.run_steps(3);
    let app = h.state_mut();
    assert_eq!(app.session.running.as_ref().map(|r| r.id.as_str()), Some("elefamilia_conector"));
    app.session.input(Input::Point(Vec2::new(0.0, 0.1))).unwrap();
    app.run("bclose", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Propriedades da Família").is_none());
    h.get_by_label("Inserir eletroduto");
    let blocos = h.state_mut().run("eletrica.blocos", json!({})).unwrap();
    let nova = blocos.as_array().unwrap().iter().find(|b| b["nome"] == "MINHA-TOMADA").expect("a família aparece na lista").clone();
    assert_eq!(nova["familia"]["categoria"], "pontoForca");

    // A mini-barra muda o modelo sem interromper o comando.
    h.state_mut().run("eletrica.bootstrap", json!({})).unwrap();
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "TOM-SIM-BAIXA" })).unwrap();
    h.state_mut().start("eleponto");
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("eleponto"));
    let uid = h.state().session.state().unwrap().uid;
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "RJ45" })).unwrap();
    h.run_steps(3);
    assert!(h.state().session.running.is_some());
    assert!(h.query_by_label("Quadro pai").is_none(), "um ponto de dados não tem quadro elétrico");
    h.state_mut().run("eletrica.ponto.modelo", json!({ "bloco": "TOM-SIM-BAIXA" })).unwrap();
    h.state_mut().session.input(Input::Enter).unwrap();
    h.run_steps(3);

    h.get_by_label("Inserir eletroduto").click();
    h.run_steps(3);
    // A mini-barra só tem o perfil e os três traçados.
    h.get_by_label("Perfil");
    for fora in ["Cota", "Traçado", "Flecha", "Ortogonal", "Piso", "Teto", "Parede", "Inverter"] {
        assert!(h.query_by_label(fora).is_none(), "{fora} saiu da mini-barra");
    }
    h.get_by_label("Reto");
    h.get_by_label("Arco");
    h.get_by_label("Ângulo 45°").click();
    h.run_steps(3);
    assert_eq!(free_eletrica::eletroduto::modelo(uid).tracado, free_eletrica::eletroduto::Tracado::Angulo45);
    // O perfil vale sem interromper o comando, e traz o caminhamento.
    h.state_mut().run("eletrica.eletroduto.modelo", json!({ "perfil": "Eletroduto PEAD - Piso" })).unwrap();
    h.run_steps(3);
    assert_eq!(free_eletrica::eletroduto::modelo(uid).instalacao, free_eletrica::rota::Instalacao::Piso);
    assert!(h.state().session.running.is_some());
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
    h.state_mut().start("eleponto");
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
