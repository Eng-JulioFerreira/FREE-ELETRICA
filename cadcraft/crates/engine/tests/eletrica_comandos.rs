// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Free Elétrica sem interface: todo o motor de cálculo responde por comandos do motor, como na
//! linha de comando, em scripts, no `cadcraft-cli` e no MCP.

use std::sync::Arc;

use cadcraft_engine::doc::geom::{Vec2, Vec3};
use cadcraft_engine::doc::{Attrib, Block, Common, Drawing, EntityKind, HAlign, Handle, Insert, Line, Space, Text, VAlign};
use cadcraft_engine::{Input, Session};
use free_eletrica::TipoPonto;
use serde_json::{Value, json};

/// As configurações são globais no processo: um teste que grava nelas passa por aqui, ou outro em
/// paralelo sobrescreve o que ele acabou de gravar.
static SETUP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn trava_setup() -> std::sync::MutexGuard<'static, ()> {
    SETUP.lock().unwrap_or_else(|e| e.into_inner())
}

fn atributo(tag: &str, valor: &str) -> Attrib {
    Attrib {
        tag: tag.into(),
        text: Text {
            insert: Vec3::ZERO,
            align_pt: None,
            height: 0.05,
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

fn inserir(d: &mut Drawing, bloco: &str, x: f64, y: f64, attrs: &[(&str, &str)]) -> Handle {
    d.blocks.entry(bloco.to_string()).or_insert_with(|| Arc::new(Block::new(bloco)));
    let ins = Insert {
        block: bloco.into(),
        insert: Vec3::new(x, y, 0.0),
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

fn eletroduto(d: &mut Drawing, a: (f64, f64), b: (f64, f64)) {
    let c = Common { layer: "ELE-ELETRODUTO-PVC-MEDIA-TETO".into(), ..Default::default() };
    d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) })).unwrap();
}

/// Quadro em 0, luz em 3 (comando a), interruptor em (3,1), TUG em 6; eletrodutos ligando tudo.
fn planta() -> (Drawing, [Handle; 4]) {
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    let qd = inserir(&mut d, "QDG", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QDG"), ("NUMERO_FASES", "3"), ("ALTURA", "1,5")]);
    let luz = inserir(&mut d, "ILU-OCT", 3.0, 0.0, &[("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("TENSAO", "220V"), ("COMANDO", "")]);
    let int = inserir(&mut d, "INT-SIM", 3.0, 1.0, &[("TIPO_PONTO", "INTERRUPTOR"), ("POTENCIA", "0W"), ("ALTURA", "1,10")]);
    let tug = inserir(&mut d, "TOM-SIM-BAIXA", 6.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("TENSAO", "220V"), ("ALTURA", "0,30")]);
    eletroduto(&mut d, (0.0, 0.0), (3.0, 0.0));
    eletroduto(&mut d, (3.0, 0.0), (6.0, 0.0));
    eletroduto(&mut d, (3.0, 0.0), (3.0, 1.0));
    (d, [qd, luz, int, tug])
}

fn circuito(s: &Session, h: Handle) -> Option<String> {
    let doc = s.doc().ok()?;
    free_eletrica::ponto::ler_ponto(doc, h)?.circuito
}

#[test]
fn projeto_inteiro_por_comandos_sem_interface() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let (d, [qd, luz, int, tug]) = planta();
    s.open_drawing(d, "Planta", None);

    // Criar circuito pelo JSON e pelo prompt interativo.
    let r = s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()] })).unwrap();
    assert_eq!(r["circuito"], "1");
    s.start("elecircuito").unwrap();
    s.input(Input::Pick(vec![qd])).unwrap();
    s.input(Input::Enter).unwrap(); // aceita o próximo: 2
    s.input(Input::Pick(vec![tug])).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(circuito(&s, tug).as_deref(), Some("2"));

    // Comandos e luminárias: o interruptor sem circuito entra no da luz.
    let r = s.execute("elecomando", &json!({ "interruptor": int.hex(), "luminarias": [luz.hex()] })).unwrap();
    assert_eq!((r["comando"].as_str(), r["circuito"].as_str()), (Some("a"), Some("1")));

    // Análise e roteamento em JSON.
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    assert_eq!(a["circuitos"].as_array().unwrap().len(), 2);
    let r = s.execute("eletrica.rotear", &json!({ "quadro": "QDG" })).unwrap();
    assert_eq!(r["tag"], "QDG");
    let c2: &Value = r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "2").unwrap();
    // 6 m + 2,50 de descida + 1,30 do quadro.
    assert!((c2["comprimento"].as_f64().unwrap() - 9.8).abs() < 1e-9);

    // Configurações pelo motor, com ou sem desenho.
    let st = s.execute("eletrica.setup", &json!({})).unwrap();
    assert_eq!(st["sistema"], "trifasico380220");

    // Sobrescrita e desfazer.
    s.execute("eletrica.sobrescrever", &json!({ "circuito": "2", "disjuntor": 16 })).unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!(r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "2").unwrap()["escolha"]["disjuntor"], 16.0);
    s.undo().unwrap();

    // Tags de fiação: uma por eletroduto; rodar de novo não duplica; limpar apaga.
    let r = s.execute("elefiacao", &json!({})).unwrap();
    assert_eq!(r["criadas"], 3, "{r}");
    let entidades = s.doc().unwrap().model.len();
    let r = s.execute("elefiacao", &json!({ "quadro": "QDG" })).unwrap();
    assert_eq!(r["criadas"], 0);
    assert_eq!(s.doc().unwrap().model.len(), entidades);
    let r = s.execute("elefiacao", &json!({ "limpar": true })).unwrap();
    assert_eq!(r["apagadas"], 3);

    // ELEFIACAO digitado: Enter põe em todos; um passo de desfazer.
    s.start("elefiacao").unwrap();
    s.input(Input::Enter).unwrap();
    assert!(s.running.is_none());
    assert_eq!(s.doc().unwrap().model.len(), entidades);
    s.undo().unwrap();
    assert_eq!(s.doc().unwrap().model.len(), entidades - 3);
}

#[test]
fn comandos_eletricos_sem_quadro_dao_erro_e_nao_travam() {
    let _setup = trava_setup();
    let mut s = Session::new();
    s.open_drawing(Drawing::new_metric(), "Vazio", None);
    assert!(s.execute("eletrica.rotear", &json!({})).is_err());
    assert!(s.execute("elefiacao", &json!({})).is_err());
    assert!(s.execute("eletrica.sobrescrever", &json!({ "circuito": "1" })).is_err());
    assert!(s.execute("eletrica.setup", &json!({ "sistema": 42 })).is_err());
    assert!(s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"].as_array().unwrap().is_empty());
}

#[test]
fn quadro_de_cargas_e_isolacao_por_comandos() {
    let mut s = Session::new();
    let (d, [qd, luz, _int, tug]) = planta();
    s.open_drawing(d, "Planta", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()] })).unwrap();
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [tug.hex()] })).unwrap();

    // Criar um circuito vazio com descrição; o próximo número livre já conta com ele.
    let r = s.execute("eletrica.circuito.criar", &json!({ "descricao": "Chuveiro" })).unwrap();
    assert_eq!(r["circuito"], "3");
    assert!(s.execute("eletrica.circuito.criar", &json!({ "circuito": "3" })).is_err(), "já existe");
    s.execute("eletrica.circuito.descrever", &json!({ "circuito": 2, "descricao": "TUG" })).unwrap();
    let cargas = s.execute("eletrica.cargas", &json!({})).unwrap();
    let ids: Vec<&str> = cargas.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["1", "2", "3"]);
    assert_eq!(cargas[2]["descricao"], "Chuveiro");
    assert!(cargas[2]["circuito"].is_null());

    // Adicionar pontos a um circuito existente: o alvo leva o ELECIRCUITO direto aos pontos.
    s.execute("eletrica.circuito.alvo", &json!({ "circuito": "3" })).unwrap();
    s.start("elecircuito").unwrap();
    s.input(Input::Pick(vec![tug])).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(circuito(&s, tug).as_deref(), Some("3"));

    // Juntar o 3 no 1 e excluir o 1.
    let r = s.execute("eletrica.circuito.juntar", &json!({ "de": "3", "para": "1" })).unwrap();
    assert_eq!(r["pontos"], 1);
    assert_eq!(circuito(&s, tug).as_deref(), Some("1"));
    let r = s.execute("eletrica.circuito.excluir", &json!({ "circuito": "1" })).unwrap();
    assert_eq!(r["pontos"], 2);
    assert_eq!(circuito(&s, tug), None);
    s.undo().unwrap();
    assert_eq!(circuito(&s, tug).as_deref(), Some("1"));

    // Isolação do quadro: aparece no roteamento; inválida dá erro; null volta ao padrão.
    s.execute("eletrica.quadro", &json!({ "isolacao": "XLPE" })).unwrap();
    assert_eq!(s.execute("eletrica.rotear", &json!({})).unwrap()["isolacao"], "xlpe");
    assert!(s.execute("eletrica.quadro", &json!({ "isolacao": "ouro" })).is_err());
    s.execute("eletrica.quadro", &json!({ "isolacao": null })).unwrap();
    assert_eq!(s.execute("eletrica.rotear", &json!({})).unwrap()["isolacao"], "pvc");

    // A fiação dimensiona o eletroduto de cada trecho e grava no XDATA dele.
    let r = s.execute("elefiacao", &json!({})).unwrap();
    assert!(r["criadas"].as_u64().unwrap() > 0);
    let e = s.execute("eletrica.eletrodutos", &json!({})).unwrap();
    assert!(e["trechos"].as_array().unwrap().iter().any(|t| t["eletroduto"]["nominal"] == 20.0), "{e}");
}

/// Um bloco de tomada com atributos na definição, como os do template.
fn bloco_tomada(d: &mut Drawing) {
    let mut b = Block::new("TOM-SIM-BAIXA");
    for (tag, valor, constante) in [("TIPO_PONTO", "TUG", true), ("POTENCIA", "100VA", false), ("CIRCUITO", "", false)] {
        let mut a = atributo(tag, valor);
        a.constant = constante;
        b.entities.push(cadcraft_engine::doc::Entity::new(d.new_handle(), EntityKind::AttDef(a)));
    }
    d.blocks.insert("TOM-SIM-BAIXA".into(), Arc::new(b));
}

#[test]
fn eletroduto_em_arco_por_comandos() {
    let mut s = Session::new();
    let (mut d, [qd, luz, int, tug]) = planta();
    // Sem os eletrodutos retos da planta: só os em arco.
    let retos: Vec<Handle> = d.model.iter().filter(|e| matches!(e.kind, EntityKind::Line(_))).map(|e| e.handle).collect();
    for h in retos {
        d.model.remove(h);
    }
    s.open_drawing(d, "Planta", None);
    // JSON: do quadro à luz, no teto; e da luz à tomada por coordenadas, no piso, reto.
    let r = s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex() })).unwrap();
    let h = Handle::parse_hex(r["eletroduto"].as_str().unwrap()).unwrap();
    assert!(matches!(s.doc().unwrap().model.get(h).unwrap().kind, EntityKind::Arc(_)), "arco é ARC");
    let r = s.execute("eleeletroduto", &json!({ "de": [3.0, 0.0], "para": [6.0, 0.05], "instalacao": "FLOOR", "flecha": 0 })).unwrap();
    assert_eq!((r["de"].as_str(), r["para"].as_str()), (Some(luz.hex().as_str()), Some(tug.hex().as_str())));
    // Pontas que não são blocos elétricos, ou o mesmo bloco, ou parâmetros hostis: erro.
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": qd.hex() })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex() })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": [50.0, 50.0], "para": qd.hex() })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": [50.0], "para": qd.hex() })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": h.hex() })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex(), "instalacao": "LUA" })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex(), "flecha": "muita" })).is_err());
    let camadas: Vec<String> =
        s.doc().unwrap().model.iter().filter(|e| e.common.layer.contains("ELETRODUTO")).map(|e| e.common.layer.clone()).collect();
    assert_eq!(camadas, ["ELE-ELETRODUTO-TETO", "ELE-ELETRODUTO-PISO"]);

    // Interativo: luz, interruptor e quadro em sequência; Inverter muda o lado do próximo arco;
    // um passo de desfazer.
    let antes = s.doc().unwrap().model.len();
    s.start("eleeletroduto").unwrap();
    s.input(Input::Point(Vec2::new(3.0, 0.0))).unwrap();
    s.input(Input::Pick(vec![int])).unwrap();
    s.input(Input::Keyword("Inverter".into())).unwrap();
    s.input(Input::Keyword("Parede".into())).unwrap();
    s.input(Input::Point(Vec2::new(0.0, 0.0))).unwrap();
    // Longe de tudo: avisa e continua.
    s.input(Input::Point(Vec2::new(90.0, 90.0))).unwrap();
    assert!(s.running.is_some());
    s.input(Input::Enter).unwrap();
    assert!(s.running.is_none());
    assert_eq!(s.doc().unwrap().model.len(), antes + 2);
    s.undo().unwrap();
    assert_eq!(s.doc().unwrap().model.len(), antes);
    // O modelo muda pelo comando, fica para o próximo e recusa valores ruins.
    let m = s.execute("eletrica.eletroduto.modelo", &json!({ "instalacao": "CEILING", "flecha": 9 })).unwrap();
    assert_eq!((m["instalacao"].as_str(), m["flecha"].as_f64()), (Some("teto"), Some(0.5)));
    assert!(s.execute("eletrica.eletroduto.modelo", &json!({ "instalacao": 3 })).is_err());
    s.execute("eletrica.eletroduto.modelo", &json!({ "flecha": 0 })).unwrap();
    let r = s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": tug.hex() })).unwrap();
    let h = Handle::parse_hex(r["eletroduto"].as_str().unwrap()).unwrap();
    assert!(matches!(s.doc().unwrap().model.get(h).unwrap().kind, EntityKind::Line(_)));
}

#[test]
fn clicar_no_meio_do_eletroduto_divide_com_caixa() {
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    let q = inserir(&mut d, "QDG", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QDG"), ("ALTURA", "1,5")]);
    let t = inserir(&mut d, "T", 6.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    let ramal = inserir(&mut d, "T", 3.0, 3.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    s.open_drawing(d, "Planta", None);
    s.execute("eletrica.ponto.excluir", &json!({ "pontos": [] })).unwrap();
    let r = s.execute("eleeletroduto", &json!({ "de": q.hex(), "para": t.hex(), "flecha": 0 })).unwrap();
    let e = r["eletroduto"].as_str().unwrap().to_string();
    let antes = s.doc().unwrap().model.len();
    // Interativo: do ramal ao meio do eletroduto. Vira uma caixa e dois trechos.
    s.start("eleeletroduto").unwrap();
    s.input(Input::Keyword("Flecha".into())).unwrap();
    s.input(Input::Text("0".into())).unwrap();
    s.input(Input::Point(Vec2::new(3.0, 3.0))).unwrap();
    s.input(Input::Point(Vec2::new(3.0, 0.05))).unwrap();
    s.input(Input::Enter).unwrap();
    // A caixa, a segunda metade do trecho dividido e o ramal.
    assert_eq!(s.doc().unwrap().model.len(), antes + 3);
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    assert_eq!(a["pontos"].as_array().unwrap().iter().filter(|p| p["tipo"] == "caixa").count(), 1);
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!(r["linhas"][0]["foraDaRede"], 0, "{r}");
    let _ = ramal;
    s.undo().unwrap();
    assert_eq!(s.doc().unwrap().model.len(), antes);
    // Pelo JSON; o ponto na ponta e um eletroduto que não existe dão erro.
    assert!(s.execute("eletrica.eletroduto.dividir", &json!({ "eletroduto": e, "ponto": [0.0, 0.0] })).is_err());
    assert!(s.execute("eletrica.eletroduto.dividir", &json!({ "eletroduto": "FFFF", "ponto": [1.0, 0.0] })).is_err());
    assert!(s.execute("eletrica.eletroduto.dividir", &json!({ "eletroduto": e })).is_err());
    let r = s.execute("eletrica.eletroduto.dividir", &json!({ "eletroduto": e, "ponto": [2.0, 0.0] })).unwrap();
    assert!(r["caixa"].is_string());
    // Diâmetro travado e instalação de um trecho.
    assert!(s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e })).is_err());
    assert!(s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e, "diametro": -3 })).is_err());
    assert!(s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": q.hex(), "diametro": 25 })).is_err(), "o quadro não é um eletroduto");
    s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e, "diametro": 32, "instalacao": "WALL" })).unwrap();
    let ee = s.execute("eletrica.eletrodutos", &json!({})).unwrap();
    let tr = ee["trechos"].as_array().unwrap().iter().find(|x| x["handle"].as_str() == Some(e.as_str())).unwrap().clone();
    assert_eq!(
        (tr["eletroduto"]["nominal"].as_f64(), tr["diametroTravado"].as_bool(), tr["instalacao"].as_str()),
        (Some(32.0), Some(true), Some("parede"))
    );
    s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e, "diametro": null })).unwrap();
    let ee = s.execute("eletrica.eletrodutos", &json!({})).unwrap();
    let tr = ee["trechos"].as_array().unwrap().iter().find(|x| x["handle"].as_str() == Some(e.as_str())).unwrap().clone();
    assert_eq!(tr["diametroTravado"], false);
}

#[test]
fn desenho_virgem_ganha_camadas_e_blocos_sem_perguntar() {
    let mut s = Session::new();
    s.open_drawing(Drawing::new_metric(), "Virgem", None);
    // Consultar não muda o desenho.
    assert!(s.execute("eletrica.blocos", &json!({})).unwrap().as_array().unwrap().is_empty());
    // Inserir já traz tudo, sem template.
    let r = s.execute("eleponto", &json!({ "bloco": "QDG", "pontos": [[0.0, 0.0]], "valores": { "TAG": "QD-1" } })).unwrap();
    assert_eq!(r["pontos"].as_array().unwrap().len(), 1);
    let doc = s.doc().unwrap();
    for c in ["ELE-PONTOS", "ELE-QUADROS", "ELE-FIACAO", "ELE-ELETRODUTO-TETO", "ELE-ELETRODUTO-PISO"] {
        assert!(doc.layer(c).is_some(), "{c}");
    }
    assert_eq!(s.execute("eletrica.bootstrap", &json!({})).unwrap()["mudou"], false);
    // Interativo: começar o comando prepara outro desenho virgem e o primeiro clique já insere.
    s.open_drawing(Drawing::new_metric(), "Virgem 2", None);
    s.start("eleponto").unwrap();
    assert!(s.prompt_text().contains("Ponto de inserção"), "{}", s.prompt_text());
    s.input(Input::Point(Vec2::new(0.5, 0.5))).unwrap();
    assert_eq!(s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"][0]["bloco"], "TOM-SIM-BAIXA");
    // Trocar de bloco digitando.
    s.input(Input::Keyword("Bloco".into())).unwrap();
    s.input(Input::Text("NADA".into())).unwrap();
    assert!(s.running.is_some());
    s.input(Input::Keyword("Bloco".into())).unwrap();
    s.input(Input::Text("qdc".into())).unwrap();
    s.input(Input::Point(Vec2::new(1.0, 1.0))).unwrap();
    s.input(Input::Enter).unwrap();
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    assert!(a["pontos"].as_array().unwrap().iter().any(|p| p["bloco"] == "QDC"), "{a}");
    // E o comando manual também funciona num desenho virgem.
    s.open_drawing(Drawing::new_metric(), "Virgem 3", None);
    assert_eq!(s.execute("eletrica.bootstrap", &json!({})).unwrap()["mudou"], true);
}

#[test]
fn barra_muda_o_modelo_enquanto_o_comando_corre_e_tab_gira() {
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    // Uma parede vertical em x = 2.
    let c = Common { layer: "PAREDE".into(), ..Default::default() };
    d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::new(2.0, 0.0, 0.0), b: Vec3::new(2.0, 9.0, 0.0) })).unwrap();
    s.open_drawing(d, "Planta", None);
    let rotacao = |s: &Session, i: usize| -> f64 {
        let insercoes: Vec<f64> = s
            .doc()
            .unwrap()
            .model
            .iter()
            .filter_map(|e| if let EntityKind::Insert(k) = &e.kind { Some(k.rotation.to_degrees()) } else { None })
            .collect();
        insercoes[i]
    };
    s.start("eleponto").unwrap();
    s.execute("eletrica.ponto.modelo", &json!({ "bloco": "TOM-SIM-BAIXA", "valores": { "ALTURA": "1,20", "POTENCIA": "300VA" } })).unwrap();
    assert!(s.running.is_some(), "o comando continua");
    // Junto da parede alinha; longe fica na rotação do modelo.
    s.input(Input::Point(Vec2::new(2.05, 1.0))).unwrap();
    assert!((rotacao(&s, 0) - 90.0).abs() < 1e-9);
    s.input(Input::Point(Vec2::new(6.0, 1.0))).unwrap();
    assert!(rotacao(&s, 1).abs() < 1e-9);
    // Girar soma 90°; com a barra, o próximo muda de bloco no meio do comando.
    s.input(Input::Keyword("Girar".into())).unwrap();
    s.execute("eletrica.ponto.modelo", &json!({ "bloco": "TOM-TUE" })).unwrap();
    s.input(Input::Point(Vec2::new(7.0, 1.0))).unwrap();
    assert!((rotacao(&s, 2) - 90.0).abs() < 1e-9);
    // Rotacao digitada fixa o ângulo e desliga o automático.
    s.input(Input::Keyword("Rotacao".into())).unwrap();
    s.input(Input::Text("45".into())).unwrap();
    s.input(Input::Point(Vec2::new(2.05, 3.0))).unwrap();
    assert!((rotacao(&s, 3) - 45.0).abs() < 1e-9);
    s.input(Input::Enter).unwrap();
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    let pontos = a["pontos"].as_array().unwrap();
    assert_eq!(
        (pontos[0]["bloco"].as_str(), pontos[0]["altura"].as_f64(), pontos[0]["potencia"].as_f64()),
        (Some("TOM-SIM-BAIXA"), Some(1.2), Some(300.0))
    );
    assert_eq!(pontos[2]["bloco"], "TOM-TUE");
    // JSON com alinhamento e parâmetros hostis.
    let r = s.execute("eleponto", &json!({ "bloco": "TOM-TUE", "pontos": [[2.05, 5.0]], "rotacaoAuto": true })).unwrap();
    assert_eq!(r["pontos"].as_array().unwrap().len(), 1);
    assert!((rotacao(&s, 4) - 90.0).abs() < 1e-9);
    assert!(s.execute("eleponto", &json!({ "bloco": "TOM-TUE", "pontos": [[f64::INFINITY, 5.0]]})).is_err());
    assert!(s.execute("eleponto", &json!({ "bloco": "TOM-TUE", "pontos": 3 })).is_err());
}

#[test]
fn cabo_eletroduto_quadro_pai_e_materiais_por_comandos() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Planta", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let um = |v: Value| v["pontos"][0].as_str().unwrap().to_string();
    let q1 = um(s.execute("eleponto", &json!({ "bloco": "QDG", "pontos": [[0.0, 0.0]], "valores": { "TAG": "QD-1" } })).unwrap());
    let q2 = um(s.execute("eleponto", &json!({ "bloco": "QDC", "pontos": [[0.0, 5.0]], "valores": { "TAG": "QD-2" } })).unwrap());

    // Quadro pai: por handle ou TAG; o que não é quadro é recusado.
    let a = um(s
        .execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[4.0, 0.0]], "quadro": "QD-1", "valores": { "CIRCUITO": "1" } }))
        .unwrap());
    assert!(s.execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[5.0, 0.0]], "quadro": "NADA" })).is_err());
    assert!(s.execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[5.0, 0.0]], "quadro": 3 })).is_err());
    let pontos = s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"].clone();
    let p = pontos.as_array().unwrap().iter().find(|p| p["handle"].as_str() == Some(a.as_str())).unwrap().clone();
    assert_eq!(p["quadro"].as_str(), Some(q1.as_str()));
    // No interativo, pelo modelo, mudando no meio do comando.
    s.start("eleponto").unwrap();
    s.execute("eletrica.ponto.modelo", &json!({ "bloco": "TOM-SIM-BAIXA", "quadro": q2, "valores": { "CIRCUITO": "9" } })).unwrap();
    s.input(Input::Point(Vec2::new(4.0, 5.0))).unwrap();
    s.input(Input::Enter).unwrap();
    let pontos = s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"].clone();
    assert!(pontos.as_array().unwrap().iter().any(|p| p["circuito"] == "9" && p["quadro"].as_str() == Some(q2.as_str())), "{pontos}");
    assert!(s.execute("eletrica.ponto.modelo", &json!({ "bloco": "TOM-SIM-BAIXA", "quadro": "NADA" })).is_err());
    s.execute("eletrica.ponto.modelo", &json!({ "bloco": "TOM-SIM-BAIXA", "quadro": null })).unwrap();

    // Cabo por circuito: inválido não grava; muda a linha; null volta ao padrão.
    let linha = |s: &mut Session, q: &str, id: &str| -> Value {
        let r = s.execute("eletrica.rotear", &json!({ "quadro": q })).unwrap();
        r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == id).unwrap().clone()
    };
    assert!(s.execute("eletrica.circuito.definir", &json!({ "quadro": q1, "circuito": "1", "cabo": "OURO" })).is_err());
    assert!(s.execute("eletrica.circuito.definir", &json!({ "quadro": q1, "circuito": "1", "cabo": 3 })).is_err());
    assert_eq!(linha(&mut s, &q1, "1")["tipoCabo"], "flexivelPvc70");
    s.execute("eletrica.circuito.definir", &json!({ "quadro": q1, "circuito": "1", "cabo": "HEPR_XLPE_90" })).unwrap();
    let l = linha(&mut s, &q1, "1");
    assert_eq!((l["tipoCabo"].as_str(), l["isolacao"].as_str()), (Some("heprXlpe90"), Some("epr")));
    s.undo().unwrap();
    assert_eq!(linha(&mut s, &q1, "1")["tipoCabo"], "flexivelPvc70");
    s.execute("eletrica.circuito.definir", &json!({ "quadro": q1, "circuito": "1", "cabo": "LSZH_HEPR_90" })).unwrap();

    // Eletroduto com tipo: pelo JSON, pelo modelo e pelo prompt; inválido recusa.
    assert!(s.execute("eleeletroduto", &json!({ "de": q1, "para": a, "tipo": "BAMBU" })).is_err());
    let e = s.execute("eleeletroduto", &json!({ "de": q1, "para": a, "tipo": "STEEL_HOT_DIP", "flecha": 0 })).unwrap();
    let e = e["eletroduto"].as_str().unwrap().to_string();
    let tr = s.execute("eletrica.eletrodutos", &json!({})).unwrap()["trechos"][0].clone();
    assert_eq!(tr["tipo"], "acoFogo");
    assert!(s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e, "tipo": "BAMBU" })).is_err());
    s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": e, "tipo": "PVC_CORR_LIGHT" })).unwrap();
    assert_eq!(s.execute("eletrica.eletrodutos", &json!({})).unwrap()["trechos"][0]["tipo"], "corrugadoLeve");
    assert!(s.execute("eletrica.eletroduto.modelo", &json!({ "tipo": "BAMBU" })).is_err());
    let m = s.execute("eletrica.eletroduto.modelo", &json!({ "tipo": "HDPE_CORR" })).unwrap();
    assert_eq!(m["tipo"], "peadCorrugado");
    s.start("eleeletroduto").unwrap();
    s.input(Input::Keyword("Tipo".into())).unwrap();
    s.input(Input::Text("bambu".into())).unwrap();
    s.input(Input::Keyword("Tipo".into())).unwrap();
    s.input(Input::Text("ag-e".into())).unwrap();
    assert!(s.prompt_text().contains("AG-E"), "{}", s.prompt_text());
    s.input(Input::Enter).unwrap();

    // Materiais, dimensionar e equilibrar.
    let m = s.execute("eletrica.materiais", &json!({})).unwrap();
    assert!(m["cabos"].as_array().unwrap().iter().any(|c| c["tipo"] == "lszhHepr90"), "{m}");
    assert!(m["eletrodutos"].as_array().unwrap().iter().any(|c| c["tipo"] == "corrugadoLeve"), "{m}");
    assert!(m["pontos"].as_array().unwrap().iter().any(|c| c["nome"] == "TUG"), "{m}");
    let r = s.execute("eletrica.dimensionar", &json!({ "quadro": "QD-1" })).unwrap();
    assert_eq!(r["circuitos"], 1);
    assert_eq!(r["quadros"].as_array().unwrap().len(), 1);
    assert!(s.execute("eletrica.dimensionar", &json!({ "quadro": "NADA" })).is_err());
    s.execute("eletrica.circuito.definir", &json!({ "quadro": q1, "circuito": "1", "fases": "T" })).unwrap();
    assert_eq!(linha(&mut s, &q1, "1")["fasesTravadas"], true);
    assert_eq!(s.execute("eletrica.quadro.equilibrar", &json!({ "quadro": q1 })).unwrap()["destravados"], 1);
    assert_eq!(linha(&mut s, &q1, "1")["fasesTravadas"], false);

    // Tudo volta do DXF.
    let dxf = cadcraft_io::write_dxf(s.doc().unwrap());
    let volta = cadcraft_io::read_dxf(dxf.as_bytes()).unwrap();
    let a2 = free_eletrica::analisar(&volta);
    let setup = free_eletrica::Setup::default();
    let r = free_eletrica::rotear_quadro(&volta, &a2, &setup, Handle::parse_hex(&q1).unwrap()).unwrap();
    assert_eq!(r.linhas[0].tipo_cabo, free_eletrica::nbr5410::TipoCabo::LszhHepr90);
    let rede = free_eletrica::rota::Rede::montar(&volta, &setup, &a2.pontos, 1.0);
    assert_eq!(rede.arestas[0].tipo, free_eletrica::rota::TipoEletroduto::CorrugadoLeve);
}

#[test]
fn tracados_dimensionamento_e_atributos_por_comandos() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Planta", None);
    assert!(s.execute("eletrica.dimensionar", &json!({})).is_err(), "sem quadro não há o que dimensionar");
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let um = |v: Value| v["pontos"][0].as_str().unwrap().to_string();
    let q = um(s.execute("eleponto", &json!({ "bloco": "QDG", "pontos": [[0.0, 0.0]] })).unwrap());
    let a = um(s.execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[4.0, 3.0]], "valores": { "CIRCUITO": "1" } })).unwrap());
    let b = um(s.execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[8.0, 3.0]], "valores": { "CIRCUITO": "2" } })).unwrap());
    let tipo = |s: &Session, h: &str| s.doc().unwrap().model.get(Handle::parse_hex(h).unwrap()).unwrap().kind.clone();

    // Traçados pelo JSON.
    let e = |s: &mut Session, p: Value| s.execute("eleeletroduto", &p).unwrap()["eletroduto"].as_str().unwrap().to_string();
    let h = e(&mut s, json!({ "de": q, "para": a, "tracado": "STRAIGHT" }));
    assert!(matches!(tipo(&s, &h), EntityKind::Line(_)));
    let h = e(&mut s, json!({ "de": q, "para": a, "tracado": "ANGLE45", "diagonalPrimeiro": true }));
    assert!(matches!(tipo(&s, &h), EntityKind::LwPolyline(_)));
    let h = e(&mut s, json!({ "de": q, "para": a, "tracado": "ARC", "desvios": [[2.0, 3.0]] }));
    assert!(!matches!(tipo(&s, &h), EntityKind::Spline(_)), "nunca SPLINE");
    for ruim in [json!({ "tracado": "ZIGUE" }), json!({ "tracado": 3 }), json!({ "desvios": 3 }), json!({ "desvios": [[1.0]] })] {
        let mut p = json!({ "de": q, "para": a });
        for (k, v) in ruim.as_object().unwrap() {
            p[k] = v.clone();
        }
        assert!(s.execute("eleeletroduto", &p).is_err(), "{p}");
    }
    assert!(s.execute("eletrica.eletroduto.modelo", &json!({ "tracado": "ZIGUE" })).is_err());
    assert_eq!(s.execute("eletrica.eletroduto.modelo", &json!({ "tracado": "ARC" })).unwrap()["tracado"], "arco");

    // Interativo: o traçado pela palavra-chave e um desvio no clique no vazio. O trecho sai pela
    // polilinha do reto com um desvio e os dois blocos nas pontas.
    let antes = s.doc().unwrap().model.len();
    s.start("eleeletroduto").unwrap();
    s.input(Input::Keyword("Reto".into())).unwrap();
    s.input(Input::Point(Vec2::new(4.0, 3.0))).unwrap();
    s.input(Input::Point(Vec2::new(6.0, 6.0))).unwrap();
    assert!(s.doc().unwrap().model.len() == antes, "o desvio não cria nada sozinho");
    s.input(Input::Point(Vec2::new(8.0, 3.0))).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(s.doc().unwrap().model.len(), antes + 1);
    let novo = s.doc().unwrap().model.iter().last().unwrap().clone();
    let EntityKind::LwPolyline(pl) = &novo.kind else { panic!("polilinha") };
    assert_eq!(pl.vertices.len(), 3);
    assert_eq!(s.execute("eletrica.eletrodutos", &json!({})).unwrap()["soltos"].as_array().unwrap().len(), 0);
    // Um desvio sem bloco de chegada não deixa rastro.
    let antes = s.doc().unwrap().model.len();
    s.start("eleeletroduto").unwrap();
    s.input(Input::Point(Vec2::new(90.0, 90.0))).unwrap();
    s.input(Input::Point(Vec2::new(4.0, 3.0))).unwrap();
    s.input(Input::Point(Vec2::new(60.0, 60.0))).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(s.doc().unwrap().model.len(), antes);

    // Editar não dimensiona; processar grava, e o que muda depois fica desatualizado.
    let linha = |s: &mut Session, id: &str| -> Value {
        let r = s.execute("eletrica.rotear", &json!({})).unwrap();
        r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == id).unwrap().clone()
    };
    assert!(linha(&mut s, "1")["calculado"].is_null());
    let r = s.execute("eletrica.dimensionar", &json!({ "temperaturaAmbiente": 35, "compartilharPe": true, "fcaManual": 0.7 })).unwrap();
    assert_eq!(r["circuitos"], 2);
    let st = s.execute("eletrica.setup", &json!({})).unwrap();
    assert_eq!((st["temperaturaAmbiente"].as_f64(), st["compartilharPe"].as_bool(), st["fcaManual"].as_f64()), (Some(35.0), Some(true), Some(0.7)));
    assert_eq!(linha(&mut s, "1")["fca"].as_f64(), Some(0.7));
    assert!(linha(&mut s, "1")["calculado"]["secao"].is_number());
    assert!(s.execute("eletrica.dimensionar", &json!({ "temperaturaAmbiente": "quente" })).is_err());
    assert!(s.execute("eletrica.dimensionar", &json!({ "quadro": "NADA" })).is_err());
    // De volta ao padrão: o processo é compartilhado entre os testes.
    s.execute("eletrica.setup", &json!({ "temperaturaAmbiente": 30, "compartilharPe": false, "fcaManual": null })).unwrap();

    // Grupo de DR.
    s.execute("eletrica.circuito.definir", &json!({ "quadro": q, "circuito": "1", "grupoDr": "IDR1" })).unwrap();
    assert_eq!(linha(&mut s, "1")["grupoDr"], "IDR1");
    assert!(s.execute("eletrica.circuito.definir", &json!({ "quadro": q, "circuito": "1", "grupoDr": 3 })).is_err());
    s.execute("eletrica.circuito.definir", &json!({ "quadro": q, "circuito": "1", "grupoDr": null })).unwrap();
    assert!(linha(&mut s, "1")["grupoDr"].is_null());

    // Inspetor de atributos.
    let l = s.execute("eletrica.atributos", &json!({ "ponto": b })).unwrap();
    assert!(l.as_array().unwrap().iter().any(|x| x["tag"] == "CIRCUITO" && x["valor"] == "2"), "{l}");
    assert!(l.as_array().unwrap().iter().any(|x| x["tag"] == "TIPO_PONTO" && x["constante"] == true), "{l}");
    s.execute("eletrica.atributo.definir", &json!({ "ponto": b, "tag": "CIRCUITO", "valor": 5 })).unwrap();
    assert_eq!(circuito(&s, Handle::parse_hex(&b).unwrap()).as_deref(), Some("5"));
    s.execute("eletrica.atributo.definir", &json!({ "ponto": b, "tag": "POTENCIA", "visivel": true, "posicao": [1.0, 2.0] })).unwrap();
    let l = s.execute("eletrica.atributos", &json!({ "ponto": b })).unwrap();
    let p = l.as_array().unwrap().iter().find(|x| x["tag"] == "POTENCIA").unwrap().clone();
    assert_eq!((p["visivel"].as_bool(), p["x"].as_f64(), p["y"].as_f64()), (Some(true), Some(1.0), Some(2.0)));
    s.execute("eletrica.atributo.definir", &json!({ "ponto": b, "tag": "ALTURA", "valor": "1,45" })).unwrap();
    let l = s.execute("eletrica.atributos", &json!({ "ponto": b })).unwrap();
    assert_eq!(l.as_array().unwrap().iter().find(|x| x["tag"] == "ALTURA").unwrap()["valor"], "h=1,45m");
    for ruim in [
        json!({}),
        json!({ "ponto": "FFFF", "tag": "CIRCUITO", "valor": "1" }),
        json!({ "ponto": b, "valor": "1" }),
        json!({ "ponto": b, "tag": "CIRCUITO" }),
        json!({ "ponto": b, "tag": "NADA", "valor": "1" }),
        json!({ "ponto": b, "tag": "TIPO_PONTO", "valor": "1" }),
        json!({ "ponto": b, "tag": "CIRCUITO", "visivel": "sim" }),
        json!({ "ponto": b, "tag": "CIRCUITO", "posicao": [1] }),
        json!({ "ponto": b, "tag": "CIRCUITO", "valor": [] }),
    ] {
        assert!(s.execute("eletrica.atributo.definir", &ruim).is_err(), "{ruim}");
    }
    assert!(s.execute("eletrica.atributos", &json!({ "ponto": "FFFF" })).is_err());
    let _ = a;
}

#[test]
fn familia_e_altura_por_comandos_e_no_dxf() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Planta", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();

    // Fora do editor, os utilitários recusam e a família pede o bloco.
    assert!(s.execute("eletrica.familia.geometria", &json!({ "forma": "tomada" })).is_err());
    assert!(s.execute("eletrica.familia.atributos", &json!({ "categoria": "DATA_DEVICE" })).is_err());
    assert!(s.execute("elefamilia_conector", &json!({ "ponto": [0, 0] })).is_err());
    assert!(s.execute("eletrica.familia.ler", &json!({})).is_err());
    assert!(s.execute("eletrica.familia.definir", &json!({ "categoria": "DATA_DEVICE" })).is_err());
    assert!(s.execute("eletrica.familia.definir", &json!({ "bloco": "NADA", "categoria": "DATA_DEVICE" })).is_err());

    // Um bloco novo pelo editor: família, geometria, atributos e conector, sem digitar atributo.
    s.execute("bedit", &json!({ "name": "VOZ-DADOS", "create": true })).unwrap();
    assert!(s.execute("eletrica.familia.definir", &json!({})).is_err(), "sem família ainda, pede a categoria");
    let f = s.execute("eletrica.familia.definir", &json!({ "categoria": "DATA_DEVICE" })).unwrap();
    assert_eq!((f["dominio"].as_str(), f["refAltura"].as_str(), f["caixa"].as_str()), (Some("dataTelecom"), Some("baixa"), Some("c4x2")));
    for ruim in [
        json!({ "categoria": "PORTA" }),
        json!({ "dominio": 3 }),
        json!({ "refAltura": "LUA" }),
        json!({ "caixa": "9X9" }),
        json!({ "fp": 7 }),
        json!({ "potencia": -2 }),
        json!({ "teclas": 9 }),
        json!({ "teclas": 999 }),
        json!({ "cotaFixa": "alta" }),
    ] {
        assert!(s.execute("eletrica.familia.definir", &ruim).is_err(), "{ruim}");
    }
    let f = s.execute("eletrica.familia.definir", &json!({ "refAltura": "MEDIUM", "caixa": "4X4", "potencia": 50, "fp": 0.9, "teclas": 2 })).unwrap();
    assert_eq!((f["refAltura"].as_str(), f["potencia"].as_f64(), f["teclas"].as_u64()), (Some("media"), Some(50.0), Some(2)));
    assert_eq!(s.execute("eletrica.familia.ler", &json!({})).unwrap()["caixa"], "c4x4");
    assert!(s.execute("eletrica.familia.geometria", &json!({ "forma": "porta" })).is_err());
    assert!(s.execute("eletrica.familia.geometria", &json!({})).is_err());
    assert_eq!(s.execute("eletrica.familia.geometria", &json!({ "forma": "contorno4x2" })).unwrap()["entidades"], 4);
    assert!(s.execute("eletrica.familia.atributos", &json!({ "categoria": "NONE" })).is_err());
    assert!(s.execute("eletrica.familia.atributos", &json!({})).unwrap()["criados"].as_u64().unwrap() >= 3);
    assert!(s.execute("elefamilia_conector", &json!({})).is_err());
    s.execute("elefamilia_conector", &json!({ "ponto": [0.0, -0.05] })).unwrap();
    s.start("elefamilia_conector").unwrap();
    s.input(Input::Point(Vec2::new(0.0, 0.0))).unwrap();
    assert!(s.running.is_none());
    s.execute("bclose", &json!({ "save": true })).unwrap();

    // O bloco é um ponto de dados; ele respeita a legenda e fica na camada de dados.
    let blocos = s.execute("eletrica.blocos", &json!({})).unwrap();
    let b = blocos.as_array().unwrap().iter().find(|b| b["nome"] == "VOZ-DADOS").expect("o bloco novo é uma família").clone();
    assert_eq!(b["tipo"], "dados");
    let r = s.execute("eleponto", &json!({ "bloco": "VOZ-DADOS", "pontos": [[1.0, 1.0]] })).unwrap();
    let na_legenda = Handle::parse_hex(r["pontos"][0].as_str().unwrap()).unwrap();
    let r = s.execute("eleponto", &json!({ "bloco": "VOZ-DADOS", "pontos": [[2.0, 1.0]], "altura": 1.45 })).unwrap();
    let fora = Handle::parse_hex(r["pontos"][0].as_str().unwrap()).unwrap();
    assert!(s.execute("eleponto", &json!({ "bloco": "VOZ-DADOS", "pontos": [[3.0, 1.0]], "altura": -2 })).is_err());
    assert!(s.execute("eleponto", &json!({ "bloco": "VOZ-DADOS", "pontos": [[3.0, 1.0]], "altura": "alto" })).is_err());
    let attr = |s: &Session, h: Handle| {
        free_eletrica::ponto::ler_ponto(s.doc().unwrap(), h).unwrap().atributos.iter().find(|(t, _)| t == "ALTURA").map(|(_, v)| v.clone()).unwrap()
    };
    assert_eq!((attr(&s, na_legenda), attr(&s, fora)), (String::new(), "h=1,45m".to_string()));
    assert_eq!(s.doc().unwrap().model.get(fora).unwrap().common.layer, "ELE-DADOS");

    // Editar a altura de uma inserção.
    s.execute("eletrica.ponto.altura", &json!({ "pontos": [na_legenda.hex()], "altura": 0.5 })).unwrap();
    assert_eq!(attr(&s, na_legenda), "h=0,50m");
    s.execute("eletrica.ponto.altura", &json!({ "pontos": [na_legenda.hex()], "altura": null })).unwrap();
    assert_eq!(attr(&s, na_legenda), "");
    assert!(s.execute("eletrica.ponto.altura", &json!({ "pontos": [na_legenda.hex()] })).is_err());
    assert!(s.execute("eletrica.ponto.altura", &json!({ "pontos": [na_legenda.hex()], "altura": 99 })).is_err());
    assert!(s.execute("eletrica.ponto.altura", &json!({ "pontos": ["FFFF"], "altura": 1 })).is_err());

    // Tudo sobrevive a gravar e abrir o DXF: família no BLOCK_RECORD, altura no XDATA.
    let dxf = cadcraft_io::write_dxf(s.doc().unwrap());
    let volta = cadcraft_io::read_dxf(dxf.as_bytes()).unwrap();
    let f = free_eletrica::familia::ler(&volta, "VOZ-DADOS").expect("a família volta do DXF");
    assert_eq!(
        (f.categoria, f.caixa, f.potencia, f.teclas),
        (free_eletrica::familia::FamilyCategory::Dados, free_eletrica::familia::CaixaFisica::C4x4, Some(50.0), 2)
    );
    assert!(free_eletrica::familia::ler(&volta, "RJ45").is_some(), "os blocos base também");
    let a = free_eletrica::analisar(&volta);
    assert_eq!(a.pontos.iter().find(|p| p.altura == Some(1.45)).map(|p| p.tipo), Some(TipoPonto::Dados));

    // Remover a família.
    s.execute("eletrica.familia.definir", &json!({ "bloco": "VOZ-DADOS", "remover": true })).unwrap();
    assert!(s.execute("eletrica.familia.ler", &json!({ "bloco": "VOZ-DADOS" })).unwrap().is_null());

    // A legenda pelo Setup, mudando e voltando: o processo é compartilhado entre os testes.
    let st = s.execute("eletrica.setup", &json!({})).unwrap();
    assert_eq!((st["alturaBaixa"].as_f64(), st["alturaMedia"].as_f64(), st["alturaAlta"].as_f64()), (Some(0.3), Some(1.2), Some(2.1)));
    assert_eq!((st["quedaIluminacao"].as_f64(), st["quedaAlimentador"].as_f64()), (Some(4.0), Some(1.0)));
}

#[test]
fn circuito_com_descricao_esquema_e_tensao() {
    let mut s = Session::new();
    let (d, [qd, luz, int, tug]) = planta();
    s.open_drawing(d, "Planta", None);
    // Parâmetros ruins não gravam nada.
    assert!(s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()], "esquema": "4F" })).is_err());
    assert!(s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()], "tensao": -1 })).is_err());
    assert_eq!(circuito(&s, luz), None);
    let r = s
        .execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()], "circuito": "C1", "descricao": "Iluminação Social", "esquema": "F+N", "tensao": 127 }))
        .unwrap();
    assert_eq!(r["circuito"], "C1");
    assert_eq!(circuito(&s, luz).as_deref(), Some("C1"));
    let c = s.execute("eletrica.rotear", &json!({})).unwrap();
    let l = &c["linhas"][0];
    assert_eq!((l["descricao"].as_str(), l["esquema"].as_str(), l["tensao"].as_f64()), (Some("Iluminação Social"), Some("fn"), Some(127.0)));

    // Interativo, com os campos pelo prompt; o ativo acompanha o comando.
    s.start("elecircuito").unwrap();
    s.input(Input::Pick(vec![qd])).unwrap();
    s.input(Input::Text("7".into())).unwrap();
    let uid = s.state().unwrap().uid;
    assert_eq!(free_eletrica::circuito::ativo(uid), Some((qd, "7".to_string())));
    for (k, v) in [("Descricao", "Tomadas"), ("Esquema", "2F+T"), ("Tensao", "220"), ("Esquema", "ouro"), ("Tensao", "nada")] {
        s.input(Input::Keyword(k.into())).unwrap();
        s.input(Input::Text(v.into())).unwrap();
    }
    assert!(s.running.is_some());
    s.input(Input::Pick(vec![tug])).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(free_eletrica::circuito::ativo(uid), None);
    let c = s.execute("eletrica.rotear", &json!({})).unwrap();
    let l = c["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "7").unwrap().clone();
    assert_eq!((l["descricao"].as_str(), l["esquema"].as_str(), l["tensao"].as_f64()), (Some("Tomadas"), Some("fft"), Some(220.0)));

    // Definir: fases travadas; valores ruins não gravam; null volta ao automático.
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "7", "fases": "st" })).unwrap();
    let c = s.execute("eletrica.rotear", &json!({})).unwrap();
    let l = c["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "7").unwrap().clone();
    assert_eq!(l["fasesTravadas"], true);
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "7", "fases": "XYZ" })).is_err());
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "7", "fases": 3 })).is_err());
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "7", "fases": null, "esquema": null, "tensao": null })).unwrap();
    let c = s.execute("eletrica.rotear", &json!({})).unwrap();
    let l = c["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == "7").unwrap().clone();
    assert_eq!(l["fasesTravadas"], false);

    // Remover do circuito: JSON e interativo; quem não é ponto recusa.
    assert!(s.execute("eleremoverponto", &json!({ "pontos": ["FFFF"] })).is_err());
    assert!(s.execute("eleremoverponto", &json!({})).is_err());
    assert_eq!(s.execute("eleremoverponto", &json!({ "pontos": [tug.hex()] })).unwrap()["removidos"], 1);
    assert_eq!(circuito(&s, tug), None);
    s.start("eleremoverponto").unwrap();
    s.input(Input::Pick(vec![luz])).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(circuito(&s, luz), None);
    let _ = int;
}

#[test]
fn comando_usa_as_teclas_do_interruptor_e_grava_o_vinculo() {
    let mut s = Session::new();
    let (mut d, [qd, luz, _int, tug]) = planta();
    let duas =
        inserir(&mut d, "INT-2S", 3.0, 2.0, &[("TIPO_PONTO", "INTERRUPTOR"), ("TIPO_INTERRUPTOR", "2S"), ("POTENCIA", "0W"), ("CIRCUITO", "1")]);
    let luz2 = inserir(&mut d, "ILU-OCT", 5.0, 2.0, &[("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    s.open_drawing(d, "Planta", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "1", "pontos": [duas.hex(), luz.hex(), luz2.hex()] })).unwrap();
    // Duas teclas: duas passadas, duas letras.
    let r = s.execute("elecomando", &json!({ "interruptor": duas.hex(), "luminarias": [luz.hex()] })).unwrap();
    assert_eq!(r["comando"], "a");
    let r = s.execute("elecomando", &json!({ "interruptor": duas.hex(), "luminarias": [luz2.hex()] })).unwrap();
    assert_eq!(r["comando"], "b");
    let ler = |s: &Session, h: Handle| free_eletrica::ponto::ler_ponto(s.doc().unwrap(), h).unwrap();
    assert_eq!(ler(&s, duas).comando.as_deref(), Some("ab"));
    assert_eq!(ler(&s, luz2).comando.as_deref(), Some("b"));
    // O vínculo fica no XDATA do interruptor.
    let extras = ler(&s, duas).extras;
    assert!(extras.iter().any(|(k, v)| k == "target_handles.a" && *v == luz.hex()), "{extras:?}");
    // Interativo com letra própria; sem tecla livre a letra troca a última.
    s.start("elecomando").unwrap();
    s.input(Input::Pick(vec![duas])).unwrap();
    s.input(Input::Keyword("Letra".into())).unwrap();
    s.input(Input::Text("x".into())).unwrap();
    s.input(Input::Pick(vec![luz2])).unwrap();
    s.input(Input::Enter).unwrap();
    assert_eq!(ler(&s, duas).comando.as_deref(), Some("ax"));
    assert_eq!(ler(&s, luz2).comando.as_deref(), Some("x"));
    // O que não é interruptor é recusado.
    assert!(s.execute("elecomando", &json!({ "interruptor": tug.hex(), "luminarias": [] })).is_err());
    assert!(s.execute("elecomando", &json!({ "interruptor": duas.hex() })).is_err());
}

#[test]
fn tags_tabela_e_inspecao_por_comandos() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let (d, [qd, luz, _int, tug]) = planta();
    s.open_drawing(d, "Planta", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "1", "pontos": [luz.hex()] })).unwrap();
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "2", "pontos": [tug.hex()] })).unwrap();
    let ee = s.execute("eletrica.eletrodutos", &json!({})).unwrap();
    let primeiro = ee["trechos"][0]["handle"].as_str().unwrap().to_string();
    // Tag num eletroduto escolhido; handle ruim não vira tag.
    assert_eq!(s.execute("elefiacao", &json!({ "eletrodutos": [primeiro] })).unwrap()["criadas"], 1);
    assert!(s.execute("elefiacao", &json!({ "eletrodutos": ["zzz"] })).is_err());

    // O projetista move a tag; mudar o quadro atualiza sem mexer nela.
    let (tag, _) = free_eletrica::fiacao::tags(s.doc().unwrap())[0];
    let mover = |s: &mut Session| {
        let doc = s.doc_mut().unwrap();
        let mut e = (**doc.model.get(tag).unwrap()).clone();
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert.y += 1.5;
        }
        doc.model.replace(e);
    };
    mover(&mut s);
    let y = |s: &Session| {
        if let EntityKind::Insert(i) = &s.doc().unwrap().model.get(tag).unwrap().kind { (i.insert.y, i.block.clone()) } else { panic!() }
    };
    let (antes, bloco) = y(&s);
    let textos = |s: &Session, b: &str| -> Vec<String> {
        s.doc().unwrap().blocks[b]
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect()
    };
    assert!(!textos(&s, &bloco).contains(&"#6".to_string()));
    s.execute("eletrica.sobrescrever", &json!({ "circuito": "2", "secao": 6 })).unwrap();
    // Editar não dimensiona: as tags só mudam ao processar o cálculo.
    assert!(!textos(&s, &bloco).contains(&"#6".to_string()));
    let r = s.execute("eletrica.dimensionar", &json!({})).unwrap();
    assert_eq!(r["quadros"][0]["tag"], "QDG");
    assert_eq!(y(&s).0, antes);
    assert!(textos(&s, &bloco).contains(&"#6".to_string()), "{:?}", textos(&s, &bloco));
    let r = s.execute("eleregen", &json!({})).unwrap();
    assert_eq!(r["tags"], 1);
    assert!(r["message"].as_str().unwrap().contains("Fiação"));
    // Digitado: sem parâmetros.
    s.start("eleregen").unwrap();
    assert!(s.running.is_none());

    // Tabela: por JSON, por prompt, um quadro só tem uma, e ela segue o quadro.
    assert!(s.execute("elequadro_tabela", &json!({})).is_err());
    assert!(s.execute("elequadro_tabela", &json!({ "ponto": [1] })).is_err());
    let r = s.execute("elequadro_tabela", &json!({ "ponto": [20.0, 5.0] })).unwrap();
    let t = r["tabela"].as_str().unwrap().to_string();
    s.start("elequadro_tabela").unwrap();
    s.input(Input::Point(Vec2::new(30.0, 5.0))).unwrap();
    assert!(s.running.is_none());
    assert_eq!(free_eletrica::tabela::tabelas(s.doc().unwrap()).len(), 1);
    let h = Handle::parse_hex(&t).unwrap();
    let EntityKind::Insert(i) = &s.doc().unwrap().model.get(h).unwrap().kind else { panic!() };
    assert_eq!(i.insert.x, 30.0);
    s.execute("eletrica.circuito.descrever", &json!({ "circuito": "1", "descricao": "Salão" })).unwrap();
    s.execute("eleregen", &json!({})).unwrap();
    let nome = i_bloco(&s, h);
    assert!(textos(&s, &nome).contains(&"Salão".to_string()));

    // Inspeção: o interruptor sem circuito é vermelho, o quadro é verde.
    let v = s.execute("eletrica.inspecionar", &json!({})).unwrap();
    let itens = v.as_array().unwrap();
    assert!(itens.iter().any(|i| i["handle"].as_str() == Some(qd.hex().as_str()) && i["nivel"] == "verde"), "{v}");
    assert_eq!(itens[0]["nivel"], "vermelho");
    // Sem eletroduto nenhum, elefiacao avisa em vez de criar nada.
    let mut vazio = Session::new();
    vazio.open_drawing(Drawing::new_metric(), "Vazio", None);
    assert!(vazio.execute("elefiacao", &json!({})).is_err());
    assert!(vazio.execute("eleregen", &json!({})).is_ok());
    assert!(vazio.execute("eletrica.inspecionar", &json!({})).unwrap().as_array().unwrap().is_empty());
}

fn i_bloco(s: &Session, h: Handle) -> String {
    let EntityKind::Insert(i) = &s.doc().unwrap().model.get(h).unwrap().kind else { panic!() };
    i.block.clone()
}

#[test]
fn inserir_e_excluir_pontos_por_comandos() {
    let mut s = Session::new();
    let (mut d, _) = planta();
    bloco_tomada(&mut d);
    s.open_drawing(d, "Planta", None);
    let blocos = s.execute("eletrica.blocos", &json!({})).unwrap();
    let nomes: Vec<&str> = blocos.as_array().unwrap().iter().map(|b| b["nome"].as_str().unwrap()).collect();
    assert!(nomes.contains(&"TOM-SIM-BAIXA"), "{blocos}");
    // JSON: duas tomadas de 600 VA no circuito 4.
    let r = s
        .execute(
            "eleponto",
            &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[1.0, 2.0], [2.0, 2.0]], "valores": { "POTENCIA": "600VA", "CIRCUITO": 4 } }),
        )
        .unwrap();
    let novos: Vec<String> = r["pontos"].as_array().unwrap().iter().map(|h| h.as_str().unwrap().to_string()).collect();
    assert_eq!(novos.len(), 2);
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    let c4 = a["circuitos"].as_array().unwrap().iter().find(|c| c["id"] == "4").unwrap();
    assert_eq!(c4["potencia"].as_f64(), Some(1200.0));
    assert!(s.execute("eleponto", &json!({ "bloco": "NENHUM", "pontos": [[0, 0]] })).is_err());
    assert!(s.execute("eleponto", &json!({ "bloco": "TOM-SIM-BAIXA", "pontos": [[0, 0]], "valores": { "TIPO_PONTO": "TUE" } })).is_err());

    // Interativo com o modelo escolhido antes (a janela Inserir pontos): dois cliques e Enter.
    s.execute("eletrica.ponto.modelo", &json!({ "bloco": "tom-sim-baixa", "rotacao": 90, "valores": { "CIRCUITO": "5" } })).unwrap();
    s.start("eleponto").unwrap();
    s.input(Input::Point(Vec2::new(3.0, 3.0))).unwrap();
    s.input(Input::Point(Vec2::new(4.0, 3.0))).unwrap();
    s.input(Input::Enter).unwrap();
    assert!(s.running.is_none());
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    assert_eq!(a["pontos"].as_array().unwrap().iter().filter(|p| p["circuito"] == "5").count(), 2);
    assert!(s.execute("eletrica.ponto.modelo", &json!({ "bloco": "PORTA" })).is_err());

    // Excluir: os dois do circuito 4; um handle que não é ponto recusa tudo.
    let linha = s.doc().unwrap().model.iter().find(|e| matches!(e.kind, EntityKind::Line(_))).unwrap().handle.hex();
    assert!(s.execute("eletrica.ponto.excluir", &json!({ "pontos": [novos[0], linha] })).is_err());
    let r = s.execute("eletrica.ponto.excluir", &json!({ "pontos": novos })).unwrap();
    assert_eq!(r["excluidos"], 2);
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    assert!(a["circuitos"].as_array().unwrap().iter().all(|c| c["id"] != "4"));
}

#[test]
fn propriedades_do_circuito_e_demanda_por_comandos() {
    let mut s = Session::new();
    let (d, [qd, luz, _int, tug]) = planta();
    s.open_drawing(d, "Planta", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()] })).unwrap();
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [tug.hex()] })).unwrap();
    let linha = |s: &mut Session, id: &str| -> Value {
        let r = s.execute("eletrica.rotear", &json!({})).unwrap();
        r["linhas"].as_array().unwrap().iter().find(|l| l["circuito"]["id"] == id).unwrap().clone()
    };

    // Isolação só do circuito 2, num passo de desfazer; null volta à do quadro.
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "isolacao": "EPR" })).unwrap();
    assert_eq!(linha(&mut s, "2")["isolacao"], "epr");
    assert_eq!(linha(&mut s, "1")["isolacao"], "pvc");
    s.undo().unwrap();
    assert_eq!(linha(&mut s, "2")["isolacao"], "pvc");
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "isolacao": "XLPE" })).unwrap();
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "isolacao": null })).unwrap();
    assert_eq!(linha(&mut s, "2")["isolacao"], "pvc");
    // Tipo de carga e FP são atributos dos blocos: o comando recusa e não grava nada.
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "2" })).is_err());
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "isolacao": "EPR", "fp": 0.8 })).is_err());
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "isolacao": "ouro" })).is_err());
    assert_eq!(linha(&mut s, "2")["isolacao"], "pvc");

    // TIPO_CARGA e FP no bloco da TUG: o quadro de cargas mostra o resultado.
    {
        let doc = s.doc_mut().unwrap();
        free_eletrica::editar::definir(doc, tug, free_eletrica::ponto::TAGS_CARGA, Some("RESERVA")).unwrap();
        free_eletrica::editar::definir(doc, tug, free_eletrica::ponto::TAGS_FP, Some("0,85")).unwrap();
    }
    let l2 = linha(&mut s, "2");
    assert_eq!((l2["tiposCarga"][0].as_str(), l2["fp"].as_f64()), (Some("reserva"), Some(0.85)));
    // A reserva sai da demanda: só a luz de 100 VA (100 %).
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!(r["demanda"]["demandada"].as_f64(), Some(100.0));

    // Tabelas de demanda, também sem desenho.
    let t = s.execute("eletrica.demanda.tabelas", &json!({})).unwrap();
    assert_eq!(t[0]["id"], "equatorial-nt001-r07");
    assert!(t[0]["fonte"].as_str().unwrap().contains("NT.00001.EQTL"));
}

#[test]
fn classe_e_fator_de_demanda_por_comando() {
    let mut s = Session::default();
    s.open_drawing(Drawing::new_metric(), "Demanda", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let r = s.execute("eleponto", &json!({ "bloco": "TOM-TUE", "pontos": [[1.0, 1.0]] })).unwrap();
    let h = r["pontos"][0].as_str().unwrap().to_string();
    // Valores ruins não gravam nada.
    assert!(s.execute("eletrica.ponto.demanda", &json!({ "pontos": [h] })).is_err());
    assert!(s.execute("eletrica.ponto.demanda", &json!({ "pontos": [h], "classe": "XPTO" })).is_err());
    assert!(s.execute("eletrica.ponto.demanda", &json!({ "pontos": [h], "fator": 1.5 })).is_err());
    assert!(s.execute("eletrica.ponto.demanda", &json!({ "pontos": ["FFFFF"], "fator": 0.5 })).is_err());
    let ponto = |s: &mut Session| {
        let a = s.execute("eletrica.analisar", &json!({})).unwrap();
        a["pontos"].as_array().unwrap().iter().find(|p| p["handle"] == h.as_str()).unwrap().clone()
    };
    s.execute("eletrica.ponto.demanda", &json!({ "pontos": [h], "classe": "MOTORES", "fator": 0.7 })).unwrap();
    let p = ponto(&mut s);
    assert_eq!((p["tipoCarga"].as_str(), p["fatorDemanda"].as_f64()), (Some("motores"), Some(0.7)));
    // Null volta à família: a classe natural do bloco (TOM-TUE é aquecimento no atributo apagado) e a tabela.
    s.execute("eletrica.ponto.demanda", &json!({ "pontos": [h], "fator": null })).unwrap();
    assert!(ponto(&mut s)["fatorDemanda"].is_null());
    // Família: a classe e o fator ficam no bloco e valem para as inserções sem valor próprio.
    s.execute("eletrica.familia.definir", &json!({ "bloco": "TOM-TUE", "classeDemanda": "AR", "fatorDemanda": 0.8 })).unwrap();
    let f = s.execute("eletrica.familia.ler", &json!({ "bloco": "TOM-TUE" })).unwrap();
    assert_eq!((f["classeDemanda"].as_str(), f["fatorDemanda"].as_f64()), (Some("arCondicionado"), Some(0.8)));
    assert_eq!(ponto(&mut s)["fatorDemanda"].as_f64(), Some(0.8));
    assert!(s.execute("eletrica.familia.definir", &json!({ "bloco": "TOM-TUE", "fatorDemanda": 2 })).is_err());
    let novo = s.execute("eleponto", &json!({ "bloco": "TOM-TUE", "pontos": [[2.0, 1.0]] })).unwrap();
    let a = s.execute("eletrica.analisar", &json!({})).unwrap();
    let inserido = a["pontos"].as_array().unwrap().iter().find(|p| p["handle"] == novo["pontos"][0]).unwrap().clone();
    assert_eq!(inserido["tipoCarga"].as_str(), Some("arCondicionado"), "a inserção nova herda a classe da família");
    s.execute("eletrica.familia.definir", &json!({ "bloco": "TOM-TUE", "classeDemanda": null, "fatorDemanda": null })).unwrap();
    assert!(s.execute("eletrica.familia.ler", &json!({ "bloco": "TOM-TUE" })).unwrap()["classeDemanda"].is_null());
}

#[test]
fn tabelas_de_demanda_por_comando() {
    let _setup = trava_setup();
    let mut s = Session::default();
    let ids = |s: &mut Session| -> Vec<String> {
        s.execute("eletrica.demanda.tabelas", &json!({})).unwrap().as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_string()).collect()
    };
    let de_fabrica = ids(&mut s);
    assert!(["equatorial-nt001-r07", "cemig-nd51", "enel-go-ntc04-r4"].iter().all(|i| de_fabrica.iter().any(|d| d == i)), "{de_fabrica:?}");
    assert!(s.execute("eletrica.demanda.salvar", &json!({})).is_err());
    assert!(s.execute("eletrica.demanda.salvar", &json!({ "tabela": { "nome": "Sem edificação" } })).is_err());
    let t = json!({
        "nome": "Teste de comando",
        "edificacoes": [{ "nome": "Casa", "modo": "totalNaFaixa", "faixas": [{ "ateKw": 5, "fator": 0.8 }, { "ateKw": null, "fator": 0.5 }] }],
        "motores": [{ "ate": 1, "fator": 1.0 }]
    });
    let r = s.execute("eletrica.demanda.salvar", &json!({ "tabela": t })).unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    assert!(ids(&mut s).contains(&id));
    assert!(
        s.execute(
            "eletrica.demanda.salvar",
            &json!({ "tabela": { "id": "cemig-nd51", "nome": "Outra", "edificacoes": [{ "nome": "Casa", "faixas": [{ "fator": 1 }] }] } })
        )
        .is_err(),
        "as de fábrica não mudam"
    );
    assert_eq!(s.execute("eletrica.demanda.excluir", &json!({ "id": id })).unwrap()["excluida"], true);
    assert_eq!(s.execute("eletrica.demanda.excluir", &json!({ "id": id })).unwrap()["excluida"], false);
    assert!(!ids(&mut s).contains(&id));
}

#[test]
fn configuracao_de_cada_quadro_por_comando() {
    let (d, [qd, luz, ..]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Quadro", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "pontos": [luz.hex()] })).unwrap();
    // Valor ruim não grava nada.
    for ruim in [
        json!({ "sistema": "4F" }),
        json!({ "tensaoNominal": "13800" }),
        json!({ "cabo": "OURO" }),
        json!({ "alimentadoPor": "QX" }),
        json!({ "alimentadoPor": "QDG" }),
    ] {
        assert!(s.execute("eletrica.quadro", &ruim).is_err(), "{ruim}");
    }
    let c = s.execute("eletrica.quadro", &json!({})).unwrap();
    assert!(c["config"]["sistema"].is_null());
    s.execute("eletrica.quadro", &json!({ "sistema": "F+N+T", "tensaoNominal": "127/220", "cabo": "LSZH_70", "alimentadoPor": "CONCESSIONARIA" }))
        .unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!((r["fasesQuadro"].as_u64(), r["tensaoFn"].as_f64(), r["caboPadrao"].as_str()), (Some(1), Some(127.0), Some("lszh70")), "{r}");
    assert_eq!(r["config"]["alimentadoPor"], "CONCESSIONARIA");
    assert_eq!(r["linhas"][0]["tensao"], 127.0);
    // Um campo não mexe nos outros; null volta ao que o bloco diz; um passo de desfazer.
    s.execute("eletrica.quadro", &json!({ "tensaoNominal": "220/380" })).unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!((r["tensaoFn"].as_f64(), r["caboPadrao"].as_str()), (Some(220.0), Some("lszh70")));
    s.undo().unwrap();
    assert_eq!(s.execute("eletrica.rotear", &json!({})).unwrap()["tensaoFn"], 127.0);
    s.execute("eletrica.quadro", &json!({ "cabo": null })).unwrap();
    assert!(s.execute("eletrica.rotear", &json!({})).unwrap()["config"]["cabo"].is_null());
}

#[test]
fn perfil_de_eletroduto_por_comando() {
    let (d, [qd, luz, ..]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Perfil", None);
    let nome = "Eletroduto PEAD - Piso";
    // Um perfil que não existe é erro: não desenha sem ele.
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex(), "perfil": "Nenhum" })).is_err());
    assert!(s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex(), "perfil": 3 })).is_err());
    assert!(s.execute("eletrica.eletroduto.modelo", &json!({ "perfil": "Nenhum" })).is_err());
    let h = s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex(), "perfil": nome, "tracado": "ANGLE45" })).unwrap()["eletroduto"]
        .as_str()
        .unwrap()
        .to_string();
    let e = s.doc().unwrap().model.get(Handle::parse_hex(&h).unwrap()).unwrap().clone();
    assert_eq!((e.common.linetype.as_str(), e.common.layer.as_str()), ("DASHDOT", free_eletrica::eletroduto::CAMADA_PISO));
    // A mini-barra: o perfil traz o tipo e o caminhamento; escolher um tipo à mão sai do perfil.
    let m = s.execute("eletrica.eletroduto.modelo", &json!({ "perfil": nome })).unwrap();
    assert_eq!((m["perfil"].as_str(), m["instalacao"].as_str(), m["tipo"].as_str()), (Some(nome), Some("piso"), Some("peadCorrugado")));
    let m = s.execute("eletrica.eletroduto.modelo", &json!({ "tipo": "PVC_RIGID" })).unwrap();
    assert!(m["perfil"].is_null());
    let uid = s.state().unwrap().uid;
    assert!(free_eletrica::eletroduto::modelo_definido(uid));
    // Interativo, num desenho novo: começa no primeiro perfil do projeto.
    let (d, [qd2, luz2, ..]) = planta();
    let mut s2 = Session::default();
    s2.open_drawing(d, "Perfil 2", None);
    s2.start("eleeletroduto").unwrap();
    s2.input(Input::Pick(vec![qd2])).unwrap();
    s2.input(Input::Keyword("Reto".into())).unwrap();
    s2.input(Input::Pick(vec![luz2])).unwrap();
    s2.input(Input::Enter).unwrap();
    let t = s2.execute("eletrica.eletrodutos", &json!({})).unwrap();
    let achado = t["trechos"].as_array().unwrap().iter().find(|x| x["perfil"] == "Eletroduto PVC Flexível - Teto");
    assert!(achado.is_some(), "{t}");
    // Aplicar um perfil a um trecho existente.
    s.execute("eletrica.eletroduto.definir", &json!({ "eletroduto": h, "perfil": "Eletroduto Aço Galvanizado" })).unwrap();
    let e = s.doc().unwrap().model.get(Handle::parse_hex(&h).unwrap()).unwrap().clone();
    // O aço galvanizado de fábrica corre no teto: linha contínua.
    assert_eq!(e.common.linetype, "Continuous");
}

#[test]
fn nome_do_cabo_do_catalogo_por_comando() {
    let (d, [qd, luz, ..]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Cabo", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "1", "pontos": [luz.hex()] })).unwrap();
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "caboNome": 3 })).is_err());
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "cabo": "LSZH_70", "caboNome": "Cabo da obra" })).unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_eq!((r["linhas"][0]["tipoCabo"].as_str(), r["linhas"][0]["nomeCabo"].as_str()), (Some("lszh70"), Some("Cabo da obra")));
    // Trocar a classe sem dar nome volta ao nome da classe.
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "cabo": "HEPR_XLPE_90" })).unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    assert_ne!(r["linhas"][0]["nomeCabo"], "Cabo da obra");
}

#[test]
fn esquema_curva_dr_e_unifilar_por_comando() {
    let _setup = trava_setup();
    let (d, [qd, luz, _, tug]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Dimensionar", None);
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "1", "pontos": [luz.hex()] })).unwrap();
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "2", "pontos": [tug.hex()] })).unwrap();
    // Os esquemas novos, a curva e qualquer nome de grupo de DR.
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "curva": "Z" })).is_err());
    assert!(s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "esquema": "4F" })).is_err());
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "esquema": "2F+N+T", "curva": "B", "grupoDr": "IDR9" })).unwrap();
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "2", "esquema": "3F+T", "grupoDr": "IDR9" })).unwrap();
    let r = s.execute("eletrica.rotear", &json!({})).unwrap();
    let l = r["linhas"].as_array().unwrap();
    assert_eq!(
        (l[0]["esquema"].as_str(), l[0]["condutores"].as_u64(), l[0]["curva"].as_str(), l[0]["curvaPropria"].as_bool()),
        (Some("ffnt"), Some(4), Some("B"), Some(true))
    );
    assert_eq!((l[1]["esquema"].as_str(), l[1]["condutores"].as_u64(), l[1]["curva"].as_str()), (Some("ffft"), Some(4), Some("C")));
    s.execute("eletrica.circuito.definir", &json!({ "circuito": "1", "curva": null })).unwrap();
    assert_eq!(s.execute("eletrica.rotear", &json!({})).unwrap()["linhas"][0]["curvaPropria"], false);
    // Dimensionar com o unifilar: o diagrama nasce e é um passo de desfazer.
    let v = s.execute("eletrica.dimensionar", &json!({ "unifilar": true })).unwrap();
    assert_eq!(v["unifilares"], 1);
    let contar = |s: &Session| free_eletrica::unifilar::unifilares(s.doc().unwrap()).len();
    assert_eq!(contar(&s), 1);
    s.execute("eletrica.dimensionar", &json!({ "unifilar": true })).unwrap();
    assert_eq!(contar(&s), 1, "atualiza, não duplica");
    s.undo().unwrap();
    assert_eq!(contar(&s), 1);
    s.undo().unwrap();
    assert_eq!(contar(&s), 0);
    assert_eq!(s.execute("eletrica.dimensionar", &json!({})).unwrap()["unifilares"], 0);
    assert_eq!(contar(&s), 0, "sem pedir, não cria");
    // Direto, num ponto.
    assert!(s.execute("eletrica.unifilar", &json!({ "ponto": "aqui" })).is_err());
    let u = s.execute("eletrica.unifilar", &json!({ "ponto": [10.0, 20.0] })).unwrap();
    let h = Handle::parse_hex(u["unifilar"].as_str().unwrap()).unwrap();
    let EntityKind::Insert(i) = &s.doc().unwrap().model.get(h).unwrap().kind else { panic!("inserção") };
    assert_eq!((i.insert.x, i.insert.y), (10.0, 20.0));
    s.execute("eletrica.setup", &json!({ "compartilharPe": false })).unwrap();
}

#[test]
fn bloco_no_caminho_do_eletroduto_ja_e_a_caixa() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Planta", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let ponto = |s: &mut Session, b: &str, x: f64, y: f64| -> String {
        s.execute("eleponto", &json!({ "bloco": b, "pontos": [[x, y]] })).unwrap()["pontos"][0].as_str().unwrap().to_string()
    };
    let trechos = |s: &mut Session| s.execute("eletrica.eletrodutos", &json!({})).unwrap();
    let q = ponto(&mut s, "QDG", 0.0, 0.0);
    let meio = ponto(&mut s, "TOM-SIM-BAIXA", 2.0, 0.0);
    let fim = ponto(&mut s, "TOM-SIM-BAIXA", 4.0, 0.0);
    // O trecho novo que passa por um bloco se corta nele.
    s.execute("eleeletroduto", &json!({ "de": q, "para": fim, "tracado": "STRAIGHT" })).unwrap();
    let r = trechos(&mut s);
    assert_eq!((r["trechos"].as_array().unwrap().len(), r["soltos"].as_array().unwrap().len()), (2, 0), "{r}");
    assert!(r["trechos"].as_array().unwrap().iter().any(|t| t["para"] == meio.as_str() || t["de"] == meio.as_str()));
    // Um ponto novo sobre um trecho existente também corta, sem caixa nova.
    let novo = ponto(&mut s, "TOM-SIM-BAIXA", 3.0, 0.0);
    let r = trechos(&mut s);
    assert_eq!(r["trechos"].as_array().unwrap().len(), 3, "{r}");
    assert!(r["trechos"].as_array().unwrap().iter().any(|t| t["de"] == novo.as_str() || t["para"] == novo.as_str()));
    assert_eq!(s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"].as_array().unwrap().len(), 4);
    // Clicar num bloco a 20 cm de um eletroduto não mexe nele: cada trecho existente é imutável.
    let de_lado = ponto(&mut s, "TOM-SIM-BAIXA", 1.0, 0.4);
    assert_eq!(trechos(&mut s)["trechos"].as_array().unwrap().len(), 3);
    let outro = ponto(&mut s, "TOM-SIM-BAIXA", 1.0, 3.0);
    let antes: Vec<_> = s.doc().unwrap().model.iter().filter(|e| e.common.layer.starts_with("ELE-ELETRODUTO")).map(|e| (**e).clone()).collect();
    s.start("eleeletroduto").unwrap();
    s.input(Input::Point(Vec2::new(1.0, 0.4))).unwrap();
    s.input(Input::Point(Vec2::new(1.0, 3.0))).unwrap();
    s.input(Input::Enter).unwrap();
    let r = trechos(&mut s);
    let lista = r["trechos"].as_array().unwrap();
    assert_eq!((lista.len(), r["soltos"].as_array().unwrap().len()), (4, 0), "{r}");
    assert_eq!(lista.iter().filter(|t| t["de"] == de_lado.as_str() || t["para"] == de_lado.as_str()).count(), 1, "{r}");
    assert!(lista.iter().any(|t| t["de"] == outro.as_str() || t["para"] == outro.as_str()));
    for e in &antes {
        assert_eq!(s.doc().unwrap().model.get(e.handle).map(|x| x.kind.clone()), Some(e.kind.clone()), "o trecho antigo não muda");
    }
}

#[test]
fn documentacao_dos_pontos_e_legenda_por_comando() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Legenda", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let h = s.execute("eleponto", &json!({ "bloco": "TOM-SIM-MEDIA", "pontos": [[1.0, 1.0]] })).unwrap()["pontos"][0].as_str().unwrap().to_string();
    let f = s.execute("eletrica.familia.ler", &json!({ "bloco": "TOM-SIM-MEDIA" })).unwrap();
    assert_eq!(f["legenda"], "Tomada média 2P+T 10A - h=1,20m");
    // Na família e no ponto; vazio volta à família.
    s.execute("eletrica.familia.definir", &json!({ "bloco": "TOM-SIM-MEDIA", "legenda": "Dupla", "descMaterial": "Kit" })).unwrap();
    let ler = |s: &mut Session| s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"][0].clone();
    assert_eq!(ler(&mut s)["legenda"], "Dupla");
    assert_eq!(ler(&mut s)["descMaterial"], "Kit");
    s.execute("eletrica.ponto.documentacao", &json!({ "pontos": [h], "legenda": "Da bancada" })).unwrap();
    assert_eq!(ler(&mut s)["legenda"], "Da bancada");
    s.execute("eletrica.ponto.documentacao", &json!({ "pontos": [h], "legenda": "" })).unwrap();
    assert_eq!(ler(&mut s)["legenda"], "Dupla");
    assert!(s.execute("eletrica.ponto.documentacao", &json!({ "pontos": [h] })).is_err());
    assert!(s.execute("eletrica.ponto.documentacao", &json!({ "pontos": [h], "legenda": 3 })).is_err());
    assert!(s.execute("eletrica.ponto.documentacao", &json!({ "pontos": [h], "legenda": "x".repeat(201) })).is_err());
    assert!(s.execute("eletrica.ponto.documentacao", &json!({ "pontos": ["0"], "legenda": "x" })).is_err());
    assert!(s.execute("eletrica.familia.definir", &json!({ "bloco": "TOM-SIM-MEDIA", "legenda": 3 })).is_err());
    // A lista de materiais conta pela descrição.
    let m = s.execute("eletrica.materiais", &json!({})).unwrap();
    assert_eq!(m["conjuntos"][0]["nome"], "Kit");
    // A legenda da prancha, por JSON e clicando.
    let r = s.execute("elelegenda", &json!({ "ponto": [10.0, 10.0] })).unwrap();
    assert_eq!(r["linhas"], 1);
    assert!(s.execute("elelegenda", &json!({})).is_err());
    let antes = s.doc().unwrap().model.len();
    s.start("elelegenda").unwrap();
    s.input(Input::Point(Vec2::new(20.0, 20.0))).unwrap();
    assert!(s.running.is_none());
    assert_eq!(s.doc().unwrap().model.len(), antes, "a legenda é uma só");
}

#[test]
fn excluir_por_comandos() {
    let _setup = trava_setup();
    let (d, [qd, luz, ..]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Excluir", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    // Grupo de DR: os circuitos ficam, sem DR.
    s.execute("elecircuito", &json!({ "quadro": qd.hex(), "circuito": "7", "pontos": [luz.hex()] })).unwrap();
    s.execute("eletrica.circuito.definir", &json!({ "quadro": qd.hex(), "circuito": "7", "grupoDr": "IDR1" })).unwrap();
    assert_eq!(s.execute("eletrica.dr.excluir", &json!({ "quadro": qd.hex(), "grupo": "idr1" })).unwrap()["circuitos"], 1);
    let r = s.execute("eletrica.rotear", &json!({ "quadro": qd.hex() })).unwrap();
    assert!(r["linhas"].as_array().unwrap().iter().all(|l| l["grupoDr"].is_null()), "{r}");
    assert!(s.execute("eletrica.dr.excluir", &json!({ "quadro": qd.hex(), "grupo": "idr1" })).is_err());
    assert!(s.execute("eletrica.dr.excluir", &json!({ "quadro": qd.hex() })).is_err());
    // Eletroduto.
    let e = s.execute("eleeletroduto", &json!({ "de": qd.hex(), "para": luz.hex() })).unwrap()["eletroduto"].as_str().unwrap().to_string();
    s.execute("elefiacao", &json!({ "eletrodutos": [e] })).unwrap();
    assert!(s.execute("eletrica.eletroduto.excluir", &json!({ "eletrodutos": [qd.hex()] })).is_err());
    assert_eq!(s.execute("eletrica.eletroduto.excluir", &json!({ "eletrodutos": [e] })).unwrap()["excluidos"], 1);
    s.undo().unwrap();
    assert!(s.doc().unwrap().model.get(Handle::parse_hex(&e).unwrap()).is_some(), "desfaz");
    // Bloco e família; com inserção só pedindo; a legenda não fica com o símbolo que saiu.
    s.execute("eleponto", &json!({ "bloco": "TOM-SIM-ALTA", "pontos": [[20.0, 20.0]] })).unwrap();
    s.execute("elelegenda", &json!({ "ponto": [30.0, 30.0] })).unwrap();
    assert!(s.execute("eletrica.familia.excluir", &json!({ "bloco": "TOM-SIM-ALTA" })).is_err());
    assert_eq!(s.execute("eletrica.familia.excluir", &json!({ "bloco": "TOM-SIM-ALTA", "comInsercoes": true })).unwrap()["insercoes"], 1);
    assert!(!s.execute("eletrica.blocos", &json!({})).unwrap().as_array().unwrap().iter().any(|b| b["nome"] == "TOM-SIM-ALTA"));
    let legenda = s.doc().unwrap().blocks.get(free_eletrica::legenda::BLOCO).unwrap().clone();
    assert!(!legenda.entities.iter().any(|e| matches!(&e.kind, EntityKind::Insert(i) if i.block == "TOM-SIM-ALTA")));
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    assert!(!s.execute("eletrica.blocos", &json!({})).unwrap().as_array().unwrap().iter().any(|b| b["nome"] == "TOM-SIM-ALTA"));
    s.execute("eletrica.bootstrap", &json!({ "restaurar": true })).unwrap();
    assert!(s.execute("eletrica.blocos", &json!({})).unwrap().as_array().unwrap().iter().any(|b| b["nome"] == "TOM-SIM-ALTA"));
    // Tabela de fábrica.
    let antes = s.execute("eletrica.demanda.tabelas", &json!({})).unwrap().as_array().unwrap().len();
    assert_eq!(s.execute("eletrica.demanda.excluir", &json!({ "id": "cemig-nd51" })).unwrap()["excluida"], true);
    assert_eq!(s.execute("eletrica.demanda.tabelas", &json!({})).unwrap().as_array().unwrap().len(), antes - 1);
    s.execute("eletrica.setup", &json!({ "tabelasExcluidas": [] })).unwrap();
    assert_eq!(s.execute("eletrica.demanda.tabelas", &json!({})).unwrap().as_array().unwrap().len(), antes);
}

#[test]
fn tag_de_fiacao_direta_ou_com_chamada() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Tag", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let ponto = |s: &mut Session, b: &str, x: f64| {
        s.execute("eleponto", &json!({ "bloco": b, "pontos": [[x, 0.0]] })).unwrap()["pontos"][0].as_str().unwrap().to_string()
    };
    let q = ponto(&mut s, "QDG", 0.0);
    let f = ponto(&mut s, "TOM-SIM-BAIXA", 6.0);
    let e = s.execute("eleeletroduto", &json!({ "de": q, "para": f, "tracado": "STRAIGHT" })).unwrap()["eletroduto"].as_str().unwrap().to_string();
    let insercao = |s: &Session| {
        let (h, _) = free_eletrica::fiacao::tags(s.doc().unwrap())[0];
        match &s.doc().unwrap().model.get(h).unwrap().kind {
            EntityKind::Insert(i) => (h, i.insert.x, i.insert.y),
            _ => panic!("inserção"),
        }
    };
    // O modo começa direto: um clique põe a tag sobre o eletroduto.
    assert_eq!(s.execute("eletrica.fiacao.modo", &json!({})).unwrap()["modo"], "direct");
    assert!(s.execute("eletrica.fiacao.modo", &json!({ "modo": "x" })).is_err());
    s.start("elefiacao").unwrap();
    s.input(Input::Point(Vec2::new(3.0, 5.0))).unwrap();
    assert!(s.running.is_some(), "fora do eletroduto só avisa");
    s.input(Input::Point(Vec2::new(2.0, 0.05))).unwrap();
    assert!(s.preview(Vec2::new(5.0, 2.0)).is_empty());
    let (h, x, y) = insercao(&s);
    assert_eq!((x, y), (2.0, 0.0));
    // Com linha de chamada, trocada com o comando correndo: dois cliques e a prévia elástica.
    s.input(Input::Keyword("Chamada".into())).unwrap();
    s.input(Input::Point(Vec2::new(3.0, 0.0))).unwrap();
    assert_eq!(s.preview(Vec2::new(5.0, 2.0)).len(), 2);
    s.input(Input::Point(Vec2::new(5.0, 2.0))).unwrap();
    assert!(s.preview(Vec2::new(5.0, 2.0)).is_empty(), "a prévia some");
    s.input(Input::Enter).unwrap();
    assert!(s.running.is_none());
    assert_eq!(insercao(&s), (h, 5.0, 2.0), "uma tag por eletroduto");
    assert_eq!(s.execute("eletrica.fiacao.modo", &json!({})).unwrap()["modo"], "leader");
    // Por JSON, sem interface, e o erro não trava.
    let r = s.execute("elefiacao", &json!({ "ancora": [4.0, 0.0], "tag": [6.0, 1.0] })).unwrap();
    assert_eq!((r["tag"].as_str(), r["modo"].as_str()), (Some(h.hex().as_str()), Some("leader")));
    let r = s.execute("elefiacao", &json!({ "ancora": [4.0, 0.0] })).unwrap();
    assert_eq!(r["modo"], "direct");
    assert!(s.execute("elefiacao", &json!({ "modo": "leader", "ancora": [4.0, 0.0] })).is_err());
    assert!(s.execute("elefiacao", &json!({ "modo": "zig", "ancora": [4.0, 0.0] })).is_err());
    assert!(s.execute("elefiacao", &json!({ "tag": [4.0, 0.0] })).is_err());
    assert!(s.execute("elefiacao", &json!({ "ancora": [40.0, 40.0], "tag": [6.0, 1.0] })).is_err());
    assert!(s.execute("elefiacao", &json!({ "eletroduto": q, "ancora": [4.0, 0.0], "tag": [6.0, 1.0] })).is_err());
    s.execute("elefiacao", &json!({ "eletroduto": e, "modo": "leader", "ancora": [4.0, 0.0], "tag": [7.0, 1.0] })).unwrap();
    // ELEREGEN preserva.
    s.execute("eleregen", &json!({})).unwrap();
    assert_eq!(insercao(&s), (h, 7.0, 1.0));
    // Esc no segundo clique cancela sem criar nada.
    s.execute("elefiacao", &json!({ "limpar": true })).unwrap();
    s.start("elefiacao").unwrap();
    s.input(Input::Point(Vec2::new(2.0, 0.0))).unwrap();
    s.input(Input::Cancel).unwrap();
    assert!(s.running.is_none());
    assert!(free_eletrica::fiacao::tags(s.doc().unwrap()).is_empty());
}

#[test]
fn quadro_com_montagem_observacoes_e_unifilar_clicando() {
    let _setup = trava_setup();
    let (d, [qd, luz, ..]) = planta();
    let mut s = Session::default();
    s.open_drawing(d, "Quadro", None);
    let q = qd.hex();
    s.execute("elecircuito", &json!({ "quadro": q, "circuito": "1", "pontos": [luz.hex()] })).unwrap();
    let r = s
        .execute("eletrica.quadro", &json!({ "quadro": q, "montagem": "SOBREPOR", "observacoes": "Porta com chave", "modulos": 24, "reservas": 5 }))
        .unwrap();
    assert_eq!(r["config"]["montagem"], "sobrepor");
    assert_eq!(r["config"]["observacoes"], "Porta com chave");
    assert_eq!((r["config"]["modulos"].as_u64(), r["config"]["reservas"].as_u64()), (Some(24), Some(5)));
    for ruim in [
        json!({ "montagem": "X" }),
        json!({ "modulos": -1 }),
        json!({ "reservas": 1000 }),
        json!({ "observacoes": 3 }),
        json!({ "observacoes": "x".repeat(201) }),
    ] {
        assert!(s.execute("eletrica.quadro", &ruim).is_err(), "{ruim}");
    }
    let r = s.execute("eletrica.quadro", &json!({ "quadro": q, "montagem": null, "observacoes": null, "modulos": null, "reservas": null })).unwrap();
    assert!(r["config"]["montagem"].is_null() && r["config"]["reservas"].is_null(), "{r}");
    // O unifilar clicando: um quadro só vai direto ao ponto, e Esc não cria nada.
    let contar = |s: &Session| free_eletrica::unifilar::unifilares(s.doc().unwrap()).len();
    let quadros = s.execute("eletrica.analisar", &json!({})).unwrap();
    let varios = quadros["pontos"].as_array().unwrap().iter().filter(|p| p["tipo"] == "quadro").count() > 1;
    s.start("eleunifilar").unwrap();
    if varios {
        s.input(Input::Enter).unwrap();
    }
    s.input(Input::Cancel).unwrap();
    assert_eq!(contar(&s), 0);
    s.start("eleunifilar").unwrap();
    if varios {
        s.input(Input::Pick(vec![qd])).unwrap();
    }
    s.input(Input::Point(Vec2::new(30.0, 30.0))).unwrap();
    assert!(s.running.is_none());
    assert_eq!(contar(&s), 1);
    let mut vazio = Session::default();
    vazio.open_drawing(Drawing::new_metric(), "Vazio", None);
    vazio.start("eleunifilar").unwrap();
    assert!(vazio.running.is_none(), "sem quadro, avisa e termina");
}

#[test]
fn empilhar_pontos_por_comando() {
    let _setup = trava_setup();
    let mut s = Session::new();
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    s.open_drawing(d, "Pilha", None);
    s.execute("eletrica.bootstrap", &json!({})).unwrap();
    let ponto = |s: &mut Session, b: &str, x: f64, y: f64| -> String {
        s.execute("eleponto", &json!({ "bloco": b, "pontos": [[x, y]], "rotacaoAuto": false })).unwrap()["pontos"][0].as_str().unwrap().to_string()
    };
    let base = ponto(&mut s, "TOM-SIM-BAIXA", 2.0, 2.0);
    // Inserido no mesmo lugar, empilha sozinho.
    let segunda = ponto(&mut s, "TOM-SIM-BAIXA", 2.0, 2.0);
    let host = |s: &mut Session, h: &str| {
        s.execute("eletrica.analisar", &json!({})).unwrap()["pontos"].as_array().unwrap().iter().find(|p| p["handle"] == h).unwrap()["host"].clone()
    };
    assert_eq!(host(&mut s, &segunda), base.as_str());
    // Explícito, por comando, e um passo de desfazer.
    let outro = ponto(&mut s, "INT-SIM", 5.0, 5.0);
    assert!(host(&mut s, &outro).is_null());
    let r = s.execute("eletrica.ponto.empilhar", &json!({ "pontos": [outro], "base": segunda })).unwrap();
    assert_eq!(r["base"], base.as_str(), "a base da pilha");
    assert_eq!(host(&mut s, &outro), base.as_str());
    s.undo().unwrap();
    assert!(host(&mut s, &outro).is_null());
    assert!(s.execute("eletrica.ponto.empilhar", &json!({ "pontos": [outro] })).is_err());
    assert!(s.execute("eletrica.ponto.empilhar", &json!({ "pontos": [outro], "base": outro })).is_err());
}
