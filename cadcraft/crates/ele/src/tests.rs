// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. CRAFT ELE: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
use std::sync::Arc;

use cadcraft_doc::*;
use cadcraft_geom::Vec3;

use crate::nbr5410::{self, Config};
use crate::ponto::{altura_m, fases, numero, potencia_va};
use crate::*;

fn texto(valor: &str) -> Text {
    Text {
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
    }
}

fn atributo(tag: &str, valor: &str, constante: bool) -> Attrib {
    Attrib {
        tag: tag.into(),
        text: texto(valor),
        invisible: false,
        constant: constante,
        prompt: String::new(),
        props: Default::default(),
        multiline: None,
        modes: Default::default(),
    }
}

/// Insere `bloco` em (x, y) com os atributos dados, criando a definição vazia se faltar.
fn inserir(d: &mut Drawing, bloco: &str, x: f64, y: f64, attrs: &[(&str, &str)]) -> Handle {
    d.blocks.entry(bloco.to_string()).or_insert_with(|| Arc::new(Block::new(bloco)));
    let ins = Insert {
        block: bloco.into(),
        insert: Vec3::new(x, y, 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
        rotation: 0.0,
        attribs: attrs.iter().map(|(t, v)| atributo(t, v, false)).collect(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    };
    d.add(&Space::Model, Common::default(), EntityKind::Insert(ins)).unwrap()
}

fn eletroduto(d: &mut Drawing, a: (f64, f64), b: (f64, f64)) {
    let c = Common { layer: "ELETRODUTO".into(), ..Default::default() };
    d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) })).unwrap();
}

fn metros() -> Drawing {
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 6);
    d
}

/// Uma planta em metros com quadro trifásico, dois pontos de luz e um interruptor (circuito 1),
/// duas TUG (circuito 2), um chuveiro (circuito 3), uma caixa de passagem, uma porta e a rede de
/// eletrodutos:
///
/// ```text
///   (0,3) TUG ─┐
///              │
///   QD (0,0) ──┼── (2,0) TUG ── (4,0) luz ── (8,0) luz
///              │                  └ (4,1) interruptor
///   (0,-5) chuveiro
/// ```
fn planta() -> Drawing {
    let mut d = metros();
    #[rustfmt::skip]
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01"), ("TENSAO", "380/220V"), ("NUMERO_FASES", "3"), ("ALTURA", "1,50")]);
    #[rustfmt::skip]
    fn luz(p: &'static str, tag: &'static str) -> Vec<(&'static str, &'static str)> {
        vec![("TIPO_PONTO", "ILUM"), ("POTENCIA", p), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a"), ("TAG", tag)]
    }
    inserir(&mut d, "CAIXA_LUZ", 4.0, 0.0, &luz("100VA", "LUZ-1"));
    inserir(&mut d, "CAIXA_LUZ", 8.0, 0.0, &luz("160VA", "LUZ-2"));
    #[rustfmt::skip]
    let int = [("TIPO_PONTO", "INTERRUPTOR"), ("POTENCIA", "0W"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a"), ("ALTURA", "1,10")];
    inserir(&mut d, "INT_SIMPLES", 4.0, 1.0, &int);
    for (x, y) in [(2.0, 0.0), (0.0, 3.0)] {
        #[rustfmt::skip]
        let tug = [("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "2"), ("ALTURA", "30cm")];
        inserir(&mut d, "TOMADA", x, y, &tug);
    }
    #[rustfmt::skip]
    let chuveiro = [("TIPO_PONTO", "TUE"), ("POTENCIA", "5,4kW"), ("TENSAO", "220V"), ("NUMERO_FASES", "2"), ("CIRCUITO", "3"), ("ALTURA", "2,20")];
    inserir(&mut d, "TUE_CHUVEIRO", 0.0, -5.0, &chuveiro);
    inserir(&mut d, "CAIXA_PASSAGEM", 6.0, 6.0, &[("TIPO_PONTO", "CAIXA DE PASSAGEM")]);
    // Um bloco de arquitetura sem atributos elétricos não entra, mesmo com nome parecido.
    inserir(&mut d, "PORTA_INTERNA", 9.0, 9.0, &[]);
    eletroduto(&mut d, (0.0, 0.0), (4.0, 0.0));
    eletroduto(&mut d, (4.0, 0.0), (8.0, 0.0));
    eletroduto(&mut d, (4.0, 0.0), (4.0, 1.0));
    eletroduto(&mut d, (0.0, 0.0), (0.0, 3.0));
    eletroduto(&mut d, (0.0, 0.0), (0.0, -5.0));
    d
}

fn quadro(d: &Drawing) -> Handle {
    analisar(d).quadros()[0].handle
}

fn perto(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// ------------------------------------------------------------------ leitura

#[test]
fn classifica_pelo_tipo_e_pelo_nome() {
    let c = TipoPonto::classificar;
    assert_eq!(c("TUG"), Some(TipoPonto::Tomada));
    assert_eq!(c("Tomada"), Some(TipoPonto::Tomada));
    assert_eq!(c("TOMADA_TUE"), Some(TipoPonto::TomadaEspecifica));
    assert_eq!(c("ILUM"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("caixa de luz"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("Luminária"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("INT_SIMPLES"), Some(TipoPonto::Interruptor));
    assert_eq!(c("QD-01"), Some(TipoPonto::Quadro));
    assert_eq!(c("CAIXA_PASSAGEM"), Some(TipoPonto::Caixa));
    assert_eq!(c("MESA"), None);
}

#[test]
fn le_numeros_potencias_fases_e_alturas() {
    assert_eq!(numero("220/127V"), Some(220.0));
    assert_eq!(numero("1,5"), Some(1.5));
    assert_eq!(numero("1.500 W"), Some(1500.0));
    assert_eq!(potencia_va("5,4kW"), Some(5400.0));
    assert_eq!(potencia_va("0W"), Some(0.0));
    assert_eq!(fases("TRIFÁSICO"), Some(3));
    assert_eq!(fases("7"), None);
    assert_eq!(altura_m("1,10"), Some(1.1));
    assert_eq!(altura_m("30cm"), Some(0.3));
    assert_eq!(altura_m("300 mm"), Some(0.3));
    // Sem unidade e acima de 10: centímetros.
    assert_eq!(altura_m("30"), Some(0.3));
    assert_eq!(altura_m("2.2 m"), Some(2.2));
}

#[test]
fn le_os_pontos_e_liga_ao_unico_quadro() {
    let d = planta();
    let a = analisar(&d);
    assert_eq!(a.pontos.len(), 8, "a porta não é ponto elétrico");
    let qd = quadro(&d);
    assert!(a.pontos.iter().filter(|p| p.tipo != TipoPonto::Quadro).all(|p| p.quadro == Some(qd)));
    // A luminária tem COMANDO e continua sendo carga.
    let luz = a.pontos.iter().find(|p| p.tag.as_deref() == Some("LUZ-1")).unwrap();
    assert_eq!(luz.tipo, TipoPonto::Iluminacao);
    // Luz sem ALTURA fica no teto sem aviso.
    assert!(luz.avisos().is_empty(), "{:?}", luz.avisos());
    let tug = a.pontos.iter().find(|p| p.tipo == TipoPonto::Tomada).unwrap();
    assert_eq!(tug.altura, Some(0.3));
}

#[test]
fn xdata_grava_le_e_preserva_outros_aplicativos() {
    let mut x = vec![RawTag { code: 1001, value: "OUTRO".into() }, RawTag { code: 1000, value: "nao mexer".into() }];
    xdata::gravar(&mut x, "CIRCUITO", Some("3"));
    xdata::gravar(&mut x, "QUADRO", Some("2A"));
    assert_eq!(xdata::ler(&x), [("CIRCUITO".to_string(), "3".to_string()), ("QUADRO".to_string(), "2A".to_string())]);
    xdata::gravar(&mut x, "circuito", Some("4"));
    assert_eq!(xdata::ler(&x)[0].1, "4");
    xdata::gravar(&mut x, "CIRCUITO", None);
    xdata::gravar(&mut x, "QUADRO", None);
    // Sem chaves, o trecho do CRAFT ELE some e o do outro aplicativo fica.
    assert_eq!(x, [RawTag { code: 1001, value: "OUTRO".into() }, RawTag { code: 1000, value: "nao mexer".into() }]);
}

// ------------------------------------------------------------------ circuitos

#[test]
fn agrupa_os_circuitos_por_quadro() {
    let a = analisar(&planta());
    let ids: Vec<&str> = a.circuitos.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["1", "2", "3"]);
    let c1 = &a.circuitos[0];
    assert_eq!((c1.natureza, c1.potencia, c1.cargas, c1.interruptores), (Natureza::Iluminacao, 260.0, 2, 1));
    let c3 = &a.circuitos[2];
    assert_eq!((c3.natureza, c3.potencia, c3.fases, c3.tensao), (Natureza::Tue, 5400.0, 2, Some(220.0)));

    // Dois quadros: o circuito 1 de cada um é um circuito diferente.
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01")]);
    inserir(&mut d, "QD", 9.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-02")]);
    inserir(&mut d, "TUG", 1.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("CIRCUITO", "1"), ("QUADRO", "QD-01")]);
    inserir(&mut d, "TUG", 8.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "200VA"), ("CIRCUITO", "1"), ("QUADRO", "qd-02")]);
    inserir(&mut d, "TUG", 5.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "300VA"), ("CIRCUITO", "1")]);
    let a = analisar(&d);
    assert_eq!(a.circuitos.len(), 3);
    let potencias: Vec<f64> = a.circuitos.iter().map(|c| c.potencia).collect();
    assert!(potencias.contains(&100.0) && potencias.contains(&200.0) && potencias.contains(&300.0));
    // Sem QUADRO e com dois quadros no desenho, o circuito fica sem quadro e avisa.
    let solto = a.circuitos.iter().find(|c| c.potencia == 300.0).unwrap();
    assert!(solto.quadro.is_none() && solto.avisos.iter().any(|w| w.contains("sem quadro")));
}

#[test]
fn proximo_circuito_e_proxima_letra() {
    let d = planta();
    let a = analisar(&d);
    let qd = quadro(&d);
    assert_eq!(circuito::proximo_circuito(&a.pontos, qd), "4");
    assert_eq!(circuito::proxima_letra(&a.pontos, Some(qd), Some("1")), "b");
    assert_eq!(circuito::proxima_letra(&a.pontos, Some(qd), Some("2")), "a");
}

#[test]
fn criar_circuito_grava_atributo_ou_xdata() {
    let mut d = planta();
    let qd = quadro(&d);
    // Um ponto sem os atributos CIRCUITO e QUADRO: tudo vai para o XDATA.
    let novo = inserir(&mut d, "TOMADA", 0.0, 3.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA")]);
    editar::atribuir_circuito(&mut d, novo, qd, "7").unwrap();
    let e = d.model.get(novo).unwrap();
    assert_eq!(xdata::ler(&e.common.xdata), [("CIRCUITO".to_string(), "7".to_string()), ("QUADRO".to_string(), qd.hex())]);
    let a = analisar(&d);
    let c7 = a.circuitos.iter().find(|c| c.id == "7").unwrap();
    assert_eq!((c7.quadro, c7.potencia), (Some(qd), 100.0));
    // Um ponto com o atributo CIRCUITO: o atributo muda e o XDATA não guarda cópia.
    let tug = a.pontos.iter().find(|p| p.tipo == TipoPonto::Tomada && p.tem_atributo(&["CIRCUITO"])).unwrap().handle;
    editar::atribuir_circuito(&mut d, tug, qd, "5").unwrap();
    let p = ponto::ler_ponto(&d, tug).unwrap();
    assert_eq!(p.circuito.as_deref(), Some("5"));
    assert!(p.extras.iter().all(|(k, _)| k != "CIRCUITO"));
    // Comando.
    editar::definir_comando(&mut d, tug, "c").unwrap();
    assert_eq!(ponto::ler_ponto(&d, tug).unwrap().comando.as_deref(), Some("c"));
    // Não é bloco: erro.
    let linha = d.model.iter().find(|e| matches!(e.kind, EntityKind::Line(_))).unwrap().handle;
    assert!(editar::definir(&mut d, linha, &["CIRCUITO"], Some("1")).is_err());
}

// ------------------------------------------------------------------ NBR 5410

#[test]
fn dimensiona_condutor_e_disjuntor_pela_corrente() {
    let a = analisar(&planta());
    let cfg = Config::default();
    let d: Vec<_> = a.circuitos.iter().map(|c| nbr5410::dimensionar(c, &cfg)).collect();
    assert_eq!((d[0].secao, d[0].disjuntor), (Some(1.5), Some(6.0)));
    assert!(perto(d[1].ib, 9.449, 0.01));
    assert_eq!((d[1].secao, d[1].disjuntor), (Some(2.5), Some(10.0)));
    // Chuveiro: 24,5 A passa dos 24 A do 2,5 mm²: vai a 4 mm² com 25 A.
    assert_eq!((d[2].secao, d[2].disjuntor, d[2].iz), (Some(4.0), Some(25.0), Some(32.0)));
    // Com agrupamento 0,7 o 4 mm² cai para 22,4 A e o circuito sobe para 6 mm².
    let agrupado = nbr5410::dimensionar(&a.circuitos[2], &Config { fca: 0.7, ..cfg });
    assert_eq!((agrupado.secao, agrupado.disjuntor), (Some(6.0), Some(25.0)));
}

#[test]
fn queda_de_tensao_aumenta_a_secao() {
    let a = analisar(&planta());
    let tug = &a.circuitos[1];
    // 9,45 A a uns 40 m: momento 378 A·m. Em 2,5 mm² a queda passa de 4 %; em 4 mm² não.
    assert!(nbr5410::queda_percentual(378.0, 2.5, 127.0, 1, nbr5410::Isolacao::Pvc) > 4.0);
    assert!(nbr5410::queda_percentual(378.0, 4.0, 127.0, 1, nbr5410::Isolacao::Pvc) < 4.0);
    let d = nbr5410::dimensionar_com(tug, &Config::default(), Some(378.0));
    assert_eq!((d.secao, d.disjuntor), (Some(4.0), Some(10.0)));
    assert!(d.queda.unwrap() < 4.0);
}

#[test]
fn tabela_42_de_agrupamento() {
    assert_eq!(nbr5410::fca_agrupamento(1), 1.0);
    assert_eq!(nbr5410::fca_agrupamento(2), 0.8);
    assert_eq!(nbr5410::fca_agrupamento(3), 0.7);
    assert_eq!(nbr5410::fca_agrupamento(10), 0.5);
    assert_eq!(nbr5410::fca_agrupamento(25), 0.38);
}

#[test]
fn verificar_aponta_o_que_a_norma_nao_aceita() {
    let a = analisar(&planta());
    let v = nbr5410::verificar(&a.circuitos[1], &Config::default(), None, 1.5, 32.0);
    assert!(v.avisos.iter().any(|w| w.contains("tabela 47")));
    assert!(v.avisos.iter().any(|w| w.contains("In > Iz")));
}

// ------------------------------------------------------------------ setup

#[test]
fn setup_define_tensao_fases_fct_e_camadas() {
    let padrao = Setup::default();
    assert_eq!(padrao.sistema, Sistema::Trifasico380220);
    let cfg = padrao.config();
    assert_eq!((cfg.tensao_fn, cfg.tensao_ff, cfg.fases_padrao, cfg.queda_admissivel), (220.0, 380.0, 3, 4.0));
    assert_eq!(Setup { temperatura_ambiente: 35.0, ..Setup::default() }.fct(), 0.94);
    assert_eq!(Setup { temperatura_ambiente: 33.0, ..Setup::default() }.fct(), 0.94);
    assert!(padrao.e_camada_eletroduto("ELE-ELETRODUTO-TETO"));
    assert!(padrao.e_camada_eletroduto("eletroduto"));
    assert!(!padrao.e_camada_eletroduto("PAREDES"));
    let varias = Setup { camadas_eletroduto: "TUBO?, *CONDUITE*".into(), ..Setup::default() };
    assert!(varias.e_camada_eletroduto("TUBO1") && varias.e_camada_eletroduto("ELE_CONDUITE") && !varias.e_camada_eletroduto("TUBO12"));
    // Gravado e lido de volta; campo faltando cai no padrão.
    let json = serde_json::to_string(&padrao).unwrap();
    assert_eq!(serde_json::from_str::<Setup>(&json).unwrap(), padrao);
    let antigo: Setup = serde_json::from_str("{\"temperaturaAmbiente\": 40}").unwrap();
    assert_eq!((antigo.temperatura_ambiente, antigo.pe_direito), (40.0, 2.8));
}

#[test]
fn catalogo_de_telecom() {
    let mut s = Setup::default();
    assert_eq!(s.cabos_telecom.len(), 12);
    assert!(s.cabos_telecom.iter().any(|c| c.nome == "Fibra óptica N12" && c.quantidade == 12));
    let novo = CaboTelecom { nome: " UTP Cat 6A ".into(), categoria: CategoriaTelecom::Utp, quantidade: 4, diametro: 7.5, ..CaboTelecom::default() };
    s.cadastrar_cabo(novo.clone()).unwrap();
    assert!(s.cadastrar_cabo(novo).is_err(), "nome repetido");
    assert!(s.cadastrar_cabo(CaboTelecom::default()).is_err(), "sem nome");
    assert_eq!(s.cabos_telecom.last().unwrap().nome, "UTP Cat 6A");
    // Os de fábrica não saem; os cadastrados saem.
    assert!(!s.remover_cabo("UTP Cat 5"));
    assert!(s.remover_cabo("utp cat 6a"));
    // Preferências antigas sem algum cabo de fábrica ganham ele de volta.
    let mut velho = Setup { cabos_telecom: Vec::new(), ..Setup::default() };
    velho.normalizar();
    assert_eq!(velho.cabos_telecom, setup::cabos_padrao());
}

#[test]
fn unidade_do_desenho() {
    assert_eq!(Unidade::Automatica.metros(6).0, 1.0);
    assert_eq!(Unidade::Automatica.metros(4).0, 0.001);
    assert!(Unidade::Automatica.metros(0).1.is_some(), "sem INSUNITS avisa");
    assert_eq!(Unidade::Centimetro.metros(6).0, 0.01);
}

// ------------------------------------------------------------------ roteamento

#[test]
fn rede_de_eletrodutos_liga_os_pontos() {
    let d = planta();
    let a = analisar(&d);
    let rede = rota::Rede::montar(&d, &Setup::default(), &a.pontos, 1.0);
    assert_eq!(rede.trechos, 5);
    // A caixa de passagem longe da rede não liga; todo o resto liga.
    assert_eq!(rede.ligacoes.len(), 7);
    let qd = rede.ligacoes[&quadro(&d)].no;
    let arvore = rede.arvore(qd);
    let luz2 = a.pontos.iter().find(|p| p.tag.as_deref() == Some("LUZ-2")).unwrap();
    assert!(perto(arvore.distancia[rede.ligacoes[&luz2.handle].no], 8.0, 1e-9));
    // A TUG em (2,0) cortou o trecho do quadro à primeira luz.
    assert_eq!(arvore.caminho(rede.ligacoes[&luz2.handle].no).unwrap().len(), 3);
}

#[test]
fn rotear_quadro_calcula_comprimento_queda_e_fca() {
    let d = planta();
    let a = analisar(&d);
    let r = rotear_quadro(&d, &a, &Setup::default(), quadro(&d)).unwrap();
    assert_eq!((r.tag.as_str(), r.fases_quadro, r.trechos_eletroduto), ("QD-01", 3, 5));
    assert_eq!(r.linhas.len(), 3);
    let (c1, c2, c3) = (&r.linhas[0], &r.linhas[1], &r.linhas[2]);
    // Pé-direito 2,80; quadro a 1,50: perna do quadro 1,30 m. Luz no teto; interruptor a 1,10.
    // Circuito 1, até a luz 2: 8 m + 1,30 = 9,30 m.
    assert!(perto(c1.comprimento.unwrap(), 9.3, 1e-9));
    // Momento até a luz 2: 2,047 A × 4 m + 1,26 A × 4 m + 2,047 A × 1,30 m = 15,89 A·m.
    let q1 = nbr5410::queda_percentual(15.89, 1.5, 127.0, 1, nbr5410::Isolacao::Pvc);
    assert!(perto(c1.escolha.queda.unwrap(), q1, 0.001), "{:?} vs {q1}", c1.escolha.queda);
    // Os circuitos 1 e 2 dividem o trecho do quadro até (2,0): FCA 0,80 nos dois.
    assert_eq!((c1.agrupados, c1.fca), (2, 0.8));
    assert_eq!((c2.agrupados, c2.fca), (2, 0.8));
    assert_eq!((c3.agrupados, c3.fca), (1, 1.0));
    // Circuito 2: até a TUG em (0,3): 3 m + 2,50 de descida + 1,30 = 6,80 m.
    assert!(perto(c2.comprimento.unwrap(), 6.8, 1e-9));
    assert_eq!((c2.escolha.secao, c2.escolha.disjuntor), (Some(2.5), Some(10.0)));
    // Chuveiro: 4 mm², 25 A; 5 m + 0,60 + 1,30.
    assert!(perto(c3.comprimento.unwrap(), 6.9, 1e-9));
    assert_eq!((c3.escolha.secao, c3.escolha.disjuntor), (Some(4.0), Some(25.0)));
    assert_eq!(c3.indicacao, "2F+T #4");
    // Fases: chuveiro em A+B, TUG em C, luz em C (a menos carregada).
    assert_eq!((c3.fases_texto().as_str(), c2.fases_texto().as_str(), c1.fases_texto().as_str()), ("A+B", "C", "C"));
    assert_eq!(r.potencia_total, 6860.0);
    // Materiais: 3 condutores por circuito monofásico, fator 1,10.
    let cabo_25 = r.materiais.iter().find(|(s, _)| *s == 2.5).unwrap().1;
    assert!(perto(cabo_25, c2.cabo, 1e-9));
    assert!(r.avisos.is_empty(), "{:?}", r.avisos);
    // JSON para o canal de controle.
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["linhas"][2]["escolha"]["disjuntor"], 25.0);
}

#[test]
fn sobrescrita_manual_vale_e_e_conferida() {
    let mut d = planta();
    let qd = quadro(&d);
    editar::definir_sobrescrita(&mut d, qd, "2", Some(1.5), Some(32.0)).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    let c2 = &r.linhas[1];
    assert_eq!((c2.automatico.secao, c2.escolha.secao, c2.escolha.disjuntor), (Some(2.5), Some(1.5), Some(32.0)));
    assert_eq!(c2.sobrescrita, Sobrescrita { secao: Some(1.5), disjuntor: Some(32.0) });
    assert!(c2.escolha.avisos.len() >= 2, "{:?}", c2.escolha.avisos);
    // Só o disjuntor: a seção continua a automática.
    editar::definir_sobrescrita(&mut d, qd, "2", None, Some(16.0)).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!((r.linhas[1].escolha.secao, r.linhas[1].escolha.disjuntor), (Some(2.5), Some(16.0)));
    // Limpa: volta ao automático e o XDATA do quadro fica vazio.
    editar::definir_sobrescrita(&mut d, qd, "2", None, None).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(r.linhas[1].escolha.disjuntor, Some(10.0));
    assert!(d.model.get(qd).unwrap().common.xdata.is_empty());
}

#[test]
fn secao_omitida_sai_da_indicacao_e_do_resumo() {
    let d = planta();
    let setup = Setup { secoes_omitidas: vec![2.5], ..Setup::default() };
    let r = rotear_quadro(&d, &analisar(&d), &setup, quadro(&d)).unwrap();
    assert_eq!(r.linhas[1].indicacao, "F+N+T");
    assert_eq!(r.linhas[2].indicacao, "2F+T #4");
    let t = quadro::texto(&r, &setup);
    assert!(t.contains("RESUMO DA FIAÇÃO - QD-01"));
    assert!(t.contains("2F+T #4"));
    assert!(!t.contains("#2,5"));
}

#[test]
fn desenho_em_milimetros_da_os_mesmos_comprimentos() {
    // A mesma planta com as coordenadas em mm.
    let mut d = Drawing::new_metric();
    assert_eq!(d.header.i64("INSUNITS", 0), 4);
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("ALTURA", "1,5")]);
    inserir(&mut d, "TUG", 3000.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("CIRCUITO", "1"), ("ALTURA", "0,3")]);
    eletroduto(&mut d, (0.0, 0.0), (3000.0, 0.0));
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    assert_eq!(r.metros_por_unidade, 0.001);
    assert!(perto(r.linhas[0].comprimento.unwrap(), 3.0 + 2.5 + 1.3, 1e-9));
}

fn eletroduto_em(d: &mut Drawing, camada: &str, a: (f64, f64), b: (f64, f64)) {
    let c = Common { layer: camada.into(), ..Default::default() };
    d.add(&Space::Model, c, EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) })).unwrap();
}

#[test]
fn eletroduto_no_piso_muda_as_pernas() {
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("ALTURA", "1,5")]);
    inserir(&mut d, "TUG", 3.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("CIRCUITO", "1"), ("ALTURA", "0,3")]);
    eletroduto_em(&mut d, "ELE-ELETRODUTO-PVC-MEDIA-PISO", (0.0, 0.0), (3.0, 0.0));
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    // No piso: 3 m, a tomada sobe 0,30 e o quadro desce 1,50.
    assert!(perto(r.linhas[0].comprimento.unwrap(), 3.0 + 0.3 + 1.5, 1e-9), "{:?}", r.linhas[0].comprimento);
}

#[test]
fn passagem_do_teto_ao_piso_soma_um_pe_direito() {
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("ALTURA", "1,5")]);
    inserir(&mut d, "TUG", 4.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("CIRCUITO", "1"), ("ALTURA", "0,3")]);
    eletroduto_em(&mut d, "ELE-ELETRODUTO-PVC-MEDIA-TETO", (0.0, 0.0), (2.0, 0.0));
    eletroduto_em(&mut d, "ELE-ELETRODUTO-PVC-MEDIA-PISO", (2.0, 0.0), (4.0, 0.0));
    let a = analisar(&d);
    let rede = rota::Rede::montar(&d, &Setup::default(), &a.pontos, 1.0);
    // Dois trechos e a subida entre eles.
    assert_eq!(rede.arestas.len(), 3);
    let r = rotear_quadro(&d, &a, &Setup::default(), quadro(&d)).unwrap();
    // Quadro no teto (1,30) + 2 m + subida 2,80 + 2 m no piso + tomada 0,30.
    assert!(perto(r.linhas[0].comprimento.unwrap(), 1.3 + 2.0 + 2.8 + 2.0 + 0.3, 1e-9), "{:?}", r.linhas[0].comprimento);
    // Com as camadas de piso desligadas tudo corre no teto.
    let teto = Setup { camadas_piso: String::new(), ..Setup::default() };
    let r = rotear_quadro(&d, &a, &teto, quadro(&d)).unwrap();
    assert!(perto(r.linhas[0].comprimento.unwrap(), 1.3 + 4.0 + 2.5, 1e-9));
}

#[test]
fn sem_eletrodutos_estima_e_avisa() {
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("ALTURA", "1,5")]);
    inserir(&mut d, "TUG", 3.0, 4.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("CIRCUITO", "1"), ("ALTURA", "0,3")]);
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    // Em L: 3 + 4 m, mais as pernas.
    assert!(perto(r.linhas[0].comprimento.unwrap(), 7.0 + 2.5 + 1.3, 1e-9));
    assert_eq!(r.linhas[0].fora_da_rede, 1);
    assert!(r.avisos.iter().any(|w| w.contains("nenhum eletroduto")));
}

#[test]
fn atributo_constante_vem_da_definicao_do_bloco() {
    let mut d = metros();
    let mut def = Block::new("TOMADA_ALTA");
    def.entities.push(Entity::new(Handle(900), EntityKind::AttDef(atributo("TIPO_PONTO", "TUG", true))));
    d.blocks.insert("TOMADA_ALTA".into(), Arc::new(def));
    inserir(&mut d, "TOMADA_ALTA", 0.0, 0.0, &[("POTENCIA", "100VA"), ("CIRCUITO", "4")]);
    let a = analisar(&d);
    assert_eq!(a.pontos[0].tipo, TipoPonto::Tomada);
    assert!(a.pontos[0].tipo_informado);
}

// ------------------------------------------------------------------ fiação

fn trecho(f: &fiacao::Fiacao, a: (f64, f64), b: (f64, f64)) -> &fiacao::Trecho {
    let igual = |p: [f64; 2], q: (f64, f64)| perto(p[0], q.0, 1e-6) && perto(p[1], q.1, 1e-6);
    f.trechos.iter().find(|t| (igual(t.a, a) && igual(t.b, b)) || (igual(t.a, b) && igual(t.b, a))).unwrap_or_else(|| panic!("trecho {a:?}-{b:?}"))
}

fn circ<'a>(t: &'a fiacao::Trecho, id: &str) -> &'a fiacao::Condutores {
    t.circuitos.iter().find(|k| k.circuito == id).unwrap_or_else(|| panic!("circuito {id}"))
}

#[test]
fn fiacao_por_trecho() {
    let d = planta();
    let a = analisar(&d);
    let f = fiacao::calcular(&d, &a, &Setup::default(), quadro(&d)).unwrap();
    assert_eq!(f.trechos.len(), 6, "{:#?}", f.trechos);
    assert!(f.avisos.is_empty(), "{:?}", f.avisos);
    // Do quadro até a TUG em (2,0): a iluminação (fase do interruptor, neutro e proteção das
    // luzes) e a TUG (fase, neutro, proteção).
    let t = trecho(&f, (0.0, 0.0), (2.0, 0.0));
    let c1 = circ(t, "1");
    assert_eq!((c1.fases, c1.neutro, c1.protecao, c1.retornos.len()), (1, true, true, 0));
    let c2 = circ(t, "2");
    assert_eq!((c2.fases, c2.neutro, c2.protecao, c2.secao), (1, true, true, Some(2.5)));
    // Descida ao interruptor: fase e o retorno "a".
    let c1 = circ(trecho(&f, (4.0, 0.0), (4.0, 1.0)), "1");
    assert_eq!((c1.fases, c1.neutro, c1.protecao), (1, false, false));
    assert_eq!(c1.retornos.iter().map(String::as_str).collect::<Vec<_>>(), ["a"]);
    // Até a segunda luz: neutro, proteção e retorno, sem fase.
    let c1 = circ(trecho(&f, (4.0, 0.0), (8.0, 0.0)), "1");
    assert_eq!((c1.fases, c1.neutro, c1.protecao, c1.retornos.len()), (0, true, true, 1));
    // Chuveiro bifásico: duas fases e proteção, sem neutro.
    let c3 = circ(trecho(&f, (0.0, 0.0), (0.0, -5.0)), "3");
    assert_eq!((c3.fases, c3.neutro, c3.protecao), (2, false, true));
    let n = fiacao::contagem(&f);
    assert_eq!((n["FIACAO-FASE"], n["FIACAO-NEUTRO"], n["FIACAO-RETORNO"], n["FIACAO-PROTECAO"]), (7, 5, 2, 6));
}

#[test]
fn ajustar_fiacao_desenha_e_refaz() {
    let mut d = planta();
    let qd = quadro(&d);
    let setup = Setup::default();
    let f = fiacao::calcular(&d, &analisar(&d), &setup, qd).unwrap();
    // 20 marcas; em cima, 7 números de circuito e 2 letras de retorno; embaixo, 7 seções e 6 diâmetros.
    assert_eq!(fiacao::aplicar(&mut d, &f, &setup).unwrap(), 42);
    for tipo in fiacao::TipoCondutor::TODOS {
        assert!(d.blocks.contains_key(tipo.bloco()), "{}", tipo.bloco());
    }
    assert!(d.layer(fiacao::CAMADA).is_some());
    assert_eq!(fiacao::gerados(&d, qd).len(), 42);
    let textos: Vec<String> = d.model.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).collect();
    assert!(
        textos.contains(&"#2,5".to_string())
            && textos.contains(&"#4".to_string())
            && textos.contains(&"a".to_string())
            && textos.iter().filter(|t| *t == "%%c20").count() == 6,
        "{textos:?}"
    );
    // No trecho horizontal (4,0)-(8,0): o diâmetro fica numa linha própria, abaixo das seções.
    let no_trecho: Vec<(String, f64)> = d
        .model
        .iter()
        .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some((t.value.clone(), t.insert.x, t.insert.y)) } else { None })
        .filter(|(_, x, y)| *x > 4.0 && *x < 8.0 && y.abs() < 0.5)
        .map(|(v, _, y)| (v, y))
        .collect();
    let y_diametro = no_trecho.iter().find(|(v, _)| v.starts_with("%%c")).map(|(_, y)| *y).expect("diâmetro no trecho");
    let secoes: Vec<f64> = no_trecho.iter().filter(|(v, _)| v.starts_with('#')).map(|(_, y)| *y).collect();
    assert!(!secoes.is_empty() && secoes.iter().all(|y| y - y_diametro >= 0.05), "{no_trecho:?}");
    // Rodar de novo não duplica: apaga e refaz.
    let f = fiacao::calcular(&d, &analisar(&d), &setup, qd).unwrap();
    assert_eq!(fiacao::aplicar(&mut d, &f, &setup).unwrap(), 42);
    assert_eq!(fiacao::gerados(&d, qd).len(), 42);
    // As marcas não viram pontos elétricos nem eletrodutos.
    assert_eq!(analisar(&d).pontos.len(), 8);
    // Seção omitida sai do rótulo.
    let omite = Setup { secoes_omitidas: vec![2.5], ..Setup::default() };
    fiacao::aplicar(&mut d, &f, &omite).unwrap();
    let textos: Vec<String> = d.model.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).collect();
    assert!(textos.contains(&"2".to_string()) && !textos.iter().any(|t| t.contains("#2,5")), "{textos:?}");
    assert_eq!(fiacao::limpar(&mut d, qd), 40);
    assert!(fiacao::gerados(&d, qd).is_empty());
}

#[test]
fn bloco_de_marca_do_projetista_e_mantido() {
    let mut d = planta();
    let mut proprio = Block::new("FIACAO-FASE");
    proprio.entities.push(Entity::new(Handle(990), EntityKind::Circle(Circle { center: Vec3::ZERO, radius: 0.02 })));
    d.blocks.insert("FIACAO-FASE".into(), Arc::new(proprio));
    fiacao::garantir_blocos(&mut d, 1.0);
    assert_eq!(d.blocks["FIACAO-FASE"].entities.len(), 1);
    assert_eq!(d.blocks["FIACAO-NEUTRO"].entities.len(), 1);
}

#[test]
fn fiacao_sem_eletroduto_no_quadro_e_erro() {
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO")]);
    inserir(&mut d, "TUG", 3.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    assert!(fiacao::calcular(&d, &analisar(&d), &Setup::default(), quadro(&d)).is_err());
}

#[test]
fn setup_global_le_e_grava_por_json() {
    let antes = setup::atual();
    let v = setup::json(&serde_json::json!({ "peDireito": 3.1 })).unwrap();
    assert_eq!(v["peDireito"], 3.1);
    assert_eq!(setup::atual().pe_direito, 3.1);
    assert!(setup::json(&serde_json::json!({ "sistema": "inexistente" })).is_err());
    setup::definir(antes);
}

// ------------------------------------------------------------------ isolação, ocupação, quadro de cargas

#[test]
fn isolacao_muda_capacidade_fct_e_resistividade() {
    use nbr5410::Isolacao;
    // Tabela 37 (EPR/XLPE, B1): 2,5 mm² com dois carregados conduz 31 A; tabela 36 (PVC): 24 A.
    assert_eq!(nbr5410::capacidade(1, 2, Isolacao::Xlpe), Some(31.0));
    assert_eq!(nbr5410::capacidade(1, 2, Isolacao::Pvc), Some(24.0));
    assert_eq!(nbr5410::capacidade(99, 2, Isolacao::Pvc), None);
    // Tabela 40 a 40 °C: PVC 0,87, EPR/XLPE 0,91.
    assert_eq!(Isolacao::Pvc.fct(40.0), 0.87);
    assert_eq!(Isolacao::Epr.fct(40.0), 0.91);
    assert!(nbr5410::queda_percentual(100.0, 2.5, 220.0, 1, Isolacao::Xlpe) > nbr5410::queda_percentual(100.0, 2.5, 220.0, 1, Isolacao::Pvc));
    assert_eq!(Isolacao::de_codigo("xlpe"), Some(Isolacao::Xlpe));
    assert_eq!(Isolacao::de_codigo("borracha"), None);
}

#[test]
fn isolacao_do_quadro_no_roteamento() {
    let mut d = planta();
    let qd = quadro(&d);
    // PVC: o chuveiro (24,5 A) vai a 4 mm². XLPE: 2,5 mm² conduz 31 A e basta, com 25 A.
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!((r.isolacao, r.linhas[2].escolha.secao), (nbr5410::Isolacao::Pvc, Some(4.0)));
    editar::definir_isolacao(&mut d, qd, Some(nbr5410::Isolacao::Xlpe)).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(r.isolacao, nbr5410::Isolacao::Xlpe);
    assert_eq!((r.linhas[2].escolha.secao, r.linhas[2].escolha.disjuntor), (Some(2.5), Some(25.0)));
    editar::definir_isolacao(&mut d, qd, None).unwrap();
    assert_eq!(rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap().isolacao, nbr5410::Isolacao::Pvc);
}

#[test]
fn ocupacao_do_eletroduto() {
    let s = Setup::default();
    assert_eq!(setup::ocupacao_maxima(1), 0.53);
    assert_eq!(setup::ocupacao_maxima(2), 0.31);
    assert_eq!(setup::ocupacao_maxima(7), 0.40);
    assert_eq!(s.diametro_condutor(6.0, nbr5410::Isolacao::Pvc), Some(4.8));
    assert_eq!(s.diametro_condutor(6.0, nbr5410::Isolacao::Epr), Some(7.2));
    assert_eq!(s.diametro_condutor(7.0, nbr5410::Isolacao::Pvc), None);
    // Dez condutores de 6 mm² (4,8 mm): 181 mm². A 40 %, o de 25 (21,3 mm, 142 mm² úteis) não
    // comporta; o de 32 (27,5 mm, 238 mm² úteis) sim.
    let (e, oc) = s.eletroduto_para(&[4.8; 10]).unwrap();
    assert_eq!(e.nominal, 32.0);
    assert!(oc <= 0.40 && oc > 0.30, "{oc}");
    // Poucos condutores: o mínimo do projeto (20).
    assert_eq!(s.eletroduto_para(&[3.0, 3.0, 3.0]).unwrap().0.nominal, 20.0);
    let minimo_25 = Setup { eletroduto_minimo: 25.0, ..Setup::default() };
    assert_eq!(minimo_25.eletroduto_para(&[3.0]).unwrap().0.nominal, 25.0);
    // Nada comporta.
    assert!(s.eletroduto_para(&[20.0; 30]).is_none());
}

#[test]
fn fiacao_dimensiona_o_eletroduto_de_cada_trecho() {
    let d = planta();
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    let t = trecho(&f, (0.0, 0.0), (2.0, 0.0));
    // 3 × 1,5 mm² (3,0 mm) e 3 × 2,5 mm² (3,6 mm): 51,7 mm², cabe no de 20 (84 mm² úteis).
    assert_eq!(t.condutores(), 6);
    assert_eq!(t.eletroduto.as_ref().map(|e| e.nominal), Some(20.0));
    assert!((t.ocupacao.unwrap() - 51.75 / 211.24).abs() < 0.01, "{:?}", t.ocupacao);
    // Sem diâmetro na tabela: avisa e não escolhe.
    let sem = Setup { diametros_condutor: vec![setup::DiametroCondutor { secao: 1.5, pvc: 3.0, epr_xlpe: 5.6 }], ..Setup::default() };
    let f = fiacao::calcular(&d, &analisar(&d), &sem, quadro(&d)).unwrap();
    assert!(trecho(&f, (0.0, 0.0), (2.0, 0.0)).eletroduto.is_none());
    assert!(f.avisos.iter().any(|w| w.contains("diâmetro externo")), "{:?}", f.avisos);
}

#[test]
fn cabo_de_telecom_exige_diametro() {
    let mut s = Setup::default();
    assert!(s.cabos_telecom.iter().all(|c| c.diametro > 0.0));
    let sem = CaboTelecom { nome: "UTP Cat 6A".into(), categoria: CategoriaTelecom::Utp, quantidade: 4, ..CaboTelecom::default() };
    assert!(s.cadastrar_cabo(sem.clone()).unwrap_err().contains("diâmetro"));
    s.cadastrar_cabo(CaboTelecom { diametro: 7.5, ..sem }).unwrap();
    // Preferências antigas (sem diâmetro) recebem o de fábrica.
    let mut velho = Setup::default();
    velho.cabos_telecom.iter_mut().for_each(|c| c.diametro = 0.0);
    velho.normalizar();
    assert!(velho.cabos_telecom.iter().all(|c| c.diametro > 0.0));
}

#[test]
fn quadro_de_cargas_cria_descreve_junta_e_exclui() {
    let mut d = planta();
    let qd = quadro(&d);
    // Criar: um circuito descrito existe sem pontos e conta no próximo número.
    editar::definir_descricao(&mut d, qd, "4", Some("Tomadas da cozinha")).unwrap();
    let a = analisar(&d);
    let lista = quadro::circuitos_do_quadro(&a, qd);
    assert_eq!(lista.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["1", "2", "3", "4"]);
    assert!(lista[3].circuito.is_none() && lista[3].descricao == "Tomadas da cozinha");
    assert_eq!(circuito::proximo_circuito(&a.pontos, qd), "5");
    // Descrição aparece no roteamento.
    editar::definir_descricao(&mut d, qd, "2", Some("TUG sala")).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(r.linhas[1].descricao, "TUG sala");
    // Juntar o 2 no 4: os pontos do 2 passam para o 4; o 2 some com a descrição e as escolhas.
    editar::definir_sobrescrita(&mut d, qd, "2", None, Some(16.0)).unwrap();
    assert_eq!(editar::juntar_circuitos(&mut d, qd, "2", "4").unwrap(), 2);
    let a = analisar(&d);
    let lista = quadro::circuitos_do_quadro(&a, qd);
    assert_eq!(lista.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["1", "3", "4"]);
    assert_eq!(lista[2].circuito.as_ref().map(|c| c.potencia), Some(1200.0));
    assert!(quadro::sobrescritas(a.ponto(qd).unwrap()).is_empty());
    assert!(editar::juntar_circuitos(&mut d, qd, "4", "4").is_err());
    // Excluir o 4: os pontos ficam sem circuito.
    assert_eq!(editar::excluir_circuito(&mut d, qd, "4").unwrap(), 2);
    let a = analisar(&d);
    assert_eq!(quadro::circuitos_do_quadro(&a, qd).len(), 2);
    assert_eq!(a.pontos.iter().filter(|p| p.tipo == TipoPonto::Tomada && p.circuito.is_none()).count(), 2);
}

#[test]
fn alvo_do_proximo_elecircuito() {
    circuito::definir_alvo(Some((Handle(5), "3".into())));
    assert_eq!(circuito::tomar_alvo(), Some((Handle(5), "3".into())));
    assert_eq!(circuito::tomar_alvo(), None);
}
