// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
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
    eletroduto(&mut d, (0.0, 0.0), (2.0, 0.0));
    eletroduto(&mut d, (2.0, 0.0), (4.0, 0.0));
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
    // Sem chaves, o trecho do Free Elétrica some e o do outro aplicativo fica.
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
    // Os de fábrica também saem, e não voltam ao normalizar as preferências.
    assert!(s.remover_cabo("UTP Cat 5"));
    s.normalizar();
    assert!(s.cabos_telecom.iter().all(|c| c.nome != "UTP Cat 5"));
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
    assert_eq!((rede.arestas.len(), rede.soltos.len()), (6, 0));
    // Todo ponto é um nó; a caixa de passagem longe da rede fica sem aresta.
    assert_eq!(rede.nos.len(), 8);
    let caixa = a.pontos.iter().find(|p| p.tipo == TipoPonto::Caixa).unwrap();
    assert!(!rede.conectado(caixa.handle));
    let arvore = rede.arvore(rede.no_de(quadro(&d)).unwrap());
    let luz2 = a.pontos.iter().find(|p| p.tag.as_deref() == Some("LUZ-2")).unwrap();
    // Passa pela TUG (0,30 m) em (2,0) e pela primeira luz.
    let n = rede.no_de(luz2.handle).unwrap();
    assert_eq!(arvore.caminho(n).unwrap().len(), 3);
    assert!(perto(arvore.distancia[n], (2.0 + 1.3 + 2.5) + (2.0 + 2.5) + 4.0, 1e-9));
}

#[test]
fn rotear_quadro_calcula_comprimento_queda_e_fca() {
    let d = planta();
    let a = analisar(&d);
    let r = rotear_quadro(&d, &a, &Setup::default(), quadro(&d)).unwrap();
    assert_eq!((r.tag.as_str(), r.fases_quadro, r.trechos_eletroduto), ("QD-01", 3, 6));
    assert_eq!(r.linhas.len(), 3);
    let (c1, c2, c3) = (&r.linhas[0], &r.linhas[1], &r.linhas[2]);
    // Pé-direito 2,80; quadro a 1,50. Circuito 1, até a luz 2, passando pela TUG (0,30 m) em (2,0):
    // (2 + 1,30 + 2,50) + (2 + 2,50) + 4 = 14,30 m.
    assert!(perto(c1.comprimento.unwrap(), 14.3, 1e-9));
    // Momento até a luz 2: 2,047 A × 10,30 m + 1,26 A × 4 m = 26,12 A·m.
    let q1 = nbr5410::queda_percentual((260.0 / 127.0) * 10.3 + (160.0 / 127.0) * 4.0, 1.5, 127.0, 1, nbr5410::Isolacao::Pvc);
    assert!(perto(c1.escolha.queda.unwrap(), q1, 0.001), "{:?} vs {q1}", c1.escolha.queda);
    // Os circuitos 1 e 2 dividem o trecho do quadro até (2,0): FCA 0,80 nos dois.
    assert_eq!((c1.agrupados, c1.fca), (2, 0.8));
    assert_eq!((c2.agrupados, c2.fca), (2, 0.8));
    assert_eq!((c3.agrupados, c3.fca), (1, 1.0));
    // Circuito 2: até a TUG em (0,3): 3 m + 1,30 do quadro + 2,50 de descida = 6,80 m.
    assert!(perto(c2.comprimento.unwrap(), 6.8, 1e-9));
    assert_eq!((c2.escolha.secao, c2.escolha.disjuntor), (Some(2.5), Some(10.0)));
    // Chuveiro: 4 mm², 25 A; 5 m + 0,60 + 1,30.
    assert!(perto(c3.comprimento.unwrap(), 6.9, 1e-9));
    assert_eq!((c3.escolha.secao, c3.escolha.disjuntor), (Some(4.0), Some(25.0)));
    assert_eq!(c3.indicacao, "2F+T #4");
    // Fases: chuveiro em R+S, TUG em T, luz em T (a menos carregada).
    assert_eq!((c3.fases_texto().as_str(), c2.fases_texto().as_str(), c1.fases_texto().as_str()), ("R+S", "T", "T"));
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
    // A passagem é numa caixa, no teto: sem bloco ali, os trechos não se ligariam.
    inserir(&mut d, "CX", 2.0, 0.0, &[("TIPO_PONTO", "CAIXA DE PASSAGEM")]);
    eletroduto_em(&mut d, "ELE-ELETRODUTO-TETO", (0.0, 0.0), (2.0, 0.0));
    eletroduto_em(&mut d, "ELE-ELETRODUTO-PISO", (2.0, 0.0), (4.0, 0.0));
    let a = analisar(&d);
    let rede = rota::Rede::montar(&d, &Setup::default(), &a.pontos, 1.0);
    assert_eq!(rede.arestas.len(), 2);
    let r = rotear_quadro(&d, &a, &Setup::default(), quadro(&d)).unwrap();
    // Quadro ao teto (1,30) + 2 m + da caixa ao piso (2,80) + 2 m + tomada (0,30).
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
    // Manhattan 3D: 3 + 4 m + |0,30 - 1,50|.
    assert!(perto(r.linhas[0].comprimento.unwrap(), 7.0 + 1.2, 1e-9));
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
    let meio = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    f.trechos.iter().find(|t| perto(t.meio[0], meio.0, 1e-6) && perto(t.meio[1], meio.1, 1e-6)).unwrap_or_else(|| panic!("trecho {a:?}-{b:?}"))
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
    let n = fiacao::contagem(&f.trechos);
    assert_eq!((n["FIACAO-FASE"], n["FIACAO-NEUTRO"], n["FIACAO-RETORNO"], n["FIACAO-PROTECAO"]), (7, 5, 2, 6));
}

#[test]
fn bloco_de_marca_do_projetista_e_mantido() {
    let mut d = planta();
    let mut proprio = Block::new("FIACAO-FASE");
    proprio.entities.push(Entity::new(Handle(990), EntityKind::Circle(Circle { center: Vec3::ZERO, radius: 0.02 })));
    d.blocks.insert("FIACAO-FASE".into(), Arc::new(proprio));
    fiacao::garantir_blocos(&mut d, 1.0);
    assert_eq!(d.blocks["FIACAO-FASE"].entities.len(), 1);
    assert!(matches!(d.blocks["FIACAO-FASE"].entities.iter().next().map(|e| &e.kind), Some(EntityKind::Circle(_))));
    assert_eq!(d.blocks["FIACAO-NEUTRO"].entities.len(), 2);
}

/// As linhas de um bloco de marca, em metros.
fn linhas_do_bloco(d: &Drawing, nome: &str) -> Vec<((f64, f64), (f64, f64))> {
    d.blocks[nome]
        .entities
        .iter()
        .filter_map(|e| if let EntityKind::Line(l) = &e.kind { Some(((l.a.x, l.a.y), (l.b.x, l.b.y))) } else { None })
        .collect()
}

#[test]
fn simbologia_de_fiacao_do_padrao() {
    let mut d = planta();
    fiacao::garantir_blocos(&mut d, 1.0);
    // Fase: só o traço que atravessa o eletroduto.
    assert_eq!(linhas_do_bloco(&d, "FIACAO-FASE"), [((0.0, -0.05), (0.0, 0.05))]);
    // Neutro: o traço com bandeira para a esquerda no topo.
    assert_eq!(linhas_do_bloco(&d, "FIACAO-NEUTRO"), [((0.0, -0.05), (0.0, 0.05)), ((-0.02, 0.05), (0.0, 0.05))]);
    // Retorno: traço curto só acima do eletroduto.
    let r = linhas_do_bloco(&d, "FIACAO-RETORNO");
    assert_eq!(r.len(), 1);
    assert!(r[0].0.1 >= 0.0 && r[0].1.1 > 0.0 && r[0].1.1 < 0.05, "{r:?}");
    // Terra: o T.
    assert_eq!(linhas_do_bloco(&d, "FIACAO-PROTECAO"), [((0.0, -0.05), (0.0, 0.05)), ((-0.02, 0.05), (0.02, 0.05))]);
}

#[test]
fn blocos_de_marca_da_versao_anterior_sao_trocados() {
    let mut d = planta();
    // A fase da versão anterior: traço com bico ("1").
    let mut velho = Block::new("FIACAO-FASE");
    for (a, b) in [((0.0, -0.05), (0.0, 0.05)), ((-0.015, 0.035), (0.0, 0.05))] {
        velho.entities.push(Entity::new(d.new_handle(), EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) })));
    }
    d.blocks.insert("FIACAO-FASE".into(), Arc::new(velho));
    fiacao::garantir_blocos(&mut d, 1.0);
    assert_eq!(linhas_do_bloco(&d, "FIACAO-FASE"), [((0.0, -0.05), (0.0, 0.05))]);
    // Rodar de novo não troca o bloco atual.
    let primeiro = |d: &Drawing| d.blocks["FIACAO-FASE"].entities.iter().next().map(|e| e.handle);
    let antes = primeiro(&d);
    fiacao::garantir_blocos(&mut d, 1.0);
    assert_eq!(primeiro(&d), antes);
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

// ------------------------------------------------------------------ demanda e propriedades do circuito

#[test]
fn tabelas_de_demanda_da_equatorial() {
    use demanda::*;
    let t = tabela("equatorial-nt001-r07");
    assert_eq!(t.id, "equatorial-nt001-r07");
    // Id desconhecido: a primeira tabela.
    assert_eq!(tabela("inexistente").id, t.id);
    // Tabela 5, residências: 100 % até 10 kW, 35 % nos 110 seguintes, 25 % acima de 120.
    let res = &t.edificacao("Residências e edifícios de apartamentos").faixas;
    assert!(perto(por_faixas(res, 8_000.0), 8_000.0, 1e-6));
    assert!(perto(por_faixas(res, 15_000.0), 11_750.0, 1e-6));
    assert!(perto(por_faixas(res, 130_000.0), 10_000.0 + 38_500.0 + 2_500.0, 1e-6));
    // Hotéis: 50 % até 20, 40 % nos 80 seguintes, 30 % acima de 100.
    assert!(perto(por_faixas(&t.edificacao("Hotéis").faixas, 150_000.0), 57_000.0, 1e-6));
    assert!(perto(por_faixas(res, -5.0), 0.0, 1e-9));
    // Tabela 4: colunas até e acima de 3,5 kW.
    assert_eq!((fator_aparelhos(&t.aparelhos, 1, false), fator_aparelhos(&t.aparelhos, 2, true)), (0.80, 0.65));
    assert_eq!(
        (fator_aparelhos(&t.aparelhos, 28, true), fator_aparelhos(&t.aparelhos, 100, false), fator_aparelhos(&t.aparelhos, 100, true)),
        (0.24, 0.30, 0.16)
    );
    // Tabela 10 e bombas.
    assert_eq!(
        (fator_quantidade(&t.ar_condicionado, 2), fator_quantidade(&t.ar_condicionado, 3), fator_quantidade(&t.ar_condicionado, 15)),
        (1.00, 0.88, 0.70)
    );
    assert_eq!((fator_quantidade(&t.bombas, 1), fator_quantidade(&t.bombas, 3)), (1.0, 0.5));
}

#[test]
fn demanda_por_tipo_de_carga() {
    use demanda::*;
    let t = tabela("equatorial-nt001-r07");
    let cargas = [
        Cargas { tipo: TipoCarga::IluminacaoTug, aparelhos: vec![100.0, 160.0, 600.0, 600.0], fator: None },
        Cargas { tipo: TipoCarga::Aquecimento, aparelhos: vec![5_400.0], fator: None },
        Cargas { tipo: TipoCarga::Aquecimento, aparelhos: vec![2_000.0, 2_000.0], fator: None },
        Cargas { tipo: TipoCarga::Reserva, aparelhos: vec![1_000.0], fator: None },
    ];
    let d = calcular(t, "", &cargas);
    assert_eq!(d.edificacao, "Residências e edifícios de apartamentos");
    // Aquecimento: um acima de 3,5 kW (0,80) e dois até 3,5 kW (0,75).
    let aq = d.linhas.iter().find(|l| l.tipo == TipoCarga::Aquecimento).unwrap();
    assert!(perto(aq.demandada, 5_400.0 * 0.80 + 4_000.0 * 0.75, 1e-6), "{aq:?}");
    // A reserva não entra na instalada nem na demandada.
    assert!(perto(d.instalada, 1_460.0 + 9_400.0, 1e-6));
    assert!(perto(d.demandada, 1_460.0 + 7_320.0, 1e-6));
    assert_eq!(TipoCarga::de_codigo("ar"), Some(TipoCarga::ArCondicionado));
}

#[test]
fn presets_de_demanda_conferidos_nas_normas() {
    use demanda::*;
    assert_eq!(tabelas().len(), 3, "só as tabelas conferidas nas normas públicas");
    for t in tabelas() {
        t.validar().unwrap_or_else(|e| panic!("{}: {e}", t.nome));
        assert!(!t.fonte.is_empty() && !t.propria, "{}", t.id);
    }
    // CEMIG, tabela 10: o fator da faixa vale para toda a carga.
    let c = tabela("cemig-nd51");
    assert_eq!(c.id, "cemig-nd51");
    let r = c.edificacao("Residências");
    assert_eq!(r.modo, ModoFaixas::TotalNaFaixa);
    assert!(perto(r.demanda(800.0), 800.0 * 0.86, 1e-6));
    assert!(perto(r.demanda(4_500.0), 4_500.0 * 0.68, 1e-6));
    assert!(perto(r.demanda(9_500.0), 9_500.0 * 0.52, 1e-6));
    assert!(perto(r.demanda(12_000.0), 12_000.0 * 0.45, 1e-6));
    // Tabela 11, acumulada: escritórios 100 % até 20 e 70 % no excesso.
    assert!(perto(c.edificacao("Escritórios, lojas e salas comerciais").demanda(30_000.0), 27_000.0, 1e-6));
    // Tabela 13.
    let n = |q: usize| fator_quantidade(&c.aquecimento, q);
    assert_eq!((n(1), n(2), n(10), n(19), n(20), n(26), n(100)), (1.00, 0.92, 0.52, 0.40, 0.40, 0.37, 0.33));
    assert_eq!(fator_quantidade(&c.ar_condicionado, 5), 0.70);
    // Enel Goiás, tabela 2.
    let e = tabela("enel-go-ntc04-r4");
    let r = e.edificacao("Residências");
    assert!(perto(r.demanda(5_500.0), 5_500.0 * 0.45, 1e-6));
    assert!(perto(r.demanda(15_000.0), 15_000.0 * 0.24, 1e-6));
    assert!(perto(e.edificacao("Hospitais").demanda(30_000.0), 14_000.0 + 4_000.0, 1e-6));
    assert_eq!((fator_quantidade(&e.aquecimento, 2), fator_quantidade(&e.aquecimento, 70)), (0.72, 0.32));
    assert_eq!((fator_quantidade(&e.ar_condicionado, 10), fator_quantidade(&e.ar_condicionado, 11)), (1.00, 0.90));
}

#[test]
fn classe_e_fator_de_demanda_por_ponto_e_familia() {
    use demanda::*;
    let t = tabela("equatorial-nt001-r07");
    // As classes da interface têm nome legível e voltam pelo texto do atributo.
    for c in TipoCarga::CLASSES {
        assert_eq!(TipoCarga::de_texto(c.nome_classe()), Some(c), "{}", c.nome_classe());
        assert_eq!(TipoCarga::de_codigo(c.codigo()), Some(c));
    }
    // Um aparelho com fator próprio sai da contagem: o outro fica com o fator de um aparelho.
    let d = calcular(
        t,
        "",
        &[
            Cargas { tipo: TipoCarga::Aquecimento, aparelhos: vec![2_000.0], fator: None },
            Cargas { tipo: TipoCarga::Aquecimento, aparelhos: vec![2_000.0], fator: Some(0.5) },
            Cargas { tipo: TipoCarga::FatorManual, aparelhos: vec![1_000.0], fator: Some(0.25) },
            Cargas { tipo: TipoCarga::FatorManual, aparelhos: vec![400.0], fator: None },
            Cargas { tipo: TipoCarga::Motores, aparelhos: vec![1_000.0, 1_000.0], fator: None },
            Cargas { tipo: TipoCarga::Reserva, aparelhos: vec![1_000.0], fator: Some(0.5) },
        ],
    );
    let linha = |tipo| d.linhas.iter().find(|l| l.tipo == tipo).unwrap();
    assert!(perto(linha(TipoCarga::Aquecimento).demandada, 2_000.0 * 0.80 + 1_000.0, 1e-6));
    assert_eq!(linha(TipoCarga::Aquecimento).aparelhos, 2);
    assert!(linha(TipoCarga::Aquecimento).regra.contains("fator próprio"));
    assert!(perto(linha(TipoCarga::FatorManual).demandada, 250.0 + 400.0, 1e-6));
    assert!(perto(linha(TipoCarga::Motores).demandada, 2_000.0, 1e-6), "sem tabela de motores: 100 %");
    assert!(perto(linha(TipoCarga::Reserva).demandada, 0.0, 1e-9));
    // No bloco: o atributo do ponto vale sobre a família.
    assert_eq!(ponto::fator_de_demanda("0,70"), Some(0.70));
    assert_eq!(ponto::fator_de_demanda("70%"), Some(0.70));
    assert_eq!(ponto::fator_de_demanda("0"), Some(0.0));
    assert_eq!(ponto::fator_de_demanda("-1"), None);
    assert_eq!(ponto::fator_de_demanda("abc"), None);
    let mut doc = metros();
    let mut f = familia::Familia::da_categoria(familia::FamilyCategory::PontoForca);
    f.classe_demanda = Some(TipoCarga::Motores);
    f.fator_demanda = Some(0.6);
    familia::gravar(&mut doc, "MOTOR", Some(&f)).unwrap();
    assert_eq!(familia::ler(&doc, "MOTOR"), Some(f.clone()));
    let h = inserir(&mut doc, "MOTOR", 1.0, 0.0, &[("POTENCIA", "1000VA")]);
    let p = ponto::ler_ponto(&doc, h).unwrap();
    assert_eq!((p.tipo_carga, p.fator_demanda), (Some(TipoCarga::Motores), Some(0.6)));
    editar::definir(&mut doc, h, ponto::TAGS_CARGA, Some("AR")).unwrap();
    editar::definir(&mut doc, h, ponto::TAGS_FATOR_DEMANDA, Some("0,9")).unwrap();
    let p = ponto::ler_ponto(&doc, h).unwrap();
    assert_eq!((p.tipo_carga, p.fator_demanda), (Some(TipoCarga::ArCondicionado), Some(0.9)));
    // Família com fator fora da faixa é recusada.
    f.fator_demanda = Some(1.5);
    assert!(familia::gravar(&mut doc, "MOTOR", Some(&f)).is_err());
    // A hachura não é oferecida na família: preencher é com a Hachura nativa.
    assert!(
        biblioteca::GEOMETRIAS
            .iter()
            .all(|(forma, nome)| !forma.contains("Media") && !forma.contains("Alta") && !nome.to_lowercase().contains("hachura"))
    );
}

#[test]
fn tabela_de_demanda_propria() {
    use demanda::*;
    let mut s = Setup::default();
    // Inválidas: sem nome, fator fora de 0 a 1, faixas fora de ordem, quantidades fora de ordem.
    assert!(s.salvar_tabela(TabelaDemanda::em_branco("  ")).is_err());
    let mut ruim = TabelaDemanda::em_branco("CPFL");
    ruim.edificacoes[0].faixas = vec![Faixa { ate_kw: Some(10.0), fator: 1.5 }];
    assert!(s.salvar_tabela(ruim).is_err());
    let mut ruim = TabelaDemanda::em_branco("CPFL");
    ruim.edificacoes[0].faixas = vec![Faixa { ate_kw: Some(10.0), fator: 1.0 }, Faixa { ate_kw: Some(5.0), fator: 0.5 }];
    assert!(s.salvar_tabela(ruim).is_err());
    let mut ruim = TabelaDemanda::em_branco("CPFL");
    ruim.ar_condicionado = vec![PorQuantidade { ate: 5, fator: 1.0 }, PorQuantidade { ate: 2, fator: 0.5 }];
    assert!(s.salvar_tabela(ruim).is_err());
    // As de fábrica não mudam.
    let mut copia = tabela("cemig-nd51").clone();
    assert!(s.salvar_tabela(copia.clone()).is_err());
    copia.id.clear();
    copia.nome = "CEMIG revisada".into();
    let id = s.salvar_tabela(copia).unwrap();
    assert_eq!(id, "propria-cemig-revisada");
    // A própria é escolhida e vale no cálculo; é editável com o mesmo id.
    let mut t = TabelaDemanda::em_branco("Minha CPFL");
    t.edificacoes[0].faixas = vec![Faixa { ate_kw: Some(10.0), fator: 0.8 }, Faixa { ate_kw: None, fator: 0.5 }];
    t.motores = vec![PorQuantidade { ate: 1, fator: 1.0 }, PorQuantidade { ate: u32::MAX, fator: 0.7 }];
    let id = s.salvar_tabela(t).unwrap();
    s.concessionaria = id.clone();
    s.normalizar();
    assert_eq!(s.concessionaria, id);
    assert_eq!(s.tabela_demanda().nome, "Minha CPFL");
    let d = calcular(
        s.tabela_demanda(),
        &s.edificacao,
        &[
            Cargas { tipo: TipoCarga::IluminacaoTug, aparelhos: vec![12_000.0], fator: None },
            Cargas { tipo: TipoCarga::Motores, aparelhos: vec![1_000.0, 1_000.0], fator: None },
        ],
    );
    assert!(perto(d.demandada, 8_000.0 + 1_000.0 + 1_400.0, 1e-6), "{d:?}");
    assert_eq!(s.tabelas_demanda().len(), demanda::tabelas().len() + 2);
    // Viaja nas preferências.
    let volta: Setup = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(volta.tabelas_proprias, s.tabelas_proprias);
    // Um arquivo editado à mão com uma tabela ruim não chega ao cálculo.
    let mut hostil = s.clone();
    hostil.tabelas_proprias[1].edificacoes[0].faixas[0].fator = f64::NAN;
    hostil.normalizar();
    assert_eq!(hostil.tabelas_proprias.len(), 1);
    assert_eq!(hostil.concessionaria, "equatorial-nt001-r07");
    // Excluir a tabela em uso volta para a primeira de fábrica.
    assert!(s.remover_tabela(&id));
    assert!(!s.remover_tabela(&id));
    assert_eq!(s.concessionaria, "equatorial-nt001-r07");
}

#[test]
fn sistema_tensao_cabo_e_alimentacao_por_quadro() {
    use nbr5410::TipoCabo as C;
    use quadro::{SistemaQuadro as S, TensaoNominal as V};
    let setup = Setup::default();
    let mut d = metros();
    let q = inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01")]);
    let q2 = inserir(&mut d, "QD", 5.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-02")]);
    let q3 = inserir(&mut d, "QD", 9.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-03")]);
    // O ponto diz 220 V, do padrão de fábrica.
    let t = inserir(&mut d, "TUG", 2.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "1270VA"), ("TENSAO", "220V")]);
    editar::atribuir_circuito(&mut d, t, q, "1").unwrap();
    let rotear = |d: &Drawing| quadro::rotear(d, &analisar(d), &setup, q).unwrap();
    // Sem escolha: os padrões do programa continuam valendo para desenhos antigos.
    let r = rotear(&d);
    assert_eq!((r.fases_quadro, r.tensao_fn, r.tensao_ff, r.cabo_padrao), (3, 220.0, 380.0, setup.cabo_padrao));
    assert_eq!(r.linhas[0].tensao, 220.0);
    // O quadro manda: monofásico 127/220 V, e o circuito fica em 127 V.
    editar::definir_sistema_quadro(&mut d, q, Some(S::Monofasico)).unwrap();
    editar::definir_tensao_nominal(&mut d, q, Some(V::V127x220)).unwrap();
    editar::definir_cabo_quadro(&mut d, q, Some(C::Lszh70)).unwrap();
    let r = rotear(&d);
    assert_eq!((r.fases_quadro, r.tensao_fn, r.tensao_ff, r.cabo_padrao), (1, 127.0, 220.0, C::Lszh70));
    assert_eq!((r.linhas[0].tensao, r.linhas[0].tipo_cabo), (127.0, C::Lszh70));
    assert!(perto(r.linhas[0].escolha.ib, 10.0, 1e-9), "{}", r.linhas[0].escolha.ib);
    assert_eq!(r.alimentacao(), "127 V monofásico");
    assert_eq!(r.config.sistema, Some(S::Monofasico));
    // A tensão do circuito vale sobre a do quadro; o cabo do circuito, sobre o padrão do quadro.
    editar::definir_tensao_circuito(&mut d, q, "1", Some(220.0)).unwrap();
    editar::definir_cabo_circuito(&mut d, q, "1", Some(C::HeprXlpe90)).unwrap();
    let r = rotear(&d);
    assert_eq!((r.linhas[0].tensao, r.linhas[0].tipo_cabo), (220.0, C::HeprXlpe90));
    editar::definir_tensao_circuito(&mut d, q, "1", None).unwrap();
    editar::definir_cabo_circuito(&mut d, q, "1", None).unwrap();
    // 2F+T não tem neutro: o circuito com neutro é avisado.
    editar::definir_sistema_quadro(&mut d, q, Some(S::BifasicoSemNeutro)).unwrap();
    let r = rotear(&d);
    assert_eq!(r.fases_quadro, 2);
    assert!(r.avisos.iter().any(|a| a.contains("sem neutro")), "{:?}", r.avisos);
    // Voltar ao que o bloco diz.
    editar::definir_tensao_nominal(&mut d, q, None).unwrap();
    assert!(quadro::config_do_quadro(&ponto::ler_ponto(&d, q).unwrap()).tensao.is_none());
    // Alimentado por: a concessionária ou outro quadro, sem ciclo.
    editar::definir_alimentacao(&mut d, q, Some("concessionaria")).unwrap();
    assert_eq!(rotear(&d).config.alimentado_por.as_deref(), Some(quadro::CONCESSIONARIA));
    editar::definir_alimentacao(&mut d, q2, Some("QD-01")).unwrap();
    editar::definir_alimentacao(&mut d, q3, Some(&q2.hex())).unwrap();
    assert_eq!(quadro::quadro_que_alimenta(&ponto::ler_ponto(&d, q3).unwrap()), Some(q2));
    assert!(editar::definir_alimentacao(&mut d, q, Some("QD-01")).is_err(), "a si mesmo");
    assert!(editar::definir_alimentacao(&mut d, q, Some("QD-03")).is_err(), "ciclo");
    assert!(editar::definir_alimentacao(&mut d, q, Some("não existe")).is_err());
    editar::definir_alimentacao(&mut d, q3, None).unwrap();
    assert_eq!(quadro::quadro_que_alimenta(&ponto::ler_ponto(&d, q3).unwrap()), None);
    // O código da tensão aceita o V no fim; lixo não vira nada.
    assert_eq!(V::de_codigo(" 220/380V "), Some(V::V220x380));
    assert_eq!(V::de_codigo("13800"), None);
    assert_eq!(S::de_codigo("3f+n+t"), Some(S::Trifasico));
}

#[test]
fn tipo_de_carga_e_fp_vem_dos_atributos() {
    use demanda::TipoCarga;
    use ponto::fator_potencia;
    // TIPO_CARGA: códigos e nomes comuns.
    assert_eq!(TipoCarga::de_texto("ilum_tug"), Some(TipoCarga::IluminacaoTug));
    assert_eq!(TipoCarga::de_texto("Chuveiro elétrico"), Some(TipoCarga::Aquecimento));
    assert_eq!(TipoCarga::de_texto("Split 9000 BTU"), Some(TipoCarga::ArCondicionado));
    assert_eq!(TipoCarga::de_texto("ar-condicionado"), Some(TipoCarga::ArCondicionado));
    assert_eq!(TipoCarga::de_texto("Geladeira"), Some(TipoCarga::Eletrodomesticos));
    assert_eq!(TipoCarga::de_texto("xyz"), None);
    // FP: decimal ou porcentagem; "1,5" é erro (não 1,5 %).
    assert_eq!(fator_potencia("0,92"), Some(0.92));
    assert_eq!(fator_potencia("92%"), Some(0.92));
    assert_eq!(fator_potencia("85"), Some(0.85));
    assert_eq!(fator_potencia("1"), Some(1.0));
    assert_eq!(fator_potencia("1,5"), None);
    assert_eq!(fator_potencia("0"), None);
}

#[test]
fn quadro_de_cargas_por_circuito_isolacao_carga_e_fp() {
    use nbr5410::Isolacao;
    let mut d = planta();
    let qd = quadro(&d);
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    // Sem atributos: o chuveiro (TUE) é aquecimento com FP 1; a TUG, iluminação e TUG com 0,92.
    assert_eq!((r.linhas[2].tipos_carga.as_slice(), r.linhas[2].fp), (&[demanda::TipoCarga::Aquecimento][..], 1.0));
    assert_eq!((r.linhas[1].tipos_carga.as_slice(), r.linhas[1].fp), (&[demanda::TipoCarga::IluminacaoTug][..], 0.92));
    assert!(perto(r.demanda.demandada, 1_460.0 + 5_400.0 * 0.80, 1e-6), "{:?}", r.demanda);
    assert!(perto(r.corrente_demandada, r.demanda.demandada / (3f64.sqrt() * r.tensao_ff), 1e-9));
    for l in &r.linhas {
        assert!(perto(l.carga_fase.iter().sum::<f64>(), l.circuito.potencia, 1e-9));
    }
    // Isolação só do chuveiro: XLPE leva a 2,5 mm²; a TUG continua em PVC.
    editar::definir_isolacao_circuito(&mut d, qd, "3", Some(Isolacao::Xlpe)).unwrap();
    // Uma das TUGs vira reserva com FP 80 % pelos atributos do bloco; a outra recebe um FP inválido.
    let a = analisar(&d);
    let tugs: Vec<Handle> = a.pontos.iter().filter(|p| p.tipo == TipoPonto::Tomada).map(|p| p.handle).collect();
    editar::definir(&mut d, tugs[0], ponto::TAGS_CARGA, Some("RESERVA")).unwrap();
    editar::definir(&mut d, tugs[0], ponto::TAGS_FP, Some("80%")).unwrap();
    editar::definir(&mut d, tugs[1], ponto::TAGS_FP, Some("1,5")).unwrap();
    let a = analisar(&d);
    let avisos: Vec<String> = a.avisos_pontos().into_iter().map(|(_, w)| w).collect();
    assert!(avisos.iter().any(|w| w.contains("FP \"1,5\" inválido")), "{avisos:?}");
    let r = rotear_quadro(&d, &a, &Setup::default(), qd).unwrap();
    assert_eq!((r.linhas[2].isolacao, r.linhas[2].isolacao_propria, r.linhas[2].escolha.secao), (Isolacao::Xlpe, true, Some(2.5)));
    assert_eq!((r.linhas[1].isolacao, r.linhas[1].isolacao_propria), (Isolacao::Pvc, false));
    // O circuito 2 mistura reserva e TUG: W = 600 × 0,80 + 600 × 0,92; FP = W / VA.
    assert_eq!(r.linhas[1].tipos_carga, [demanda::TipoCarga::Reserva, demanda::TipoCarga::IluminacaoTug]);
    assert!(perto(r.linhas[1].potencia_w, 1_032.0, 1e-9));
    assert!(perto(r.linhas[1].fp, 0.86, 1e-9));
    // A reserva sai da demanda: iluminação e a outra TUG (860 VA) e o chuveiro (5400 × 0,80).
    assert!(perto(r.demanda.demandada, 860.0 + 4_320.0, 1e-6), "{:?}", r.demanda);
    // A fiação usa a isolação de cada circuito: 2,5 mm² em XLPE tem 6,0 mm.
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(circ(trecho(&f, (0.0, 0.0), (0.0, -5.0)), "3").isolacao, Isolacao::Xlpe);
    // Excluir o circuito apaga as propriedades dele.
    editar::excluir_circuito(&mut d, qd, "2").unwrap();
    assert!(!quadro::propriedades(analisar(&d).ponto(qd).unwrap()).contains_key("2"));
}

#[test]
fn tensoes_do_quadro_pelo_atributo() {
    let mut d = metros();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TENSAO", "220/127V"), ("NUMERO_FASES", "2")]);
    // TUG sem TENSAO: usa a fase-neutro do quadro.
    inserir(&mut d, "TUG", 3.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "1270VA"), ("CIRCUITO", "1")]);
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    assert_eq!((r.tensao_fn, r.tensao_ff), (127.0, 220.0));
    assert_eq!(r.linhas[0].tensao, 127.0);
    assert!(perto(r.linhas[0].escolha.ib, 10.0, 1e-9));
    // Bifásico: corrente por fase = S / (2 × fase-neutro).
    assert!(perto(r.corrente_instalada, 1270.0 / 254.0, 1e-9));
}

// ------------------------------------------------------------------ eletroduto em arco e biblioteca de pontos

#[test]
fn arco_do_eletroduto() {
    use eletroduto::pontos_do_arco;
    let (a, b) = ([0.0, 0.0], [4.0, 0.0]);
    let p = pontos_do_arco(a, b, 0.15, 5);
    assert_eq!(p.len(), 5);
    assert!(perto(p[0][0], 0.0, 1e-9) && perto(p[0][1], 0.0, 1e-9) && perto(p[4][0], 4.0, 1e-9) && perto(p[4][1], 0.0, 1e-9), "{p:?}");
    // O meio fica a 15 % da corda (0,6 m) à esquerda de quem vai de A para B (y positivo).
    assert!(perto(p[2][0], 2.0, 1e-9) && perto(p[2][1], 0.6, 1e-9), "{p:?}");
    // Todos numa circunferência: raio (c²/4 + h²) / 2h, centro abaixo da corda.
    let r = (4.0 + 0.36) / 1.2;
    let centro = [2.0, 0.6 - r];
    assert!(p.iter().all(|q| perto((q[0] - centro[0]).hypot(q[1] - centro[1]), r, 1e-9)), "{p:?}");
    // Flecha negativa: para a direita. Zero: reta. Acima de meia corda: limitada.
    assert!(perto(pontos_do_arco(a, b, -0.15, 5)[2][1], -0.6, 1e-9));
    assert!(pontos_do_arco(a, b, 0.0, 5).iter().all(|q| q[1] == 0.0));
    assert!(perto(pontos_do_arco(a, b, 3.0, 5)[2][1], 2.0, 1e-9));
    assert!(pontos_do_arco(a, b, f64::NAN, 3).iter().all(|q| q[1] == 0.0));
}

#[test]
fn eletroduto_em_arco_entra_no_roteamento_e_na_fiacao() {
    let mut d = metros();
    let qd = inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("ALTURA", "1,50")]);
    let tug = inserir(&mut d, "TUG", 4.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("CIRCUITO", "1"), ("ALTURA", "0,30")]);
    let h = eletroduto::inserir(&mut d, qd, tug, rota::Instalacao::Teto, 0.15).unwrap();
    assert!(matches!(&d.model.get(h).unwrap().kind, EntityKind::Spline(_)));
    assert_eq!(d.model.get(h).unwrap().common.layer, eletroduto::CAMADA_TETO);
    // Comprimento: o arco (2φR = 4,237 m) mais a descida do quadro (1,30) e da tomada (2,50).
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    let raio: f64 = (4.0 + 0.36) / 1.2;
    let arco = 2.0 * (2.0 / raio).asin() * raio;
    let c = r.linhas[0].comprimento.unwrap();
    assert!(perto(c, arco + 1.3 + 2.5, 0.01), "{c} vs {}", arco + 3.8);
    assert_eq!(r.linhas[0].fora_da_rede, 0);
    // A curva é um trecho só, e a tag fica no meio dela.
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(f.trechos.len(), 1, "{:?}", f.trechos);
    let t = &f.trechos[0];
    assert!(perto(t.meio[0], 2.0, 0.01) && perto(t.meio[1], 0.6, 0.01) && perto(t.angulo, 0.0, 0.05), "{t:?}");
    // Com flecha zero o eletroduto é uma linha.
    let mut d2 = metros();
    let a = inserir(&mut d2, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO")]);
    let b = inserir(&mut d2, "TUG", 4.0, 0.0, &[("TIPO_PONTO", "TUG")]);
    let h = eletroduto::inserir(&mut d2, a, b, rota::Instalacao::Piso, 0.0).unwrap();
    assert!(matches!(&d2.model.get(h).unwrap().kind, EntityKind::Line(_)));
    assert_eq!(d2.model.get(h).unwrap().common.layer, eletroduto::CAMADA_PISO);
}

/// Um bloco de tomada com atributos na definição (TIPO_PONTO constante).
fn bloco_tomada(d: &mut Drawing) {
    let mut b = Block::new("TOMADA-X");
    for (tag, valor, constante) in
        [("TIPO_PONTO", "TUG", true), ("POTENCIA", "100VA", false), ("CIRCUITO", "", false), ("TIPO_CARGA", "ILUM_TUG", false)]
    {
        b.entities.push(Entity::new(d.new_handle(), EntityKind::AttDef(atributo(tag, valor, constante))));
    }
    d.blocks.insert("TOMADA-X".into(), Arc::new(b));
}

#[test]
fn biblioteca_lista_insere_e_exclui_pontos() {
    let mut d = metros();
    bloco_tomada(&mut d);
    // Um bloco sem atributos elétricos não é ponto.
    d.blocks.insert("PORTA".into(), Arc::new(Block::new("PORTA")));
    let lista = biblioteca::blocos(&d);
    assert_eq!(lista.len(), 1);
    assert_eq!((lista[0].nome.as_str(), lista[0].tipo, lista[0].inseridos), ("TOMADA-X", TipoPonto::Tomada, 0));
    assert!(lista[0].atributos.iter().any(|a| a.tag == "TIPO_PONTO" && a.constante));
    // Inserir com valores por cima dos padrões.
    let valores = [("POTENCIA".to_string(), "600VA".to_string()), ("CIRCUITO".to_string(), "3".to_string())];
    let h = biblioteca::inserir(&mut d, "tomada-x", [2.0, 1.0], 90.0, &valores).unwrap();
    let p = ponto::ler_ponto(&d, h).unwrap();
    assert_eq!((p.tipo, p.potencia, p.circuito.as_deref(), p.posicao), (TipoPonto::Tomada, Some(600.0), Some("3"), [2.0, 1.0]));
    assert_eq!(p.tipo_carga, Some(demanda::TipoCarga::IluminacaoTug));
    assert_eq!(d.model.get(h).unwrap().common.layer, biblioteca::CAMADA_PONTOS);
    assert_eq!(biblioteca::blocos(&d)[0].inseridos, 1);
    // Atributo que o bloco não tem, ou constante: erro, nada é inserido.
    assert!(biblioteca::inserir(&mut d, "TOMADA-X", [0.0, 0.0], 0.0, &[("COR".into(), "azul".into())]).is_err());
    assert!(biblioteca::inserir(&mut d, "TOMADA-X", [0.0, 0.0], 0.0, &[("TIPO_PONTO".into(), "TUE".into())]).is_err());
    assert!(biblioteca::inserir(&mut d, "NENHUM", [0.0, 0.0], 0.0, &[]).is_err());
    assert!(biblioteca::inserir(&mut d, "TOMADA-X", [f64::NAN, 0.0], 0.0, &[]).is_err());
    assert_eq!(biblioteca::blocos(&d)[0].inseridos, 1);
    // Excluir: só pontos elétricos; um objeto que não é ponto recusa tudo.
    let h2 = biblioteca::inserir(&mut d, "TOMADA-X", [5.0, 1.0], 0.0, &[]).unwrap();
    let linha = d.add(&Space::Model, Common::default(), EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(1.0, 0.0, 0.0) })).unwrap();
    assert!(biblioteca::excluir(&mut d, &[h, linha]).is_err());
    assert_eq!(biblioteca::blocos(&d)[0].inseridos, 2);
    assert_eq!(biblioteca::excluir(&mut d, &[h, h2]).unwrap(), 2);
    assert_eq!(biblioteca::blocos(&d)[0].inseridos, 0);
}

#[test]
fn xdata_do_nome_antigo_continua_lido() {
    // Desenho feito quando o projeto se chamava CRAFT ELE.
    let mut x = vec![RawTag { code: 1001, value: "CRAFTELE".into() }, RawTag { code: 1000, value: "CIRCUITO=3".into() }];
    assert_eq!(xdata::ler(&x), [("CIRCUITO".to_string(), "3".to_string())]);
    // A próxima gravação passa o trecho para o nome novo, sem duplicar.
    xdata::gravar(&mut x, "COMANDO", Some("a"));
    assert_eq!(x.iter().filter(|t| t.code == 1001).map(|t| t.value.as_str()).collect::<Vec<_>>(), [xdata::APP]);
    assert_eq!(xdata::ler(&x).len(), 2);
}

#[test]
fn setup_normaliza_a_concessionaria() {
    let mut s = Setup { concessionaria: "nenhuma".into(), edificacao: "hotéis".into(), ..Setup::default() };
    s.normalizar();
    assert_eq!((s.concessionaria.as_str(), s.edificacao.as_str()), ("equatorial-nt001-r07", "Hotéis"));
}

// ------------------------------------------------------------------ versão 2.0

/// Uma tomada em (x, y).
fn tug(d: &mut Drawing, x: f64, y: f64, circuito: &str) -> Handle {
    inserir(d, "TOMADA", x, y, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("CIRCUITO", circuito), ("ALTURA", "0,30")])
}

fn qd(d: &mut Drawing, x: f64, y: f64) -> Handle {
    inserir(d, "QD", x, y, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01"), ("ALTURA", "1,50")])
}

fn caixa(d: &mut Drawing, x: f64, y: f64) -> Handle {
    inserir(d, "CX", x, y, &[("TIPO_PONTO", "CAIXA DE PASSAGEM")])
}

fn rede_de(d: &Drawing) -> rota::Rede {
    rota::Rede::montar(d, &Setup::default(), &analisar(d).pontos, 1.0)
}

#[test]
fn grafo_estrito_cruzamento_em_x_nao_liga() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let t = tug(&mut d, 4.0, 0.0, "1");
    let (a, b) = (caixa(&mut d, 2.0, -2.0), caixa(&mut d, 2.0, 2.0));
    eletroduto(&mut d, (0.0, 0.0), (4.0, 0.0));
    // Uma linha cruza em X a primeira, ligando duas caixas, sem bloco no cruzamento.
    eletroduto(&mut d, (2.0, -2.0), (2.0, 2.0));
    let rede = rede_de(&d);
    assert_eq!(rede.arestas.len(), 2);
    let arvore = rede.arvore(rede.no_de(q).unwrap());
    assert!(arvore.distancia[rede.no_de(t).unwrap()].is_finite());
    assert!(arvore.caminho(rede.no_de(a).unwrap()).is_none(), "o X não pode ligar a caixa ao quadro");
    assert!(arvore.caminho(rede.no_de(b).unwrap()).is_none());
}

#[test]
fn grafo_estrito_t_e_bloco_no_meio_nao_ligam() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let longe = tug(&mut d, 6.0, 0.0, "1");
    // Em T: a ponta encosta no meio do outro eletroduto.
    let t = tug(&mut d, 3.0, 2.0, "1");
    // Um bloco em cima do meio do eletroduto.
    let no_meio = tug(&mut d, 1.5, 0.0, "1");
    eletroduto(&mut d, (0.0, 0.0), (6.0, 0.0));
    eletroduto(&mut d, (3.0, 0.0), (3.0, 2.0));
    let rede = rede_de(&d);
    let arvore = rede.arvore(rede.no_de(q).unwrap());
    assert!(arvore.caminho(rede.no_de(longe).unwrap()).is_some());
    assert!(arvore.caminho(rede.no_de(t).unwrap()).is_none(), "T sem caixa não liga");
    assert!(arvore.caminho(rede.no_de(no_meio).unwrap()).is_none(), "bloco no meio do trecho não liga");
    // O ramal em T tem uma ponta sem bloco: fica solto e é apontado.
    assert_eq!(rede.soltos.len(), 1);
    assert!(rede.soltos[0].motivo.contains("uma das pontas"), "{:?}", rede.soltos);
}

#[test]
fn grafo_estrito_pontas_sem_bloco_nao_se_ligam() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let t = tug(&mut d, 4.0, 0.0, "1");
    // Duas linhas com pontas coincidentes em (2,0), sem bloco ali.
    eletroduto(&mut d, (0.0, 0.0), (2.0, 0.0));
    eletroduto(&mut d, (2.0, 0.0), (4.0, 0.0));
    let rede = rede_de(&d);
    assert_eq!((rede.arestas.len(), rede.soltos.len()), (0, 2));
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), q).unwrap();
    // Sem caminho: estimado por Manhattan 3D (4 m + |0,30 - 1,50|).
    assert!(perto(r.linhas[0].comprimento.unwrap(), 5.2, 1e-9));
    assert_eq!(r.linhas[0].fora_da_rede, 1);
    assert!(r.avisos.iter().any(|w| w.contains("sem bloco nas duas pontas")), "{:?}", r.avisos);
    // Com uma caixa em (2,0), ligam; e passa a valer o comprimento real.
    caixa(&mut d, 2.0, 0.0);
    let rede = rede_de(&d);
    assert_eq!((rede.arestas.len(), rede.soltos.len()), (2, 0));
    assert!(rede.arvore(rede.no_de(q).unwrap()).caminho(rede.no_de(t).unwrap()).is_some());
}

#[test]
fn eletroduto_grava_as_pontas_e_elas_valem_entre_blocos_vizinhos() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    // Duas tomadas a 10 cm uma da outra, dentro do raio de captura da ponta.
    let t1 = tug(&mut d, 4.0, 0.0, "1");
    let t2 = tug(&mut d, 4.1, 0.0, "2");
    let h = eletroduto::inserir(&mut d, q, t2, rota::Instalacao::Teto, 0.0).unwrap();
    let xd = xdata::ler(&d.model.get(h).unwrap().common.xdata);
    assert!(xd.contains(&(rota::CHAVE_DE.to_string(), q.hex())) && xd.contains(&(rota::CHAVE_PARA.to_string(), t2.hex())), "{xd:?}");
    let rede = rede_de(&d);
    let a = &rede.arestas[0];
    assert_eq!((rede.nos[a.de].handle, rede.nos[a.para].handle), (q, t2));
    let _ = t1;
    // A cópia do eletroduto noutro lugar: o gravado não encosta, então vale a geometria.
    let copia = Common { layer: eletroduto::CAMADA_TETO.into(), xdata: d.model.get(h).unwrap().common.xdata.clone(), ..Common::default() };
    d.add(&Space::Model, copia, EntityKind::Line(Line { a: Vec3::new(20.0, 0.0, 0.0), b: Vec3::new(30.0, 0.0, 0.0) })).unwrap();
    let rede = rede_de(&d);
    assert_eq!((rede.arestas.len(), rede.soltos.len()), (1, 1));
    // Bloco que não é elétrico, ou o mesmo nas duas pontas: erro.
    assert!(eletroduto::inserir(&mut d, q, q, rota::Instalacao::Teto, 0.15).is_err());
    assert!(eletroduto::inserir(&mut d, q, Handle(0xFFFF), rota::Instalacao::Teto, 0.15).is_err());
}

#[test]
fn comprimento_3d_de_cada_instalacao() {
    for (instalacao, esperado) in [
        // 2D + |cota - Z do quadro| + |cota - Z da tomada|; pé-direito 2,80.
        (rota::Instalacao::Teto, 4.0 + 1.3 + 2.5),
        (rota::Instalacao::Piso, 4.0 + 1.5 + 0.3),
        // Na parede a cota é a do bloco A.
        (rota::Instalacao::Parede, 4.0 + 0.0 + 1.2),
    ] {
        let mut d = metros();
        let q = qd(&mut d, 0.0, 0.0);
        let t = tug(&mut d, 4.0, 0.0, "1");
        eletroduto::inserir(&mut d, q, t, instalacao, 0.0).unwrap();
        let rede = rede_de(&d);
        assert_eq!(rede.arestas[0].instalacao, instalacao);
        assert!(perto(rede.arestas[0].comprimento_2d, 4.0, 1e-9));
        assert!(perto(rede.arestas[0].comprimento, esperado, 1e-9), "{instalacao:?}: {}", rede.arestas[0].comprimento);
        let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), q).unwrap();
        assert!(perto(r.linhas[0].comprimento.unwrap(), esperado, 1e-9));
    }
}

#[test]
fn clicar_no_meio_divide_e_insere_caixa() {
    let mut d = metros();
    let setup = Setup::default();
    let q = qd(&mut d, 0.0, 0.0);
    let t = tug(&mut d, 6.0, 0.0, "1");
    let h = eletroduto::inserir(&mut d, q, t, rota::Instalacao::Teto, 0.0).unwrap();
    // Longe de qualquer eletroduto, nada; perto do meio, o eletroduto.
    assert!(eletroduto::eletroduto_em(&d, &setup, [3.0, 5.0], 1.0).is_none());
    let (achado, p) = eletroduto::eletroduto_em(&d, &setup, [3.0, 0.1], 1.0).unwrap();
    assert_eq!(achado, h);
    assert!(perto(p[0], 3.0, 1e-9) && perto(p[1], 0.0, 1e-9));
    assert!(eletroduto::dividir(&mut d, &setup, h, [0.0, 0.0], 1.0).is_err(), "na ponta não divide");
    let cx = eletroduto::dividir(&mut d, &setup, h, p, 1.0).unwrap();
    let a = analisar(&d);
    let ponto_cx = a.ponto(cx).unwrap();
    assert_eq!((ponto_cx.tipo, ponto_cx.posicao), (TipoPonto::Caixa, [3.0, 0.0]));
    let rede = rede_de(&d);
    assert_eq!((rede.arestas.len(), rede.soltos.len()), (2, 0));
    let arvore = rede.arvore(rede.no_de(q).unwrap());
    assert_eq!(arvore.caminho(rede.no_de(t).unwrap()).unwrap().len(), 2);
    assert!(perto(arvore.distancia[rede.no_de(cx).unwrap()], 3.0 + 1.3, 1e-9));
    // A derivação em T agora liga pela caixa.
    let ramal = tug(&mut d, 3.0, 2.0, "1");
    eletroduto::inserir(&mut d, cx, ramal, rota::Instalacao::Teto, 0.0).unwrap();
    let rede = rede_de(&d);
    assert!(rede.arvore(rede.no_de(q).unwrap()).caminho(rede.no_de(ramal).unwrap()).is_some());
    // Uma curva também divide.
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let t = tug(&mut d, 6.0, 0.0, "1");
    let h = eletroduto::inserir(&mut d, q, t, rota::Instalacao::Piso, 0.15).unwrap();
    let cx = eletroduto::dividir(&mut d, &setup, h, [3.0, 0.9], 1.0).unwrap();
    assert_eq!(ponto::ler_ponto(&d, cx).unwrap().altura, Some(0.0), "a caixa fica na cota do eletroduto");
    assert_eq!(rede_de(&d).arestas.len(), 2);
    assert!(rede_de(&d).arestas.iter().all(|a| a.instalacao == rota::Instalacao::Piso));
}

#[test]
fn bootstrap_poe_camadas_e_blocos_uma_vez_e_respeita_os_do_projetista() {
    let setup = Setup::default();
    let mut d = metros();
    let mut proprio = Block::new("QDG");
    proprio.entities.push(Entity::new(Handle(991), EntityKind::AttDef(atributo("TIPO_PONTO", "QUADRO", true))));
    d.blocks.insert("QDG".into(), Arc::new(proprio));
    assert!(biblioteca::falta_base(&d));
    assert!(biblioteca::garantir_base(&mut d, &setup, 1.0));
    assert!(!biblioteca::falta_base(&d));
    assert!(!biblioteca::garantir_base(&mut d, &setup, 1.0), "na segunda vez não muda nada");
    assert_eq!(d.blocks["QDG"].entities.len(), 1, "o bloco do projetista fica");
    for c in ["ELE-PONTOS", "ELE-QUADROS", "ELE-FIACAO", "ELE-ELETRODUTO-TETO", "ELE-ELETRODUTO-PISO"] {
        assert!(d.layer(c).is_some(), "{c}");
    }
    // Todos os tipos da barra de inserção existem, e cada um é um ponto do tipo certo.
    let blocos = biblioteca::blocos(&d);
    let tipo = |n: &str| blocos.iter().find(|b| b.nome == n).map(|b| b.tipo);
    assert_eq!(tipo("TOM-SIM-BAIXA"), Some(TipoPonto::Tomada));
    assert_eq!(tipo("TOM-TUE"), Some(TipoPonto::TomadaEspecifica));
    assert_eq!(tipo("ILU-OCT"), Some(TipoPonto::Iluminacao));
    assert_eq!(tipo("CX-PASSAGEM"), Some(TipoPonto::Caixa));
    assert_eq!(tipo("QDC"), Some(TipoPonto::Quadro));
    for (n, esperado) in [
        ("INT-SIM", ponto::TipoInterruptor::Simples1),
        ("INT-2S", ponto::TipoInterruptor::Simples2),
        ("INT-3S", ponto::TipoInterruptor::Simples3),
        ("INT-PAR", ponto::TipoInterruptor::Paralelo),
        ("INT-INT", ponto::TipoInterruptor::Intermediario),
    ] {
        let h = biblioteca::inserir(&mut d, n, [1.0, 1.0], 0.0, &[]).unwrap();
        let p = ponto::ler_ponto(&d, h).unwrap();
        assert_eq!((p.tipo, p.tipo_interruptor), (TipoPonto::Interruptor, Some(esperado)), "{n}");
    }
    // Em milímetros, o símbolo sai 1000 vezes maior: tem o mesmo tamanho no mundo.
    let mut mm = Drawing::new_metric();
    biblioteca::garantir_base(&mut mm, &setup, 0.001);
    let raio = mm.blocks["INT-SIM"].entities.iter().find_map(|e| if let EntityKind::Circle(c) = &e.kind { Some(c.radius) } else { None }).unwrap();
    assert!(perto(raio, 75.0, 1e-9));
}

#[test]
fn rotacao_alinha_a_parede_mais_proxima() {
    let setup = Setup::default();
    let mut d = metros();
    // Uma parede vertical em x = 2.
    d.add(
        &Space::Model,
        Common { layer: "PAREDES".into(), ..Common::default() },
        EntityKind::Line(Line { a: Vec3::new(2.0, 0.0, 0.0), b: Vec3::new(2.0, 5.0, 0.0) }),
    )
    .unwrap();
    // Um eletroduto não é parede.
    eletroduto(&mut d, (0.0, 0.0), (0.0, 5.0));
    assert!(perto(biblioteca::alinhar_a_parede(&d, &setup, [2.1, 2.0], 1.0).unwrap(), 90.0, 1e-9));
    assert!(biblioteca::alinhar_a_parede(&d, &setup, [5.0, 2.0], 1.0).is_none(), "longe da parede");
    assert!(biblioteca::alinhar_a_parede(&d, &setup, [0.1, 2.0], 1.0).is_none(), "o eletroduto não alinha");
}

#[test]
fn esquema_tensao_e_fases_travadas_do_circuito() {
    let mut d = planta();
    let qd = quadro(&d);
    // F+N: sem proteção, um condutor a menos no eletroduto e na indicação.
    editar::definir_esquema(&mut d, qd, "2", Some(nbr5410::Esquema::Fn)).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!((r.linhas[1].esquema, r.linhas[1].condutores), (nbr5410::Esquema::Fn, 2));
    assert_eq!(r.linhas[1].indicacao, "F+N #2,5");
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    let c2 = circ(trecho(&f, (0.0, 0.0), (0.0, 3.0)), "2");
    assert_eq!((c2.fases, c2.neutro, c2.protecao), (1, true, false));
    // A tensão do circuito vale sobre a dos pontos: 1200 VA a 220 V = 5,45 A.
    editar::definir_tensao_circuito(&mut d, qd, "2", Some(220.0)).unwrap();
    assert!(editar::definir_tensao_circuito(&mut d, qd, "2", Some(-1.0)).is_err());
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert!(perto(r.linhas[1].escolha.ib, 1200.0 / 220.0, 1e-9));
    assert_eq!(r.linhas[1].tensao, 220.0);
    // Fases: sem trava o chuveiro vai para R+S e a TUG para T; travada, a TUG fica em R.
    assert_eq!(r.linhas[1].fases_texto(), "T");
    editar::travar_fases(&mut d, qd, "2", Some("r")).unwrap();
    assert!(editar::travar_fases(&mut d, qd, "2", Some("X")).is_err());
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!((r.linhas[1].fases_texto().as_str(), r.linhas[1].fases_travadas), ("R", true));
    assert_eq!(r.carga_por_fase.iter().sum::<f64>(), r.potencia_total);
    // Excluir o circuito apaga o que foi escolhido para ele.
    editar::excluir_circuito(&mut d, qd, "2").unwrap();
    assert!(!quadro::propriedades(analisar(&d).ponto(qd).unwrap()).contains_key("2"));
}

#[test]
fn corrente_corrigida_e_alimentador() {
    let d = planta();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), quadro(&d)).unwrap();
    // I'b = Ib / (FCT × FCA): a TUG tem FCA 0,80.
    let c2 = &r.linhas[1];
    assert!(perto(c2.escolha.ib_corrigida, c2.escolha.ib / 0.8, 1e-9));
    // Alimentador pela corrente demandada: 5580 VA trifásico a 380 V = 8,5 A... em 2,5 mm².
    let a = &r.alimentador;
    assert!(perto(a.ib, r.corrente_demandada, 1e-12));
    assert_eq!((a.secao, a.disjuntor, a.dr), (Some(2.5), Some(10.0), Some(25.0)));
    assert!(perto(a.dps_uc, 1.1 * 220.0, 1e-9));
    // Corrente acima da tabela: sem seção, sem disjuntor e sem DR.
    let cfg = nbr5410::Config { isolacao: nbr5410::Isolacao::Pvc, ..Config::default() };
    let grande = quadro::alimentador(5000.0, 3, 220.0, &cfg);
    assert_eq!((grande.secao, grande.disjuntor, grande.dr), (None, None, None));
}

#[test]
fn interruptor_paralelo_leva_retornos_paralelos() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let par = [
        ("TIPO_PONTO", "INTERRUPTOR"),
        ("TIPO_INTERRUPTOR", "PARALELO"),
        ("POTENCIA", "0W"),
        ("TENSAO", "127V"),
        ("CIRCUITO", "1"),
        ("COMANDO", "a"),
        ("ALTURA", "1,10"),
    ];
    let s1 = inserir(&mut d, "INT", 2.0, 0.0, &par);
    let s2 = inserir(&mut d, "INT", 4.0, 0.0, &par);
    let luz =
        inserir(&mut d, "LUZ", 6.0, 0.0, &[("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("TENSAO", "127V"), ("CIRCUITO", "1"), ("COMANDO", "a")]);
    for (a, b) in [(q, s1), (s1, s2), (s2, luz)] {
        eletroduto::inserir(&mut d, a, b, rota::Instalacao::Teto, 0.0).unwrap();
    }
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), q).unwrap();
    // Do quadro ao primeiro: a fase, e o neutro e a proteção que seguem até a luz.
    let c = circ(trecho(&f, (0.0, 0.0), (2.0, 0.0)), "1");
    assert_eq!((c.fases, c.neutro, c.protecao, c.retornos.len()), (1, true, true, 0));
    // Entre os dois: os dois retornos paralelos, sem fase.
    let c = circ(trecho(&f, (2.0, 0.0), (4.0, 0.0)), "1");
    assert_eq!(c.fases, 0);
    assert_eq!(c.retornos.iter().map(String::as_str).collect::<Vec<_>>(), ["a\"", "a'"]);
    // Do último à luz: o retorno comum.
    let c = circ(trecho(&f, (4.0, 0.0), (6.0, 0.0)), "1");
    assert_eq!((c.fases, c.retornos.iter().map(String::as_str).collect::<Vec<_>>()), (0, vec!["a"]));
    assert!(f.avisos.is_empty(), "{:?}", f.avisos);
    assert_eq!(ponto::TipoInterruptor::de_texto("intermediário"), Some(ponto::TipoInterruptor::Intermediario));
    assert_eq!(ponto::TipoInterruptor::Simples3.teclas(), 3);
}

/// A tag do eletroduto e a inserção dela.
fn tag_de(d: &Drawing, eletroduto: Handle) -> (Handle, Insert) {
    let (t, _) = fiacao::tags(d).into_iter().find(|(_, p)| *p == eletroduto).expect("tag do eletroduto");
    let EntityKind::Insert(i) = &d.model.get(t).unwrap().kind else { panic!("a tag é um bloco") };
    (t, i.clone())
}

/// Os textos do bloco de uma tag.
fn textos_da_tag(d: &Drawing, i: &Insert) -> Vec<String> {
    d.blocks[&i.block].entities.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).collect()
}

/// Os handles de todos os eletrodutos da planta.
fn eletrodutos(d: &Drawing) -> Vec<Handle> {
    rede_de(d).arestas.iter().map(|a| a.eletroduto).collect()
}

#[test]
fn tags_de_fiacao_guardam_a_posicao_e_acompanham_o_projeto() {
    let mut d = planta();
    let setup = Setup::default();
    let qd = quadro(&d);
    let todos = eletrodutos(&d);
    let r = fiacao::regenerar(&mut d, &setup, Some(&todos)).unwrap();
    assert_eq!((r.criadas, r.tags), (6, 0));
    // Rodar de novo sem pedir tags novas só atualiza: nada duplica.
    let r = fiacao::regenerar(&mut d, &setup, None).unwrap();
    assert_eq!((r.criadas, r.tags), (0, 6));
    assert_eq!(fiacao::tags(&d).len(), 6);
    for tipo in fiacao::TipoCondutor::TODOS {
        assert!(d.blocks.contains_key(tipo.bloco()), "{}", tipo.bloco());
    }
    assert_eq!(analisar(&d).pontos.len(), 8, "as tags não viram pontos elétricos");

    // A tag do trecho do chuveiro: no meio dele, com circuito, seção e diâmetro.
    let rede = rede_de(&d);
    let aresta = rede.arestas.iter().find(|a| perto(a.meio[1], -2.5, 1e-9)).unwrap().eletroduto;
    let (tag, ins) = tag_de(&d, aresta);
    assert!(perto(ins.insert.x, 0.0, 1e-9) && perto(ins.insert.y, -2.5, 1e-9), "{ins:?}");
    assert!(perto(ins.rotation, std::f64::consts::FRAC_PI_2, 1e-9), "trecho vertical lê de baixo para cima: {}", ins.rotation);
    let textos = textos_da_tag(&d, &ins);
    assert!(textos.contains(&"3".to_string()) && textos.contains(&"#4".to_string()) && textos.contains(&"%%c20".to_string()), "{textos:?}");
    // O eletroduto guarda as pontas, a tag, o diâmetro e os fios.
    let xd = xdata::ler(&d.model.get(aresta).unwrap().common.xdata);
    assert!(xd.contains(&(rota::CHAVE_TAG.to_string(), tag.hex())), "{xd:?}");
    assert!(xd.iter().any(|(k, v)| k == "passing_wires.0" && v == "3:F,F,PE:4"), "{xd:?}");
    assert!(xd.iter().any(|(k, v)| k == rota::CHAVE_DIAMETRO && v == "20"), "{xd:?}");

    // O projetista move a tag; a seção do circuito muda; regenerar não mexe na posição.
    let mut e = (**d.model.get(tag).unwrap()).clone();
    if let EntityKind::Insert(i) = &mut e.kind {
        i.insert = Vec3::new(1.0, -2.5, 0.0);
    }
    d.model.replace(e);
    editar::definir_sobrescrita(&mut d, qd, "3", Some(6.0), None).unwrap();
    assert!(fiacao::regenerar_se_houver(&mut d, &setup).unwrap().is_some());
    let (t2, ins) = tag_de(&d, aresta);
    assert_eq!(t2, tag, "a mesma tag");
    assert!(perto(ins.insert.x, 1.0, 1e-9) && perto(ins.insert.y, -2.5, 1e-9), "a tag ficou onde o projetista pôs: {ins:?}");
    let textos = textos_da_tag(&d, &ins);
    assert!(textos.contains(&"#6".to_string()) && !textos.contains(&"#4".to_string()), "{textos:?}");
    // Afastada, a tag ganha a linha de chamada até o meio do eletroduto.
    let chamada =
        d.blocks[&ins.block].entities.iter().find_map(|e| if let EntityKind::Line(l) = &e.kind { Some(l.clone()) } else { None }).expect("chamada");
    // Girada 90°: o ponto (-1, 0) do mundo, em relação à tag, é (0, 1) no bloco.
    assert!(perto(chamada.b.x, 0.0, 1e-9) && perto(chamada.b.y, 1.0, 1e-9), "{chamada:?}");

    // O eletroduto apagado leva a tag junto.
    d.model.remove(aresta);
    let r = fiacao::regenerar(&mut d, &setup, None).unwrap();
    assert_eq!(r.apagadas, 1);
    assert_eq!(fiacao::tags(&d).len(), 5);
    assert!(!d.blocks.contains_key(&format!("{}{}", fiacao::PREFIXO_TAG, aresta.hex())));
    assert_eq!(fiacao::limpar(&mut d, None), 5);
    assert!(fiacao::tags(&d).is_empty());
    assert!(fiacao::regenerar_se_houver(&mut d, &setup).unwrap().is_none(), "sem tags nem tabelas, nada a fazer");
}

#[test]
fn tag_sem_dados_mostra_o_aviso_e_marcas_antigas_somem() {
    let mut d = metros();
    let setup = Setup::default();
    let q = qd(&mut d, 0.0, 0.0);
    let c = caixa(&mut d, 3.0, 0.0);
    let h = eletroduto::inserir(&mut d, q, c, rota::Instalacao::Teto, 0.0).unwrap();
    // Uma marca solta da versão 1.
    let mut x = Vec::new();
    xdata::gravar(&mut x, "FIACAO", Some(&q.hex()));
    let antiga = d
        .add(&Space::Model, Common { xdata: x, ..Common::default() }, EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(1.0, 0.0, 0.0) }))
        .unwrap();
    let r = fiacao::regenerar(&mut d, &setup, Some(&[h])).unwrap();
    assert_eq!((r.criadas, r.apagadas), (1, 1));
    assert!(d.model.get(antiga).is_none());
    let (_, ins) = tag_de(&d, h);
    let textos = textos_da_tag(&d, &ins);
    assert!(textos.contains(&"?".to_string()) && textos.contains(&"%%c?".to_string()), "{textos:?}");
}

#[test]
fn textos_da_tag_nao_se_sobrepoem() {
    let mut d = planta();
    let setup = Setup::default();
    let todos = eletrodutos(&d);
    fiacao::regenerar(&mut d, &setup, Some(&todos)).unwrap();
    // O primeiro trecho leva dois circuitos.
    let rede = rede_de(&d);
    let a = rede.arestas.iter().find(|a| perto(a.meio[0], 1.0, 1e-9) && perto(a.meio[1], 0.0, 1e-9)).unwrap().eletroduto;
    let (_, ins) = tag_de(&d, a);
    let textos: Vec<Text> =
        d.blocks[&ins.block].entities.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.clone()) } else { None }).collect();
    let (base, topo) = (|t: &Text| t.insert.y - t.height / 2.0, |t: &Text| t.insert.y + t.height / 2.0);
    let diametro = textos.iter().find(|t| t.value.starts_with("%%c")).unwrap();
    let secoes: Vec<&Text> = textos.iter().filter(|t| t.value.starts_with('#')).collect();
    assert_eq!(secoes.len(), 2);
    assert!(textos.iter().filter(|t| t.insert.y > 0.0).all(|t| base(t) > 0.05), "circuito encosta nas marcas");
    assert!(secoes.iter().all(|t| topo(t) < -0.05 && topo(diametro) < base(t) && diametro.height < t.height));
    // Textos na mesma linha não se sobrepõem (circuitos vizinhos).
    for (i, a) in textos.iter().enumerate() {
        for b in textos.iter().skip(i + 1).filter(|b| (b.insert.y - a.insert.y).abs() < 1e-9) {
            let folga =
                (a.insert.x - b.insert.x).abs() - (fiacao::largura_texto(&a.value, a.height) + fiacao::largura_texto(&b.value, b.height)) / 2.0;
            assert!(folga >= 0.0, "\"{}\" e \"{}\" se sobrepõem", a.value, b.value);
        }
    }
}

#[test]
fn diametro_travado_a_mao_vale_e_e_conferido() {
    let mut d = planta();
    let setup = Setup::default();
    let qd = quadro(&d);
    let rede = rede_de(&d);
    let primeiro = rede.arestas.iter().find(|a| perto(a.meio[0], 1.0, 1e-9)).unwrap().eletroduto;
    eletroduto::travar_diametro(&mut d, primeiro, Some(32.0)).unwrap();
    assert!(eletroduto::travar_diametro(&mut d, primeiro, Some(-1.0)).is_err());
    let f = fiacao::calcular(&d, &analisar(&d), &setup, qd).unwrap();
    let t = trecho(&f, (0.0, 0.0), (2.0, 0.0));
    assert_eq!((t.eletroduto.as_ref().map(|e| e.nominal), t.diametro_travado), (Some(32.0), true));
    // Regenerar não troca o diâmetro travado.
    fiacao::regenerar(&mut d, &setup, Some(&[primeiro])).unwrap();
    assert_eq!(fiacao::diametro_travado(&d, primeiro), Some(32.0));
    // Travado pequeno demais: passa do limite e avisa.
    eletroduto::travar_diametro(&mut d, primeiro, Some(16.0)).unwrap();
    let f = fiacao::calcular(&d, &analisar(&d), &setup, qd).unwrap();
    assert!(trecho(&f, (0.0, 0.0), (2.0, 0.0)).ocupacao_excedida());
    assert!(f.avisos.iter().any(|w| w.contains("travado")), "{:?}", f.avisos);
    // Destravado: volta a ser automático.
    eletroduto::travar_diametro(&mut d, primeiro, None).unwrap();
    assert_eq!(fiacao::diametro_travado(&d, primeiro), None);
    // A instalação muda com a camada.
    eletroduto::definir_instalacao(&mut d, primeiro, rota::Instalacao::Parede).unwrap();
    assert_eq!(d.model.get(primeiro).unwrap().common.layer, eletroduto::CAMADA_PAREDE);
    assert_eq!(rede_de(&d).arestas.iter().find(|a| a.eletroduto == primeiro).unwrap().instalacao, rota::Instalacao::Parede);
}

#[test]
fn tabela_do_quadro_no_desenho_se_refaz_no_mesmo_lugar() {
    let mut d = planta();
    let setup = Setup::default();
    let qd = quadro(&d);
    let a = analisar(&d);
    let h = tabela::inserir(&mut d, &a, &setup, qd, [20.0, 10.0]).unwrap();
    assert_eq!(tabela::tabelas(&d), [(h, qd)]);
    let textos = |d: &Drawing| -> Vec<String> {
        d.blocks[&format!("{}{}", tabela::PREFIXO, qd.hex())]
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect()
    };
    let t = textos(&d);
    assert!(t.contains(&"PAINEL: QD-01".to_string()) && t.contains(&"CIRCUITO".to_string()) && t.contains(&"FASE T (VA)".to_string()), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("ALIMENTADOR:")) && t.iter().any(|x| x.starts_with("DEMANDA:")), "{t:?}");
    assert_eq!(analisar(&d).pontos.len(), 8, "a tabela não é ponto elétrico");
    // O projetista move a tabela e muda uma descrição: ela se refaz sem sair do lugar.
    let mut e = (**d.model.get(h).unwrap()).clone();
    if let EntityKind::Insert(i) = &mut e.kind {
        i.insert = Vec3::new(30.0, 15.0, 0.0);
    }
    d.model.replace(e);
    editar::definir_descricao(&mut d, qd, "2", Some("Tomadas da sala")).unwrap();
    let r = fiacao::regenerar_se_houver(&mut d, &setup).unwrap().unwrap();
    assert_eq!(r.tabelas, 1);
    assert!(textos(&d).contains(&"Tomadas da sala".to_string()));
    let EntityKind::Insert(i) = &d.model.get(h).unwrap().kind else { panic!() };
    assert_eq!((i.insert.x, i.insert.y), (30.0, 15.0));
    // Inserir de novo só move: um quadro, uma tabela.
    let a = analisar(&d);
    assert_eq!(tabela::inserir(&mut d, &a, &setup, qd, [1.0, 1.0]).unwrap(), h);
    assert_eq!(tabela::tabelas(&d).len(), 1);
    assert!(tabela::inserir(&mut d, &a, &setup, Handle(0xFFFF), [1.0, 1.0]).is_err());
    assert!(tabela::inserir(&mut d, &a, &setup, qd, [f64::NAN, 1.0]).is_err());
    // O quadro apagado leva a tabela junto.
    d.model.remove(qd);
    let a = analisar(&d);
    tabela::regenerar(&mut d, &a, &setup).unwrap();
    assert!(tabela::tabelas(&d).is_empty());
}

#[test]
fn inspecao_classifica_pontos_e_eletrodutos() {
    use inspecao::{Nivel, inspecionar};
    let setup = Setup::default();
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let ok = tug(&mut d, 3.0, 0.0, "1");
    let sem_circuito = inserir(&mut d, "TOMADA", 3.0, 5.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA")]);
    let sem_caminho = tug(&mut d, 9.0, 9.0, "1");
    let no_meio = tug(&mut d, 1.5, 0.0, "1");
    let interruptor = inserir(&mut d, "INT", 5.0, 0.0, &[("TIPO_PONTO", "INTERRUPTOR"), ("POTENCIA", "0W"), ("CIRCUITO", "1"), ("COMANDO", "z")]);
    eletroduto::inserir(&mut d, q, ok, rota::Instalacao::Teto, 0.0).unwrap();
    eletroduto(&mut d, (20.0, 0.0), (21.0, 0.0));
    let itens = inspecionar(&d, &analisar(&d), &setup);
    let nivel = |h: Handle| itens.iter().find(|i| i.handle == h).map(|i| (i.nivel, i.motivo.clone())).unwrap();
    assert_eq!(nivel(q).0, Nivel::Verde);
    assert_eq!(nivel(ok).0, Nivel::Verde);
    assert_eq!(nivel(sem_circuito).0, Nivel::Vermelho);
    assert_eq!(nivel(interruptor).0, Nivel::Vermelho);
    assert!(nivel(interruptor).1.contains("luminária"));
    assert_eq!(nivel(sem_caminho).0, Nivel::Laranja);
    assert_eq!(nivel(no_meio).0, Nivel::Laranja);
    assert!(nivel(no_meio).1.contains("em linha"), "{:?}", nivel(no_meio));
    // Encaixado, o bloco deixa de ser problema: já é a caixa do trecho.
    let mut d2 = d.clone();
    assert_eq!(eletroduto::encaixar(&mut d2, &setup, 1.0, eletroduto::TOLERANCIA_EM_LINHA, None, None).unwrap(), 1);
    let itens2 = inspecionar(&d2, &analisar(&d2), &setup);
    assert_ne!(itens2.iter().find(|i| i.handle == no_meio).unwrap().motivo, nivel(no_meio).1);
    let solto = itens.iter().find(|i| i.eletroduto).unwrap();
    assert_eq!(solto.nivel, Nivel::Laranja);
    // Do mais grave ao menos grave.
    assert_eq!(itens[0].nivel, Nivel::Vermelho);
    // Queda acima do limite e ocupação acima do limite: amarelo.
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let t = tug(&mut d, 3.0, 0.0, "1");
    let h = eletroduto::inserir(&mut d, q, t, rota::Instalacao::Teto, 0.0).unwrap();
    editar::definir_sobrescrita(&mut d, q, "1", Some(1.5), Some(6.0)).unwrap();
    eletroduto::travar_diametro(&mut d, h, Some(16.0)).unwrap();
    let apertado = Setup { queda_admissivel: 0.5, ..Setup::default() };
    let itens = inspecionar(&d, &analisar(&d), &apertado);
    assert_eq!(itens.iter().find(|i| i.handle == t).unwrap().nivel, Nivel::Amarelo, "{itens:?}");
}

#[test]
fn novos_nomes_do_xdata_e_vinculo_do_comando() {
    let mut d = planta();
    let a = analisar(&d);
    let sw = a.pontos.iter().find(|p| p.tipo == TipoPonto::Interruptor).unwrap().handle;
    let luz = a.pontos.iter().find(|p| p.tag.as_deref() == Some("LUZ-1")).unwrap().handle;
    editar::vincular_luminaria(&mut d, sw, "a", luz).unwrap();
    editar::vincular_luminaria(&mut d, sw, "a", luz).unwrap();
    let e = d.model.get(sw).unwrap();
    assert_eq!(e.common.xdata[0].value, "FREE_ELETRICA");
    assert_eq!(xdata::ler(&e.common.xdata), [("target_handles.a".to_string(), luz.hex())]);
    // Um trecho de um nome antigo é lido e migrado na próxima gravação.
    let mut x = vec![RawTag { code: 1001, value: "FREEELETRICA".into() }, RawTag { code: 1000, value: "CIRCUITO=3".into() }];
    assert_eq!(xdata::ler(&x).len(), 1);
    xdata::gravar(&mut x, "COMANDO", Some("a"));
    assert_eq!(x[0].value, "FREE_ELETRICA");
    // A letra livre conta as de interruptores de várias teclas.
    let mut d = planta();
    let a = analisar(&d);
    let sw = a.pontos.iter().find(|p| p.tipo == TipoPonto::Interruptor).unwrap().handle;
    editar::definir_comando(&mut d, sw, "ab").unwrap();
    let qd = quadro(&d);
    assert_eq!(circuito::proxima_letra(&analisar(&d).pontos, Some(qd), Some("1")), "c");
    // Remover do circuito.
    assert_eq!(editar::remover_do_circuito(&mut d, &[sw]).unwrap(), 1);
    assert_eq!(ponto::ler_ponto(&d, sw).unwrap().circuito, None);
    assert_eq!(editar::remover_do_circuito(&mut d, &[sw]).unwrap(), 0);
    assert!(editar::remover_do_circuito(&mut d, &[Handle(0xFFFF)]).is_err());
}

#[test]
fn familia_grava_no_bloco_classifica_e_recusa_lixo() {
    let mut d = metros();
    let setup = Setup::default();
    d.blocks.insert("SIMBOLO".into(), Arc::new(Block::new("SIMBOLO")));
    assert_eq!(familia::ler(&d, "SIMBOLO"), None);
    assert!(biblioteca::blocos(&d).iter().all(|b| b.nome != "SIMBOLO"), "sem família e sem atributos, não é ponto");
    let mut f = familia::Familia::da_categoria(familia::FamilyCategory::Dados);
    assert_eq!(f.dominio, familia::SystemDomain::DataTelecom);
    f.caixa = familia::CaixaFisica::C4x4;
    f.potencia = Some(50.0);
    familia::gravar(&mut d, "simbolo", Some(&f)).unwrap();
    assert_eq!(familia::ler(&d, "SIMBOLO").as_ref(), Some(&f), "o nome do bloco não diferencia maiúsculas");
    let b = biblioteca::blocos(&d).into_iter().find(|b| b.nome == "SIMBOLO").unwrap();
    assert_eq!(b.tipo, TipoPonto::Dados);
    // Dados não vai para os circuitos nem para a inspeção de força.
    let h = biblioteca::inserir_ponto(&mut d, "SIMBOLO", [1.0, 1.0], 0.0, &[], &setup).unwrap();
    let a = analisar(&d);
    assert_eq!(a.ponto(h).unwrap().tipo, TipoPonto::Dados);
    assert_eq!(d.model.get(h).unwrap().common.layer, biblioteca::CAMADA_DADOS);
    assert!(inspecao::inspecionar(&d, &a, &setup).is_empty());
    // Nenhum: mesmo com atributos elétricos, é um bloco comum.
    let t = tug(&mut d, 2.0, 2.0, "1");
    familia::gravar(&mut d, "TOMADA", Some(&familia::Familia::da_categoria(familia::FamilyCategory::Nenhum))).unwrap();
    assert!(analisar(&d).ponto(t).is_none());
    assert!(biblioteca::blocos(&d).iter().all(|b| b.nome != "TOMADA"));
    // Tirar a família devolve o que era.
    familia::gravar(&mut d, "TOMADA", None).unwrap();
    assert!(analisar(&d).ponto(t).is_some());
    // Valores hostis não chegam ao desenho.
    for ruim in [
        familia::Familia { fp: Some(1.5), ..f.clone() },
        familia::Familia { fp: Some(f64::NAN), ..f.clone() },
        familia::Familia { potencia: Some(-1.0), ..f.clone() },
        familia::Familia { teclas: 9, ..f.clone() },
        familia::Familia { cota_fixa: f64::INFINITY, ..f.clone() },
    ] {
        assert!(familia::gravar(&mut d, "SIMBOLO", Some(&ruim)).is_err(), "{ruim:?}");
    }
    assert_eq!(familia::ler(&d, "SIMBOLO").as_ref(), Some(&f));
    assert!(familia::gravar(&mut d, "  ", Some(&f)).is_err());
    // Os códigos dos enums voltam.
    for c in familia::FamilyCategory::TODAS {
        assert_eq!(familia::FamilyCategory::de_codigo(c.codigo()), Some(c));
    }
    for c in familia::SystemDomain::TODOS {
        assert_eq!(familia::SystemDomain::de_codigo(c.codigo()), Some(c));
    }
    for c in familia::RefAltura::TODAS {
        assert_eq!(familia::RefAltura::de_codigo(c.codigo()), Some(c));
    }
    for c in familia::CaixaFisica::TODAS {
        assert_eq!(familia::CaixaFisica::de_codigo(c.codigo()), Some(c));
    }
    // XDATA editado à mão: volta ao natural da categoria em vez de quebrar.
    let mut bruto = Vec::new();
    xdata::gravar(&mut bruto, "category", Some("JUNCTION_BOX"));
    xdata::gravar(&mut bruto, "keys", Some("99"));
    Arc::make_mut(&mut d.foreign)
        .records
        .insert(("BLOCK_RECORD".into(), "CX".into()), cadcraft_doc::foreign::ForeignExtra { xdata: bruto, ..Default::default() });
    assert_eq!(familia::ler(&d, "CX"), Some(familia::Familia::da_categoria(familia::FamilyCategory::Caixa)));
}

#[test]
fn altura_na_legenda_fica_vazia_e_fora_dela_mostra_h() {
    let setup = Setup::default();
    let mut d = metros();
    biblioteca::garantir_base(&mut d, &setup, 1.0);
    let attr =
        |d: &Drawing, h: Handle| ponto::ler_ponto(d, h).unwrap().atributos.iter().find(|(t, _)| t == "ALTURA").map(|(_, v)| v.clone()).unwrap();
    let xd = |d: &Drawing, h: Handle| xdata::ler(&d.model.get(h).unwrap().common.xdata);
    let altura_rota = |d: &Drawing, h: Handle, s: &Setup| rota::altura(&ponto::ler_ponto(d, h).unwrap(), s);

    // Sem altura pedida, ou igual à legenda: o atributo fica vazio e vale a legenda.
    let a = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [0.0, 0.0], 0.0, &[], &setup).unwrap();
    assert_eq!((attr(&d, a).as_str(), altura_rota(&d, a, &setup)), ("", 0.30));
    assert!(xd(&d, a).is_empty());
    let valores = [("ALTURA".to_string(), "0,30".to_string())];
    let b = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [1.0, 0.0], 0.0, &valores, &setup).unwrap();
    assert_eq!(attr(&d, b), "");

    // Fora da legenda: XDATA para o cálculo, h= visível na planta.
    let valores = [("ALTURA".to_string(), "1,45".to_string())];
    let c = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [2.0, 0.0], 0.0, &valores, &setup).unwrap();
    assert_eq!(attr(&d, c), "h=1,45m");
    assert!(xd(&d, c).iter().any(|(k, v)| k == ponto::CHAVE_ALTURA && v == "1.45"));
    assert_eq!(altura_rota(&d, c, &setup), 1.45);

    // Mudar a legenda move quem a segue e não mexe em quem tem altura própria.
    let alto = Setup { altura_baixa: 0.40, ..Setup::default() };
    assert_eq!((altura_rota(&d, a, &alto), altura_rota(&d, c, &alto)), (0.40, 1.45));

    // Editar uma inserção: voltar à legenda limpa os dois lugares.
    editar::definir_altura(&mut d, c, Some(0.30), &setup).unwrap();
    assert_eq!(attr(&d, c), "");
    assert!(xd(&d, c).is_empty());
    editar::definir_altura(&mut d, c, Some(0.5), &setup).unwrap();
    assert_eq!(attr(&d, c), "h=0,50m");
    editar::definir_altura(&mut d, c, None, &setup).unwrap();
    assert_eq!(attr(&d, c), "");
    // Um número com três casas não vira milhar.
    editar::definir_altura(&mut d, c, Some(1.125), &setup).unwrap();
    assert_eq!(altura_rota(&d, c, &setup), 1.125);

    // Entrada hostil.
    for ruim in [-1.0, 99.0, f64::NAN, f64::INFINITY] {
        assert!(editar::definir_altura(&mut d, a, Some(ruim), &setup).is_err(), "{ruim}");
    }
    assert!(editar::definir_altura(&mut d, Handle(0xFFFF), Some(1.0), &setup).is_err());
    let valores = [("ALTURA".to_string(), "alto".to_string())];
    assert!(biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [3.0, 0.0], 0.0, &valores, &setup).is_err());

    // As referências e os padrões da família.
    assert_eq!(setup.altura_de(familia::RefAltura::Teto, 0.0), 2.8);
    assert_eq!(setup.altura_de(familia::RefAltura::Piso, 9.0), 0.0);
    assert_eq!(setup.altura_de(familia::RefAltura::Fixa, 1.7), 1.7);
    let blocos = biblioteca::blocos(&d);
    let padrao = |n: &str| blocos.iter().find(|b| b.nome == n).unwrap().altura_padrao(&setup);
    assert_eq!((padrao("TOM-SIM-BAIXA"), padrao("INT-SIM"), padrao("ILU-OCT"), padrao("QDG"), padrao("RJ45")), (0.30, 1.20, 2.8, 1.5, 0.30));
    let mut f = familia::ler(&d, "TOM-SIM-BAIXA").unwrap();
    f.potencia = Some(300.0);
    f.fp = Some(0.8);
    familia::gravar(&mut d, "TOM-SIM-BAIXA", Some(&f)).unwrap();
    let e = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [4.0, 0.0], 0.0, &[], &setup).unwrap();
    let p = ponto::ler_ponto(&d, e).unwrap();
    assert_eq!((p.potencia, p.fp), (Some(300.0), Some(0.8)));
    // Bloco sem família, com altura própria na definição: o padrão dele fica.
    let mut velho = Block::new("TOMADA-VELHA");
    let mut alt = atributo("ALTURA", "0,45", false);
    alt.invisible = true;
    velho.entities.push(Entity::new(Handle(0xABC), EntityKind::AttDef(alt)));
    d.blocks.insert("TOMADA-VELHA".into(), Arc::new(velho));
    let g = biblioteca::inserir_ponto(&mut d, "TOMADA-VELHA", [5.0, 0.0], 0.0, &[], &setup).unwrap();
    assert_eq!(ponto::ler_ponto(&d, g).unwrap().altura, Some(0.45));
}

#[test]
fn utilitarios_do_editor_de_blocos() {
    let mut d = metros();
    assert_eq!(biblioteca::inserir_geometria(&mut d, "tomada", 1.0).unwrap(), 3);
    assert_eq!(biblioteca::inserir_geometria(&mut d, "tomadaAlta", 1.0).unwrap(), 4);
    assert!(biblioteca::inserir_geometria(&mut d, "porta", 1.0).is_err());
    for (forma, _) in biblioteca::GEOMETRIAS {
        assert!(biblioteca::inserir_geometria(&mut d, forma, 1.0).unwrap() > 0, "{forma}");
    }
    // Em milímetros a geometria sai mil vezes maior.
    let mut mm = Drawing::new_metric();
    biblioteca::inserir_geometria(&mut mm, "comando", 0.001).unwrap();
    assert!(mm.model.iter().any(|e| matches!(&e.kind, EntityKind::Circle(c) if perto(c.radius, 75.0, 1e-9))));

    // O conector vira a origem: tudo anda junto; definir de novo só troca o marcador.
    let mut d = metros();
    d.add(&Space::Model, Common::default(), EntityKind::Line(Line { a: Vec3::new(1.0, 1.0, 0.0), b: Vec3::new(2.0, 1.0, 0.0) })).unwrap();
    let c1 = biblioteca::definir_conector(&mut d, [1.0, 1.0], 1.0).unwrap();
    let linha = |d: &Drawing| d.model.iter().find_map(|e| if let EntityKind::Line(l) = &e.kind { Some(l.clone()) } else { None }).unwrap();
    assert!(perto(linha(&d).a.x, 0.0, 1e-9) && perto(linha(&d).b.x, 1.0, 1e-9));
    assert_eq!(d.model.get(c1).unwrap().common.layer, "Defpoints");
    let c2 = biblioteca::definir_conector(&mut d, [0.5, 0.0], 1.0).unwrap();
    assert_eq!(d.model.iter().filter(|e| matches!(e.kind, EntityKind::Circle(_))).count(), 1);
    assert!(d.model.get(c1).is_none());
    let EntityKind::Circle(circulo) = &d.model.get(c2).unwrap().kind else { panic!() };
    assert_eq!((circulo.center.x, circulo.center.y), (0.0, 0.0));
    assert!(perto(linha(&d).a.x, -0.5, 1e-9));
    assert!(biblioteca::definir_conector(&mut d, [f64::NAN, 0.0], 1.0).is_err());

    // Atributos: os da categoria, sem duplicar; ALTURA fica visível e vazio.
    use familia::FamilyCategory as C;
    let mut d = metros();
    let n = biblioteca::gerar_atributos(&mut d, C::Interruptor, 1.0).unwrap();
    assert!(n >= 5);
    assert_eq!(biblioteca::gerar_atributos(&mut d, C::Interruptor, 1.0).unwrap(), 0);
    let tags: Vec<String> = d.model.iter().filter_map(|e| if let EntityKind::AttDef(a) = &e.kind { Some(a.tag.clone()) } else { None }).collect();
    for t in ["CIRCUITO", "COMANDO", "ALTURA"] {
        assert!(tags.iter().any(|x| x == t), "{t}");
    }
    assert!(d.model.iter().any(|e| matches!(&e.kind, EntityKind::AttDef(a) if a.tag == "ALTURA" && !a.invisible && a.text.value.is_empty())));
    assert!(biblioteca::gerar_atributos(&mut d, C::Nenhum, 1.0).is_err());
    for c in C::TODAS.into_iter().filter(|c| *c != C::Nenhum) {
        let mut d = metros();
        assert!(biblioteca::gerar_atributos(&mut d, c, 1.0).unwrap() >= 2, "{c:?}");
        assert!(d.model.iter().any(|e| matches!(&e.kind, EntityKind::AttDef(a) if a.tag == "ALTURA")), "{c:?}");
    }
}

#[test]
fn limite_de_queda_por_natureza_do_circuito() {
    let d = planta();
    let setup = Setup { queda_iluminacao: 3.0, queda_admissivel: 5.0, ..Setup::default() };
    let r = rotear_quadro(&d, &analisar(&d), &setup, quadro(&d)).unwrap();
    let l = r.linhas.iter().find(|l| l.circuito.natureza == circuito::Natureza::Iluminacao).unwrap();
    assert_eq!(l.queda_admissivel, 3.0);
    let l = r.linhas.iter().find(|l| l.circuito.natureza != circuito::Natureza::Iluminacao).unwrap();
    assert_eq!(l.queda_admissivel, 5.0);
}

#[test]
fn tipo_de_cabo_por_circuito_escolhe_a_tabela() {
    use nbr5410::{Isolacao, TipoCabo as C};
    let mut d = planta();
    let qd = quadro(&d);
    let setup = Setup::default();
    let linha = |d: &Drawing, id: &str, s: &Setup| {
        rotear_quadro(d, &analisar(d), s, quadro(d)).unwrap().linhas.into_iter().find(|l| l.circuito.id == id).unwrap()
    };
    assert_eq!((linha(&d, "3", &setup).tipo_cabo, linha(&d, "3", &setup).isolacao), (C::FlexivelPvc70, Isolacao::Pvc));
    let antes = linha(&d, "3", &setup).escolha;

    // O chuveiro em HEPR 90 °C: outra capacidade e outro FCT; os outros circuitos não mudam.
    editar::definir_cabo_circuito(&mut d, qd, "3", Some(C::HeprXlpe90)).unwrap();
    let l = linha(&d, "3", &setup);
    assert_eq!((l.tipo_cabo, l.isolacao, l.isolacao_propria), (C::HeprXlpe90, Isolacao::Epr, true));
    assert!(l.escolha.iz.unwrap() != antes.iz.unwrap() || l.escolha.secao != antes.secao);
    assert_eq!(linha(&d, "2", &setup).tipo_cabo, C::FlexivelPvc70);
    let quente = Setup { temperatura_ambiente: 50.0, ..Setup::default() };
    assert!(linha(&d, "3", &quente).fct > linha(&d, "2", &quente).fct, "a 50 °C o cabo de 90 °C perde menos");

    // Todos os tipos: a temperatura da isolação decide a tabela, e a tensão a coluna do diâmetro.
    for c in C::TODOS {
        assert_eq!(C::de_codigo(c.codigo()), Some(c));
        editar::definir_cabo_circuito(&mut d, qd, "3", Some(c)).unwrap();
        let l = linha(&d, "3", &setup);
        assert_eq!(l.tipo_cabo, c);
        assert_eq!(l.isolacao, if matches!(c, C::HeprXlpe90 | C::LszhHepr90) { Isolacao::Epr } else { Isolacao::Pvc }, "{c:?}");
    }
    assert!(setup.diametro_do_cabo(2.5, C::UnipolarPvc70) > setup.diametro_do_cabo(2.5, C::FlexivelPvc70), "0,6/1 kV é mais grosso");
    assert_eq!(setup.diametro_do_cabo(3.0, C::FlexivelPvc70), None);

    // None volta ao padrão das configurações; o padrão vale para quem não escolheu.
    editar::definir_cabo_circuito(&mut d, qd, "3", None).unwrap();
    assert!(!linha(&d, "3", &setup).isolacao_propria);
    let padrao = Setup { cabo_padrao: C::Lszh70, ..Setup::default() };
    assert_eq!(linha(&d, "3", &padrao).tipo_cabo, C::Lszh70);

    // Desenho anterior, com isolação gravada: vira o cabo equivalente; o tipo novo a substitui.
    editar::definir_isolacao_circuito(&mut d, qd, "3", Some(Isolacao::Xlpe)).unwrap();
    assert_eq!(linha(&d, "3", &setup).tipo_cabo, C::HeprXlpe90);
    editar::definir_cabo_circuito(&mut d, qd, "3", Some(C::FlexivelPvc70)).unwrap();
    assert_eq!(linha(&d, "3", &setup).isolacao, Isolacao::Pvc);
    assert!(editar::definir_cabo_circuito(&mut d, qd, " ", Some(C::Lszh70)).is_err());
    // Excluir o circuito leva junto.
    editar::excluir_circuito(&mut d, qd, "3").unwrap();
    assert!(!quadro::propriedades(analisar(&d).ponto(qd).unwrap()).contains_key("3"));
}

#[test]
fn tipo_de_eletroduto_entra_na_ocupacao_e_no_xdata() {
    use rota::TipoEletroduto as T;
    let setup = Setup::default();
    for t in T::TODOS {
        assert_eq!(T::de_codigo(t.codigo()), Some(t));
        assert_eq!(T::de_codigo(t.abreviado()), Some(t));
        assert!(!setup.tamanhos_de(t).is_empty());
    }
    assert_eq!(setup.tamanhos_de(T::PvcRigido), setup.eletrodutos, "o rígido usa a tabela editável");
    // Os mesmos condutores ocupam mais num corrugado do que num rígido do mesmo nominal.
    let tres = [3.6, 3.6, 3.6, 3.6, 3.6, 3.6, 3.6];
    let (rigido, oc_r) = setup.eletroduto_do_tipo(&tres, T::PvcRigido).unwrap();
    let (corrugado, oc_c) = setup.eletroduto_do_tipo(&tres, T::CorrugadoReforcado).unwrap();
    assert!(corrugado.nominal > rigido.nominal || oc_c > oc_r, "{rigido:?} {corrugado:?}");
    // PEAD não tem 20 mm: começa no menor dele.
    assert_eq!(setup.eletroduto_do_tipo(&[3.6], T::PeadCorrugado).unwrap().0.nominal, 40.0);

    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 4.0, 0.0, "1");
    let h = eletroduto::inserir_com_tipo(&mut d, q, a, rota::Instalacao::Teto, 0.0, T::AcoFogo).unwrap();
    assert!(xdata::ler(&d.model.get(h).unwrap().common.xdata).iter().any(|(k, v)| k == "conduit_type" && v == "STEEL_HOT_DIP"));
    assert_eq!(rede_de(&d).arestas[0].tipo, T::AcoFogo);
    let f = fiacao::calcular(&d, &analisar(&d), &setup, q).unwrap();
    assert_eq!(f.trechos[0].tipo, T::AcoFogo);
    // Dividir mantém o tipo nas duas partes.
    eletroduto::dividir(&mut d, &setup, h, [2.0, 0.0], 1.0).unwrap();
    assert!(rede_de(&d).arestas.iter().all(|e| e.tipo == T::AcoFogo));
    eletroduto::definir_tipo(&mut d, h, T::PeadCorrugado).unwrap();
    assert!(rede_de(&d).arestas.iter().any(|e| e.tipo == T::PeadCorrugado));
    assert!(eletroduto::definir_tipo(&mut d, Handle(0xFFFF), T::PvcRigido).is_err());
    // Um eletroduto sem tipo gravado é PVC rígido; um diâmetro travado precisa existir no tipo.
    let mut d = metros();
    let q2 = qd(&mut d, 0.0, 0.0);
    let b = tug(&mut d, 4.0, 0.0, "1");
    eletroduto(&mut d, (0.0, 0.0), (4.0, 0.0));
    assert_eq!(rede_de(&d).arestas[0].tipo, T::PvcRigido);
    let h = rede_de(&d).arestas[0].eletroduto;
    eletroduto::definir_tipo(&mut d, h, T::CorrugadoLeve).unwrap();
    eletroduto::travar_diametro(&mut d, h, Some(85.0)).unwrap();
    let f = fiacao::calcular(&d, &analisar(&d), &setup, q2).unwrap();
    assert!(f.avisos.iter().any(|a| a.contains("PVC-CL")), "{:?}", f.avisos);
    let _ = b;
}

#[test]
fn lista_de_materiais_separa_por_tipo_e_diametro() {
    use nbr5410::TipoCabo as C;
    use rota::TipoEletroduto as T;
    let setup = Setup::default();
    let mut d = metros();
    biblioteca::garantir_base(&mut d, &setup, 1.0);
    let v = |c: &str| vec![("CIRCUITO".to_string(), c.to_string())];
    let q = biblioteca::inserir_ponto(&mut d, "QDG", [0.0, 0.0], 0.0, &[], &setup).unwrap();
    let a = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [4.0, 0.0], 0.0, &v("1"), &setup).unwrap();
    let b = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [8.0, 0.0], 0.0, &v("2"), &setup).unwrap();
    biblioteca::inserir_ponto(&mut d, "RJ45", [9.0, 9.0], 0.0, &[], &setup).unwrap();
    biblioteca::inserir_ponto(&mut d, "ILU-OCT", [1.0, 9.0], 0.0, &[], &setup).unwrap();
    let solto = biblioteca::inserir_ponto(&mut d, "CX-PASSAGEM", [20.0, 0.0], 0.0, &[], &setup).unwrap();
    let _ = solto;
    editar::definir_cabo_circuito(&mut d, q, "2", Some(C::HeprXlpe90)).unwrap();
    eletroduto::inserir_com_tipo(&mut d, q, a, rota::Instalacao::Teto, 0.0, T::CorrugadoReforcado).unwrap();
    eletroduto::inserir_com_tipo(&mut d, a, b, rota::Instalacao::Teto, 0.0, T::AcoFogo).unwrap();
    eletroduto(&mut d, (30.0, 0.0), (31.0, 0.0));
    let m = materiais::levantar(&d, &analisar(&d), &setup);
    // Mesma seção, tipos diferentes: duas linhas; com a folga de cabos.
    assert!(m.cabos.iter().any(|c| c.tipo == C::FlexivelPvc70 && c.secao == 2.5 && c.metros > 0.0), "{:?}", m.cabos);
    assert!(m.cabos.iter().any(|c| c.tipo == C::HeprXlpe90 && c.metros > 0.0), "{:?}", m.cabos);
    // Metragem em 3D por tipo e diâmetro.
    let duto = |t: T| m.eletrodutos.iter().find(|e| e.tipo == t).unwrap().clone();
    assert!(perto(duto(T::CorrugadoReforcado).metros, 4.0 + 1.3 + 2.5, 1e-9), "{:?}", m.eletrodutos);
    // Três cabos HEPR de 6 mm passam de 40 % no Ø20 de aço a fogo.
    assert_eq!(duto(T::AcoFogo).diametro, Some(25.0));
    assert_eq!(m.eletrodutos_soltos, 1);
    // Caixas pela família; pontos e disjuntores contados.
    let qtd = |l: &[materiais::ItemContado], n: &str| l.iter().find(|i| i.nome == n).map_or(0, |i| i.quantidade);
    assert_eq!(qtd(&m.caixas, "4x2\""), 3, "{:?}", m.caixas);
    assert_eq!(qtd(&m.caixas, "Octogonal"), 1);
    assert_eq!(qtd(&m.caixas, "Passagem 15x15"), 1);
    assert_eq!(qtd(&m.pontos, "TUG"), 2);
    assert_eq!(qtd(&m.pontos, "Dados/Telecom"), 1);
    assert_eq!(m.disjuntores.iter().map(|i| i.quantidade).sum::<usize>(), 3, "dois circuitos e o geral: {:?}", m.disjuntores);
    assert_eq!(m.pontos_sem_caixa, 0);
    // Desenho vazio.
    assert_eq!(materiais::levantar(&metros(), &analisar(&metros()), &setup), materiais::Materiais::default());
}

#[test]
fn perfis_de_eletroduto_aplicam_linha_cor_caminhamento_e_diametros() {
    use eletroduto::{Percurso, Tracado};
    use setup::{PerfilEletroduto, TamanhoEletroduto, TipoLinha};
    let padrao = Setup::default();
    let nomes: Vec<&str> = padrao.perfis_eletroduto.iter().map(|p| p.nome.as_str()).collect();
    assert_eq!(nomes, ["Eletroduto PVC Flexível - Teto", "Eletroduto PVC Flexível - Parede", "Eletroduto PEAD - Piso", "Eletroduto Aço Galvanizado"]);
    assert!(padrao.perfis_eletroduto.iter().all(|p| !p.diametros.is_empty()));
    // Um perfil próprio com tabela curta: Ø30 interno 26.
    let perfil = PerfilEletroduto {
        nome: "Meu perfil".into(),
        tipo: rota::TipoEletroduto::AcoFogo,
        tipo_linha: TipoLinha::Tracejada,
        cor: 5,
        caminhamento: rota::Instalacao::Piso,
        diametros: vec![TamanhoEletroduto { nominal: 30.0, polegadas: String::new(), interno: 26.0 }],
    };
    let mut s = Setup::default();
    s.perfis_eletroduto.push(perfil.clone());
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 4.0, 0.0, "1");
    let h = eletroduto::inserir_com_perfil(&mut d, q, a, &Percurso { tracado: Tracado::Reto, ..Percurso::default() }, &perfil).unwrap();
    let e = d.model.get(h).unwrap();
    // No piso a linha é traço e ponto, mesmo que o perfil peça outra.
    assert_eq!((e.common.linetype.as_str(), e.common.layer.as_str()), ("DASHDOT", eletroduto::CAMADA_PISO));
    assert_eq!(e.common.color, cadcraft_doc::color::Color::Index(5));
    assert!(d.linetype("DASHDOT").is_some(), "o tipo de linha entra no desenho");
    let (m, _) = s.unidade.metros(d.header.i64("INSUNITS", 0));
    let rede = rota::Rede::montar(&d, &s, &analisar(&d).pontos, m);
    assert_eq!(
        (rede.arestas[0].perfil.as_deref(), rede.arestas[0].instalacao, rede.arestas[0].tipo),
        (Some("Meu perfil"), rota::Instalacao::Piso, rota::TipoEletroduto::AcoFogo)
    );
    // O diâmetro sai da tabela do perfil.
    let f = fiacao::calcular(&d, &analisar(&d), &s, q).unwrap();
    assert_eq!(f.trechos[0].eletroduto.as_ref().map(|e| e.nominal), Some(30.0));
    assert_eq!(s.tamanhos_do_trecho(Some("Meu perfil"), rota::TipoEletroduto::PvcRigido)[0].nominal, 30.0);
    // Sem o perfil (excluído depois), o trecho volta à tabela do tipo.
    assert_eq!(s.tamanhos_do_trecho(Some("Excluído"), rota::TipoEletroduto::AcoFogo), s.tamanhos_de(rota::TipoEletroduto::AcoFogo));
    let f = fiacao::calcular(&d, &analisar(&d), &Setup::default(), q).unwrap();
    assert_eq!(f.trechos[0].eletroduto.as_ref().map(|e| e.nominal), Some(20.0));
    // Trocar o tipo à mão tira o perfil.
    eletroduto::definir_tipo(&mut d, h, rota::TipoEletroduto::PvcRigido).unwrap();
    let rede = rota::Rede::montar(&d, &s, &analisar(&d).pontos, m);
    assert_eq!(rede.arestas[0].perfil, None);
    // Dividir o trecho mantém a linha e a cor nas duas partes.
    eletroduto::aplicar_perfil(&mut d, h, &perfil).unwrap();
    eletroduto::dividir(&mut d, &s, h, [2.0, 0.0], m).unwrap();
    let partes: Vec<_> = d.model.iter().filter(|e| e.common.layer == eletroduto::CAMADA_PISO).collect();
    assert_eq!(partes.len(), 2);
    assert!(partes.iter().all(|e| e.common.linetype == "DASHDOT" && e.common.color == cadcraft_doc::color::Color::Index(5)));
    // Preferências hostis: nome vazio ou repetido sai, diâmetro inválido sai; nunca fica sem perfil.
    let mut h1 = Setup::default();
    h1.perfis_eletroduto.push(PerfilEletroduto { nome: "  ".into(), ..PerfilEletroduto::default() });
    h1.perfis_eletroduto.push(PerfilEletroduto { nome: "eletroduto pead - piso".into(), ..PerfilEletroduto::default() });
    h1.perfis_eletroduto.push(PerfilEletroduto {
        nome: "Cor zero".into(),
        diametros: vec![TamanhoEletroduto { nominal: f64::NAN, polegadas: String::new(), interno: 1.0 }],
        ..PerfilEletroduto::default()
    });
    h1.normalizar();
    assert_eq!(h1.perfis_eletroduto.len(), 5);
    let ultimo = h1.perfis_eletroduto.last().unwrap();
    assert!(ultimo.cor >= 1 && ultimo.diametros.is_empty());
    h1.perfis_eletroduto.clear();
    h1.normalizar();
    assert_eq!(h1.perfis_eletroduto.len(), 4);
    // Scripts antigos ainda pedem ORTHOGONAL.
    assert_eq!(Tracado::de_codigo("ORTHOGONAL"), Some(Tracado::Angulo45));
}

#[test]
fn catalogos_de_cabos_com_edicao_e_exclusao() {
    use nbr5410::TipoCabo as C;
    use setup::{CaboEletrico, CaboTelecom, CategoriaTelecom};
    let mut s = Setup::default();
    // Elétricos: cadastrar, editar, excluir, nunca ficar sem nenhum.
    assert_eq!(s.cabos_eletricos.len(), C::TODOS.len());
    assert!(s.cadastrar_cabo_eletrico(CaboEletrico { nome: " ".into(), ..CaboEletrico::default() }).is_err());
    let novo = CaboEletrico { nome: "Sil Afitox 750".into(), classe: C::Lszh70, observacao: "obra X".into() };
    s.cadastrar_cabo_eletrico(novo.clone()).unwrap();
    assert!(s.cadastrar_cabo_eletrico(novo.clone()).is_err(), "nome repetido");
    assert!(s.atualizar_cabo_eletrico("não existe", novo.clone()).is_err());
    let outro = s.cabos_eletricos[0].nome.clone();
    assert!(s.atualizar_cabo_eletrico("Sil Afitox 750", CaboEletrico { nome: outro.clone(), ..novo.clone() }).is_err(), "nome de outro cabo");
    s.atualizar_cabo_eletrico("Sil Afitox 750", CaboEletrico { nome: "Sil Afitox".into(), classe: C::LszhHepr90, observacao: String::new() })
        .unwrap();
    assert_eq!(s.cabos_eletricos.last().unwrap().classe, C::LszhHepr90);
    assert!(s.remover_cabo_eletrico("Sil Afitox"));
    assert!(!s.remover_cabo_eletrico("Sil Afitox"));
    while s.cabos_eletricos.len() > 1 {
        let n = s.cabos_eletricos[0].nome.clone();
        assert!(s.remover_cabo_eletrico(&n));
    }
    let n = s.cabos_eletricos[0].nome.clone();
    assert!(!s.remover_cabo_eletrico(&n), "o último fica");
    // O nome do catálogo chega ao circuito e aos materiais; a classe vale no cálculo.
    let mut s = Setup::default();
    s.cadastrar_cabo_eletrico(CaboEletrico { nome: "Sil Afitox".into(), classe: C::HeprXlpe90, observacao: String::new() }).unwrap();
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 4.0, 0.0, "1");
    eletroduto::inserir(&mut d, q, a, rota::Instalacao::Teto, 0.0).unwrap();
    editar::definir_cabo_circuito(&mut d, q, "1", Some(C::HeprXlpe90)).unwrap();
    editar::definir_nome_cabo(&mut d, q, "1", Some("Sil Afitox")).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &s, q).unwrap();
    assert_eq!((r.linhas[0].tipo_cabo, r.linhas[0].nome_cabo.as_str()), (C::HeprXlpe90, "Sil Afitox"));
    let m = materiais::levantar(&d, &analisar(&d), &s);
    assert!(m.cabos.iter().any(|c| c.nome == "Sil Afitox" && c.tipo == C::HeprXlpe90 && c.metros > 0.0), "{:?}", m.cabos);
    // Sem nome próprio: o da classe no catálogo.
    editar::definir_nome_cabo(&mut d, q, "1", None).unwrap();
    assert_eq!(rotear_quadro(&d, &analisar(&d), &s, q).unwrap().linhas[0].nome_cabo, C::HeprXlpe90.nome());
    // Telecom: editar um de fábrica sem mudar o nome o mantém de fábrica; renomear libera o nome antigo.
    let mut t = Setup::default();
    let base = CaboTelecom {
        nome: "UTP Cat 6".into(),
        categoria: CategoriaTelecom::Utp,
        quantidade: 4,
        diametro: 6.3,
        observacao: "lote".into(),
        padrao: false,
    };
    t.atualizar_cabo("UTP Cat 6", base.clone()).unwrap();
    let c = t.cabos_telecom.iter().find(|c| c.nome == "UTP Cat 6").unwrap();
    assert!(c.padrao && c.diametro == 6.3);
    assert!(t.atualizar_cabo("UTP Cat 6", CaboTelecom { diametro: f64::NAN, ..base.clone() }).is_err());
    assert!(t.atualizar_cabo("UTP Cat 6", CaboTelecom { nome: "UTP Cat 5".into(), ..base.clone() }).is_err(), "nome de outro cabo");
    t.atualizar_cabo("UTP Cat 6", CaboTelecom { nome: "UTP Cat 6 blindado".into(), ..base }).unwrap();
    t.normalizar();
    assert!(t.cabos_telecom.iter().all(|c| c.nome != "UTP Cat 6"), "o nome antigo não volta");
    let volta: Setup = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(volta.cabos_telecom, t.cabos_telecom);
}

#[test]
fn esquemas_curva_grupos_de_dr_e_unifilar() {
    use nbr5410::{Curva, Esquema as E};
    // Os seis esquemas, com os condutores de cada um.
    let codigos: Vec<&str> = E::TODOS.iter().map(|e| e.codigo()).collect();
    assert_eq!(codigos, ["F+N", "F+N+T", "2F+T", "2F+N+T", "3F+T", "3F+N+T"]);
    for e in E::TODOS {
        assert_eq!(E::de_codigo(e.codigo()), Some(e));
    }
    assert_eq!((E::Ffnt.fases(), E::Ffnt.neutro(), E::Ffnt.protecao(), E::Ffnt.condutores(), E::Ffnt.carregados()), (2, true, true, 4, 3));
    assert_eq!((E::Ffft.fases(), E::Ffft.neutro(), E::Ffft.protecao(), E::Ffft.condutores(), E::Ffft.carregados()), (3, false, true, 4, 3));
    assert_eq!(E::Fft.carregados(), 2);

    let setup = Setup::default();
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 2.0, 0.0, "1");
    let b = tug(&mut d, 4.0, 0.0, "2");
    let c = tug(&mut d, 6.0, 0.0, "3");
    for (de, para) in [(q, a), (a, b), (b, c)] {
        eletroduto::inserir(&mut d, de, para, rota::Instalacao::Teto, 0.0).unwrap();
    }
    let rotear = |d: &Drawing| rotear_quadro(d, &analisar(d), &setup, q).unwrap();

    // Duas fases com neutro carregam três condutores: a capacidade é a da coluna de três.
    let dois = {
        editar::definir_esquema(&mut d, q, "1", Some(E::Fft)).unwrap();
        rotear(&d).linhas[0].escolha.iz
    };
    editar::definir_esquema(&mut d, q, "1", Some(E::Ffnt)).unwrap();
    let r = rotear(&d);
    assert_eq!((r.linhas[0].esquema, r.linhas[0].condutores, r.linhas[0].circuito.fases), (E::Ffnt, 4, 2));
    assert!(r.linhas[0].escolha.iz.unwrap() < dois.unwrap(), "{:?} {:?}", r.linhas[0].escolha.iz, dois);
    assert_eq!(r.linhas[0].indicacao.split(' ').next(), Some("2F+N+T"));
    editar::definir_esquema(&mut d, q, "2", Some(E::Ffft)).unwrap();
    assert_eq!(rotear(&d).linhas[1].condutores, 4);
    editar::definir_esquema(&mut d, q, "1", None).unwrap();
    editar::definir_esquema(&mut d, q, "2", None).unwrap();

    // Curva: a sugerida e a escolhida.
    assert_eq!(quadro::curva_sugerida(&[demanda::TipoCarga::Aquecimento]), Curva::B);
    assert_eq!(quadro::curva_sugerida(&[demanda::TipoCarga::Aquecimento, demanda::TipoCarga::IluminacaoTug]), Curva::C);
    assert_eq!(quadro::curva_sugerida(&[]), Curva::C);
    let r = rotear(&d);
    assert_eq!((r.linhas[0].curva, r.linhas[0].curva_propria), (Curva::C, false));
    editar::definir_curva(&mut d, q, "1", Some(Curva::D)).unwrap();
    let r = rotear(&d);
    assert_eq!((r.linhas[0].curva, r.linhas[0].curva_propria), (Curva::D, true));
    assert_eq!(Curva::de_codigo(" d "), Some(Curva::D));
    assert_eq!(Curva::de_codigo("Z"), None);

    // Grupos de DR à vontade, com a soma das correntes.
    assert!(rotear(&d).grupos_dr().is_empty());
    assert_eq!(quadro::proximo_grupo_dr(&[]), "IDR1");
    assert_eq!(quadro::proximo_grupo_dr(&["idr1".into(), "IDR3".into()]), "IDR2");
    editar::definir_grupo_dr(&mut d, q, "1", Some("IDR2")).unwrap();
    editar::definir_grupo_dr(&mut d, q, "2", Some("idr2")).unwrap();
    editar::definir_grupo_dr(&mut d, q, "3", Some("IDR7")).unwrap();
    let r = rotear(&d);
    let g = r.grupos_dr();
    assert_eq!(g.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(), ["IDR2", "IDR7"]);
    assert_eq!(g[0].circuitos, ["1", "2"]);
    assert!(perto(g[0].ib, r.linhas[0].escolha.ib + r.linhas[1].escolha.ib, 1e-9));
    assert_eq!(g[0].dr, Some(25.0));

    // Unifilar: nasce ao lado do quadro, com um disjuntor por circuito e o geral; regenerar mantém o lugar.
    let analise = analisar(&d);
    let h = unifilar::gerar(&mut d, &analise, &setup, q, None).unwrap();
    assert_eq!(unifilar::unifilares(&d), vec![(h, q)]);
    let posicao = |d: &Drawing| match &d.model.get(h).unwrap().kind {
        EntityKind::Insert(i) => [i.insert.x, i.insert.y],
        _ => panic!("inserção"),
    };
    assert_eq!(posicao(&d), [3.0, 0.0]);
    let nome = format!("{}{}", unifilar::PREFIXO, q.hex());
    let textos = |d: &Drawing| -> Vec<String> {
        d.blocks
            .get(&nome)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect()
    };
    let t = textos(&d);
    assert!(t.iter().any(|x| x.starts_with("DIAGRAMA UNIFILAR")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("IDR2") && x.contains("30 mA")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("1 ") && x.contains("A/D")), "{t:?}");
    assert_eq!(t.iter().filter(|x| x.contains(" VA")).count(), 3, "{t:?}");
    unifilar::gerar(&mut d, &analise, &setup, q, Some([50.0, 50.0])).unwrap();
    assert_eq!(posicao(&d), [50.0, 50.0]);
    assert!(unifilar::gerar(&mut d, &analise, &setup, q, Some([f64::NAN, 0.0])).is_err());
    // ELEREGEN atualiza o conteúdo e não mexe na posição.
    editar::definir_descricao(&mut d, q, "1", Some("Sala")).unwrap();
    let reg = fiacao::regenerar(&mut d, &setup, None).unwrap();
    assert_eq!(reg.unifilares, 1);
    assert!(textos(&d).iter().any(|x| x.contains("Sala")));
    assert_eq!(posicao(&d), [50.0, 50.0]);
    assert_eq!(unifilar::unifilares(&d).len(), 1);
    // O quadro saiu: o diagrama sai junto.
    d.model.remove(q);
    let analise = analisar(&d);
    assert_eq!(unifilar::regenerar(&mut d, &analise, &setup).unwrap(), 0);
    assert!(unifilar::unifilares(&d).is_empty());
}

#[test]
fn liberar_fases_devolve_ao_balanceamento() {
    let mut d = planta();
    let qd = quadro(&d);
    editar::travar_fases(&mut d, qd, "2", Some("R")).unwrap();
    editar::travar_fases(&mut d, qd, "1", Some("R")).unwrap();
    assert_eq!(editar::liberar_fases(&mut d, qd).unwrap(), 2);
    assert_eq!(editar::liberar_fases(&mut d, qd).unwrap(), 0);
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert!(r.linhas.iter().all(|l| !l.fases_travadas));
    assert!(editar::liberar_fases(&mut d, Handle(0xFFFF)).is_err());
}

#[test]
fn tracados_de_eletroduto_e_comprimento_real() {
    use eletroduto::{Percurso, Tracado as T};
    let setup = Setup::default();
    for c in T::TODOS {
        assert_eq!(T::de_codigo(c.codigo()), Some(c));
    }
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 4.0, 3.0, "1");
    let tipo = rota::TipoEletroduto::PvcRigido;
    let inserir = |d: &mut Drawing, p: Percurso| {
        let h = eletroduto::inserir_percurso(d, q, a, rota::Instalacao::Teto, &p, tipo).unwrap();
        let rede = rede_de(d);
        let e = rede.arestas.iter().find(|e| e.eletroduto == h).unwrap().clone();
        (h, e.comprimento_2d, e.comprimento)
    };
    // Reto: a hipotenusa, e os desníveis 3D somam igual em todos.
    let (h, l2d, l3d) = inserir(&mut d, Percurso { tracado: T::Reto, ..Percurso::default() });
    assert!(matches!(d.model.get(h).unwrap().kind, EntityKind::Line(_)));
    assert!(perto(l2d, 5.0, 1e-9) && perto(l3d, 5.0 + 1.3 + 2.5, 1e-9));
    // 45°: de (0,0) a (4,3), um trecho reto de 1 e uma diagonal de 3; invertido, a diagonal vem antes.
    let quarenta_e_cinco = 1.0 + 3.0 * 2.0_f64.sqrt();
    let (h, l2d, _) = inserir(&mut d, Percurso { tracado: T::Angulo45, ..Percurso::default() });
    let EntityKind::LwPolyline(pl) = &d.model.get(h).unwrap().kind else { panic!("polilinha") };
    assert_eq!((pl.vertices[1].p.x, pl.vertices[1].p.y), (1.0, 0.0));
    assert!(perto(l2d, quarenta_e_cinco, 1e-9), "{l2d}");
    let (h, l2d, _) = inserir(&mut d, Percurso { tracado: T::Angulo45, diagonal_primeiro: true, ..Percurso::default() });
    let EntityKind::LwPolyline(pl) = &d.model.get(h).unwrap().kind else { panic!("polilinha") };
    assert_eq!((pl.vertices[1].p.x, pl.vertices[1].p.y), (3.0, 3.0));
    assert!(perto(l2d, quarenta_e_cinco, 1e-9), "{l2d}");
    // Para todos os lados, cada trecho é reto ou a 45° exatos.
    for destino in [[-5.0, 2.0], [2.0, -5.0], [-3.0, -7.0], [6.0, 1.0]] {
        let v = Percurso { tracado: T::Angulo45, ..Percurso::default() }.pontos([0.0, 0.0], destino, 5);
        assert_eq!((v.first().copied(), v.last().copied()), (Some([0.0, 0.0]), Some(destino)));
        for w in v.windows(2) {
            let (dx, dy) = ((w[1][0] - w[0][0]).abs(), (w[1][1] - w[0][1]).abs());
            assert!(dx < 1e-9 || dy < 1e-9 || (dx - dy).abs() < 1e-9, "{v:?}");
        }
    }
    // Arco: mais comprido que a corda.
    let (h, l2d, _) = inserir(&mut d, Percurso { tracado: T::Arco, flecha: 0.2, ..Percurso::default() });
    assert!(matches!(d.model.get(h).unwrap().kind, EntityKind::Spline(_)));
    assert!(l2d > 5.0, "{l2d}");
    // Desvios: passa por eles em qualquer traçado, sem perder as pontas.
    let desvios = vec![[2.0, 2.0]];
    let (_, l2d, _) = inserir(&mut d, Percurso { tracado: T::Reto, desvios: desvios.clone(), ..Percurso::default() });
    assert!(perto(l2d, 8.0_f64.sqrt() + 5.0_f64.sqrt(), 1e-9), "{l2d}");
    let (h, _, _) = inserir(&mut d, Percurso { tracado: T::Arco, desvios: desvios.clone(), ..Percurso::default() });
    assert!(matches!(d.model.get(h).unwrap().kind, EntityKind::Spline(_)));
    let (_, l2d, _) = inserir(&mut d, Percurso { tracado: T::Angulo45, desvios, ..Percurso::default() });
    assert!(perto(l2d, 1.0 + 3.0 * 2.0_f64.sqrt(), 1e-9), "{l2d}");
    assert!(rede_de(&d).soltos.is_empty(), "todos ligam os dois blocos");
    // Alinhados: o 45° é uma linha; entrada hostil recusa.
    let b = tug(&mut d, 4.0, 0.0, "1");
    let h =
        eletroduto::inserir_percurso(&mut d, q, b, rota::Instalacao::Teto, &Percurso { tracado: T::Angulo45, ..Percurso::default() }, tipo).unwrap();
    assert!(matches!(d.model.get(h).unwrap().kind, EntityKind::Line(_)));
    let ruim = Percurso { desvios: vec![[f64::NAN, 0.0]], ..Percurso::default() };
    assert!(eletroduto::inserir_percurso(&mut d, q, b, rota::Instalacao::Teto, &ruim, tipo).is_err());
    let _ = setup;
}

#[test]
fn compartilhar_o_pe_reduz_condutores_ocupacao_e_compra() {
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let cx = caixa(&mut d, 2.0, 0.0);
    let (a, b) = (tug(&mut d, 4.0, 1.0, "1"), tug(&mut d, 4.0, -1.0, "2"));
    for (x, y) in [(q, cx), (cx, a), (cx, b)] {
        eletroduto::inserir(&mut d, x, y, rota::Instalacao::Teto, 0.0).unwrap();
    }
    editar::definir_sobrescrita(&mut d, q, "2", Some(4.0), None).unwrap();
    let normal = Setup::default();
    let dividido = Setup { compartilhar_pe: true, ..Setup::default() };
    let tronco =
        |s: &Setup, d: &Drawing| fiacao::calcular(d, &analisar(d), s, q).unwrap().trechos.into_iter().find(|t| t.circuitos.len() == 2).unwrap();
    let (t1, t2) = (tronco(&normal, &d), tronco(&dividido, &d));
    assert_eq!((t1.condutores(), t2.condutores()), (6, 5));
    assert!(t2.ocupacao.unwrap() < t1.ocupacao.unwrap());
    // Fica o de maior seção, e a marca sai junto.
    let pe: Vec<_> = t2.circuitos.iter().filter(|k| k.protecao).collect();
    assert_eq!((pe.len(), pe[0].secao), (1, Some(4.0)));
    assert_eq!(t2.pe_removidos.len(), 1);
    // Nos ramais, com um circuito só, nada muda.
    let ramais = fiacao::calcular(&d, &analisar(&d), &dividido, q).unwrap();
    assert!(ramais.trechos.iter().filter(|t| t.circuitos.len() == 1).all(|t| t.circuitos[0].protecao));
    // A compra de cabo cai o comprimento do trecho, com a folga.
    let metros_de = |s: &Setup| materiais::levantar(&d, &analisar(&d), s).cabos.iter().map(|c| c.metros).sum::<f64>();
    assert!(perto(metros_de(&normal) - metros_de(&dividido), t2.comprimento * normal.fator_materiais, 1e-6));
    // FCA manual vale sobre o de cada trecho.
    let r = rotear_quadro(&d, &analisar(&d), &Setup { fca_manual: Some(0.5), ..Setup::default() }, q).unwrap();
    assert!(r.linhas.iter().all(|l| l.fca == 0.5));
    let mut s = Setup { fca_manual: Some(9.0), ..Setup::default() };
    s.normalizar();
    assert_eq!(s.fca_manual, Some(1.0));
}

#[test]
fn calculo_so_grava_ao_processar_e_acusa_o_que_mudou() {
    use quadro::EstadoCalculo as E;
    let setup = Setup::default();
    let mut d = planta();
    let qd = quadro(&d);
    let linhas = |d: &Drawing| rotear_quadro(d, &analisar(d), &setup, qd).unwrap().linhas;
    assert!(linhas(&d).iter().all(|l| l.estado_calculo() == E::Pendente && l.calculado.is_none()));
    // Consultar não muda o desenho.
    let antes = d.model.get(qd).unwrap().common.xdata.clone();
    rotear_quadro(&d, &analisar(&d), &setup, qd).unwrap();
    assert_eq!(d.model.get(qd).unwrap().common.xdata, antes);
    let feito = editar::processar_calculo(&mut d, &setup, None).unwrap();
    assert_eq!((feito.len(), feito[0].circuitos), (1, linhas(&d).len()));
    assert!(linhas(&d).iter().all(|l| l.estado_calculo() == E::EmDia));
    let guardado = linhas(&d).iter().find(|l| l.circuito.id == "3").unwrap().calculado.unwrap();
    // O projeto muda: a tabela continua mostrando o último cálculo, marcado como desatualizado.
    editar::definir_sobrescrita(&mut d, qd, "3", Some(16.0), None).unwrap();
    let l = linhas(&d).into_iter().find(|l| l.circuito.id == "3").unwrap();
    assert_eq!((l.estado_calculo(), l.calculado), (E::Desatualizado, Some(guardado)));
    assert_ne!(l.escolha.secao, guardado.secao);
    // Processar de novo põe em dia; circuito excluído some do que estava gravado.
    editar::processar_calculo(&mut d, &setup, Some(qd)).unwrap();
    assert!(linhas(&d).iter().all(|l| l.estado_calculo() == E::EmDia));
    editar::excluir_circuito(&mut d, qd, "3").unwrap();
    assert!(!quadro::propriedades(analisar(&d).ponto(qd).unwrap()).contains_key("3"));
    assert!(editar::processar_calculo(&mut d, &setup, Some(Handle(0xFFFF))).is_err());
    // Atualiza as tags que existem, sem mexer em nada antes disso.
    let mut d = planta();
    let qd = quadro(&d);
    let todos = eletrodutos(&d);
    fiacao::regenerar(&mut d, &setup, Some(&todos)).unwrap();
    let (tag, _) = fiacao::tags(&d)[0];
    let bloco = |d: &Drawing| if let EntityKind::Insert(i) = &d.model.get(tag).unwrap().kind { i.block.clone() } else { panic!() };
    let textos = |d: &Drawing| {
        d.blocks[&bloco(d)]
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect::<Vec<_>>()
    };
    let velho = textos(&d);
    editar::definir_sobrescrita(&mut d, qd, "1", Some(25.0), None).unwrap();
    editar::definir_sobrescrita(&mut d, qd, "2", Some(25.0), None).unwrap();
    editar::definir_sobrescrita(&mut d, qd, "3", Some(25.0), None).unwrap();
    assert_eq!(textos(&d), velho);
    editar::processar_calculo(&mut d, &setup, None).unwrap();
    assert!(textos(&d).iter().any(|t| t == "#25"), "{:?}", textos(&d));
}

#[test]
fn grupo_dr_por_circuito_e_na_lista_de_materiais() {
    let mut d = planta();
    let qd = quadro(&d);
    editar::definir_grupo_dr(&mut d, qd, "1", Some(" IDR1 ")).unwrap();
    editar::definir_grupo_dr(&mut d, qd, "2", Some("IDR1")).unwrap();
    let r = rotear_quadro(&d, &analisar(&d), &Setup::default(), qd).unwrap();
    assert_eq!(r.linhas.iter().find(|l| l.circuito.id == "1").unwrap().grupo_dr.as_deref(), Some("IDR1"));
    let m = materiais::levantar(&d, &analisar(&d), &Setup::default());
    assert_eq!(m.grupos_dr, [materiais::ItemContado { nome: "QD-01: IDR1".into(), quantidade: 2 }]);
    editar::definir_grupo_dr(&mut d, qd, "1", None).unwrap();
    editar::definir_grupo_dr(&mut d, qd, "2", Some("  ")).unwrap();
    assert!(materiais::levantar(&d, &analisar(&d), &Setup::default()).grupos_dr.is_empty());
    assert!(editar::definir_grupo_dr(&mut d, qd, "", Some("IDR1")).is_err());
}

#[test]
fn inspetor_le_e_muda_os_atributos_de_uma_insercao() {
    let setup = Setup::default();
    let mut d = metros();
    biblioteca::garantir_base(&mut d, &setup, 1.0);
    let h = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [5.0, 5.0], 0.0, &[("CIRCUITO".to_string(), "1".to_string())], &setup).unwrap();
    let lista = |d: &Drawing| editar::atributos_de(d, h).unwrap();
    let de = |d: &Drawing, t: &str| lista(d).into_iter().find(|a| a.tag == t).unwrap();
    assert_eq!(de(&d, "CIRCUITO").valor, "1");
    assert!(de(&d, "CIRCUITO").visivel && !de(&d, "POTENCIA").visivel);
    assert!(de(&d, "TIPO_PONTO").constante);
    // Valor, visibilidade e posição.
    editar::editar_atributo(&mut d, h, "circuito", Some("7"), None, None, &setup).unwrap();
    assert_eq!(ponto::ler_ponto(&d, h).unwrap().circuito.as_deref(), Some("7"));
    editar::editar_atributo(&mut d, h, "POTENCIA", None, Some(true), None, &setup).unwrap();
    assert!(de(&d, "POTENCIA").visivel);
    editar::editar_atributo(&mut d, h, "CIRCUITO", None, None, Some([9.0, 8.0]), &setup).unwrap();
    assert_eq!((de(&d, "CIRCUITO").x, de(&d, "CIRCUITO").y), (9.0, 8.0));
    // ALTURA segue a regra da legenda.
    editar::editar_atributo(&mut d, h, "ALTURA", Some("1,45"), None, None, &setup).unwrap();
    assert_eq!(de(&d, "ALTURA").valor, "h=1,45m");
    assert_eq!(ponto::ler_ponto(&d, h).unwrap().altura, Some(1.45));
    editar::editar_atributo(&mut d, h, "ALTURA", Some(""), None, None, &setup).unwrap();
    assert_eq!(de(&d, "ALTURA").valor, "");
    // Recusas.
    assert!(editar::editar_atributo(&mut d, h, "TIPO_PONTO", Some("X"), None, None, &setup).is_err());
    assert!(editar::editar_atributo(&mut d, h, "NADA", Some("X"), None, None, &setup).is_err());
    assert!(editar::editar_atributo(&mut d, h, "ALTURA", Some("alto"), None, None, &setup).is_err());
    assert!(editar::editar_atributo(&mut d, h, "CIRCUITO", None, None, Some([f64::NAN, 0.0]), &setup).is_err());
    assert!(editar::editar_atributo(&mut d, Handle(0xFFFF), "CIRCUITO", Some("1"), None, None, &setup).is_err());
    let linha = d.add(&Space::Model, Common::default(), EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(1.0, 0.0, 0.0) })).unwrap();
    assert!(editar::atributos_de(&d, linha).is_none());
}

#[test]
fn letra_nova_entra_nas_teclas_livres() {
    use circuito::comando_com_letra as c;
    assert_eq!(c("", "a", 1), "a");
    assert_eq!(c("a", "b", 1), "b", "uma tecla só: troca");
    assert_eq!(c("a", "b", 2), "ab");
    assert_eq!(c("ab", "a", 2), "ab", "a letra que já tem não muda nada");
    assert_eq!(c("ab", "x", 2), "ax", "sem tecla livre, troca a última");
    assert_eq!(c("a", "", 2), "a");
}

#[test]
fn bloco_no_clique_e_modelo_do_eletroduto() {
    let setup = Setup::default();
    let mut d = metros();
    let t = tug(&mut d, 3.0, 0.0, "1");
    let longe = tug(&mut d, 3.1, 0.0, "1");
    assert_eq!(eletroduto::bloco_em(&d, &setup, [3.02, 0.0], 1.0), Some(t), "o mais perto");
    assert_eq!(eletroduto::bloco_em(&d, &setup, [3.09, 0.0], 1.0), Some(longe));
    assert_eq!(eletroduto::bloco_em(&d, &setup, [9.0, 0.0], 1.0), None);
    // Cada desenho tem o seu modelo.
    let m = eletroduto::ModeloEletroduto {
        instalacao: rota::Instalacao::Piso,
        flecha: 0.0,
        tipo: rota::TipoEletroduto::AcoFogo,
        tracado: eletroduto::Tracado::Reto,
        perfil: None,
    };
    eletroduto::definir_modelo(991_001, m.clone());
    assert_eq!(eletroduto::modelo(991_001), m);
    assert_eq!(eletroduto::modelo(991_002), eletroduto::ModeloEletroduto::default());
    circuito::definir_ativo(991_001, Some((t, "1".into())));
    assert_eq!(circuito::ativo(991_001), Some((t, "1".to_string())));
    assert_eq!(circuito::ativo(991_002), None);
    circuito::definir_ativo(991_001, None);
    assert_eq!(circuito::ativo(991_001), None);
}

#[test]
fn alvo_do_proximo_elecircuito() {
    // O alvo é do desenho que o escolheu: outro desenho aberto (ou outra sessão) não o toma.
    circuito::definir_alvo(901, Some((Handle(5), "3".into())));
    assert_eq!(circuito::tomar_alvo(902), None);
    assert_eq!(circuito::tomar_alvo(901), Some((Handle(5), "3".into())));
    assert_eq!(circuito::tomar_alvo(901), None);
}

#[test]
fn modelo_do_proximo_eleponto_e_do_desenho() {
    let m = biblioteca::Modelo { bloco: "TOM".into(), ..Default::default() };
    biblioteca::definir_modelo(911, Some(m.clone()));
    assert_eq!(biblioteca::modelo(912), None);
    // Fica para as próximas vezes no mesmo desenho.
    assert_eq!(biblioteca::modelo(911), Some(m.clone()));
    assert_eq!(biblioteca::modelo(911), Some(m));
    biblioteca::definir_modelo(911, None);
    assert_eq!(biblioteca::modelo(911), None);
}

#[test]
fn bloco_em_linha_corta_o_eletroduto_sem_caixa_nova() {
    let setup = Setup::default();
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let meio = tug(&mut d, 2.0, 0.0, "1");
    let fim = tug(&mut d, 4.0, 0.0, "1");
    let perto = tug(&mut d, 3.0, 0.2, "1");
    let h = eletroduto::inserir(&mut d, q, fim, rota::Instalacao::Teto, 0.0).unwrap();
    let pontos_antes = analisar(&d).pontos.len();
    // Passar a 20 cm não é estar em linha; o bloco em linha corta o trecho, sem caixa nova.
    assert_eq!(eletroduto::encaixar(&mut d, &setup, 1.0, eletroduto::TOLERANCIA_EM_LINHA, None, None).unwrap(), 1);
    assert_eq!(analisar(&d).pontos.len(), pontos_antes);
    let a = analisar(&d);
    let rede = rede_de(&d);
    assert_eq!(rede.arestas.len(), 2);
    let ligados = |h: Handle| {
        let n = rede.no_de(h).unwrap();
        rede.arestas.iter().filter(|x| x.de == n || x.para == n).count()
    };
    assert_eq!((ligados(q), ligados(meio), ligados(fim), ligados(perto)), (1, 2, 1, 0));
    assert!(rede.arestas.iter().any(|x| x.eletroduto == h));
    assert_eq!(a.pontos.len(), pontos_antes);
    // De novo não corta nada: as pontas não contam.
    assert_eq!(eletroduto::encaixar(&mut d, &setup, 1.0, eletroduto::TOLERANCIA_EM_LINHA, None, None).unwrap(), 0);
    // Clicado, o bloco a 20 cm também liga, pelo raio de captura.
    assert_eq!(eletroduto::encaixar(&mut d, &setup, 1.0, setup.raio_captura, Some(&[perto]), None).unwrap(), 1);
    assert_eq!(rede_de(&d).arestas.len(), 3);
    // Dois blocos no mesmo trecho, com a restrição ao eletroduto novo.
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let (b1, b2) = (tug(&mut d, 2.0, 0.0, "1"), tug(&mut d, 4.0, 0.0, "1"));
    let fim = tug(&mut d, 6.0, 0.0, "1");
    let h = eletroduto::inserir(&mut d, q, fim, rota::Instalacao::Teto, 0.0).unwrap();
    assert_eq!(eletroduto::encaixar(&mut d, &setup, 1.0, eletroduto::TOLERANCIA_EM_LINHA, None, Some(&[h])).unwrap(), 2);
    let rede = rede_de(&d);
    assert_eq!(rede.arestas.len(), 3);
    let ligados = |h: Handle| {
        let n = rede.no_de(h).unwrap();
        rede.arestas.iter().filter(|x| x.de == n || x.para == n).count()
    };
    assert_eq!(ligados(b1) + ligados(b2), 4);
}

#[test]
fn material_define_a_cor_e_o_caminhamento_define_a_linha() {
    use cadcraft_doc::color::Color;
    use rota::{Instalacao as I, TipoEletroduto as E};
    use setup::TipoLinha;
    // Os perfis de fábrica: o PVC flexível tem uma cor só e cada caminhamento a sua linha.
    let s = Setup::default();
    let de = |n: &str| s.perfis_eletroduto.iter().find(|p| p.nome.contains(n)).unwrap();
    assert_eq!(de("Flexível - Teto").cor, de("Flexível - Parede").cor);
    assert_ne!(de("Flexível - Teto").cor, de("PEAD").cor);
    assert_eq!(de("Flexível - Teto").tipo_linha, TipoLinha::Continua);
    assert_eq!(de("Flexível - Parede").tipo_linha, TipoLinha::Tracejada);
    assert_eq!(de("PEAD").tipo_linha, TipoLinha::TracoPonto);
    assert_eq!(de("Aço").tipo_linha, TipoLinha::Continua);
    // Normalizar: a primeira cor do material vale para os outros; a linha errada volta à do caminhamento.
    let mut n = Setup::default();
    n.perfis_eletroduto[1].cor = 2;
    n.perfis_eletroduto[1].tipo_linha = TipoLinha::Continua;
    n.normalizar();
    assert_eq!(n.perfis_eletroduto[1].cor, n.perfis_eletroduto[0].cor);
    assert_eq!(n.perfis_eletroduto[1].tipo_linha, TipoLinha::Tracejada);
    // Pontilhada também vale na parede.
    n.perfis_eletroduto[1].tipo_linha = TipoLinha::Pontilhada;
    n.normalizar();
    assert_eq!(n.perfis_eletroduto[1].tipo_linha, TipoLinha::Pontilhada);
    n.definir_cor_do_material(E::CorrugadoLeve, 2);
    assert_eq!((n.perfis_eletroduto[0].cor, n.perfis_eletroduto[1].cor), (2, 2));
    assert_ne!(n.perfis_eletroduto[2].cor, 2);
    // Sem perfil: a mesma cor em qualquer cota e a linha muda junto com a cota.
    let mut d = metros();
    let q = qd(&mut d, 0.0, 0.0);
    let a = tug(&mut d, 4.0, 0.0, "1");
    let b = tug(&mut d, 8.0, 0.0, "1");
    let t = eletroduto::inserir_com_tipo(&mut d, q, a, I::Teto, 0.0, E::CorrugadoLeve).unwrap();
    let p = eletroduto::inserir_com_tipo(&mut d, a, b, I::Piso, 0.0, E::CorrugadoLeve).unwrap();
    let (et, ep) = (d.model.get(t).unwrap(), d.model.get(p).unwrap());
    assert_eq!(et.common.color, ep.common.color);
    assert_eq!((et.common.linetype.as_str(), ep.common.linetype.as_str()), ("Continuous", "DASHDOT"));
    eletroduto::definir_instalacao(&mut d, t, I::Parede).unwrap();
    assert_eq!(d.model.get(t).unwrap().common.linetype, "DASHED");
    eletroduto::definir_tipo(&mut d, t, E::PeadCorrugado).unwrap();
    assert_eq!(d.model.get(t).unwrap().common.color, Color::Index(E::PeadCorrugado.cor_padrao()));
    // Num desenho em metros, o traço fica com 20 cm, e não com o padrão em milímetros.
    let e = d.model.get(t).unwrap();
    let traco = d.linetype("DASHED").unwrap().pattern[0].length.abs() * e.common.ltscale;
    assert!(perto(traco, 0.20, 1e-9), "{traco}");
    // As camadas não diferem pela cor.
    let cor = |n: &str| d.layer(n).unwrap().color;
    assert_eq!(cor(eletroduto::CAMADA_TETO), cor(eletroduto::CAMADA_PISO));
    // Um desenho antigo entra no padrão ao atualizar.
    let mut e = (**d.model.get(p).unwrap()).clone();
    e.common.color = Color::Index(4);
    e.common.linetype = "Continuous".into();
    d.model.replace(e);
    assert_eq!(eletroduto::padronizar(&mut d, &s), 1);
    assert_eq!(d.model.get(p).unwrap().common.linetype, "DASHDOT");
    assert_eq!(eletroduto::padronizar(&mut d, &s), 0);
}

#[test]
fn familias_de_fabrica_de_tomadas_e_interruptores() {
    use biblioteca::{BLOCOS_BASE, FuncaoTecla, Nivel, interruptor_base, tomada_base};
    let setup = Setup::default();
    let mut d = metros();
    assert!(biblioteca::garantir_base(&mut d, &setup, 1.0));
    let geometria = |d: &Drawing, n: &str| -> (usize, usize, usize, usize) {
        let e = &d.blocks[n].entities;
        let conta = |f: &dyn Fn(&EntityKind) -> bool| e.iter().filter(|x| f(&x.kind)).count();
        (
            conta(&|k| matches!(k, EntityKind::Circle(_))),
            conta(&|k| matches!(k, EntityKind::Hatch(_))),
            conta(&|k| matches!(k, EntityKind::Solid(_))),
            conta(&|k| matches!(k, EntityKind::Line(_))),
        )
    };
    // Interruptores: 3 funções em 1, 2 e 3 seções, no ponto médio e no ponto alto.
    let mut alturas = Vec::new();
    for (corpo, funcao) in [("SIM", FuncaoTecla::Simples), ("PAR", FuncaoTecla::Paralelo), ("INT", FuncaoTecla::Intermediario)] {
        for secoes in 1u8..=3 {
            let nome = match (corpo, secoes) {
                ("SIM", 1) => "INT-SIM".to_string(),
                ("SIM", n) => format!("INT-{n}S"),
                (c, 1) => format!("INT-{c}"),
                (c, n) => format!("INT-{c}-{n}S"),
            };
            for (nome, alto) in [(nome.clone(), false), (format!("{nome}-ALTO"), true)] {
                assert!(BLOCOS_BASE.contains(&nome.as_str()), "{nome}");
                assert_eq!(interruptor_base(&nome), Some(biblioteca::InterruptorBase { funcao, secoes, alto }));
                let (circulos, hachuras, _, _) = geometria(&d, &nome);
                assert_eq!(circulos, usize::from(secoes), "{nome}");
                assert_eq!(hachuras, if funcao == FuncaoTecla::Simples { 0 } else { usize::from(secoes) }, "{nome}");
                let h = biblioteca::inserir_ponto(&mut d, &nome, [0.0, 0.0], 0.0, &[], &setup).unwrap();
                let p = ponto::ler_ponto(&d, h).unwrap();
                assert_eq!(p.tipo_interruptor.unwrap().teclas(), usize::from(secoes), "{nome}");
                alturas.push(rota::altura(&p, &setup));
            }
        }
    }
    assert_eq!(alturas.iter().filter(|a| perto(**a, 1.20, 1e-9)).count(), 9);
    assert_eq!(alturas.iter().filter(|a| perto(**a, 2.10, 1e-9)).count(), 9);
    // Círculos tangentes, empilhados na direção da haste.
    let centros: Vec<(f64, f64)> = d.blocks["INT-3S"]
        .entities
        .iter()
        .filter_map(|e| if let EntityKind::Circle(c) = &e.kind { Some((c.center.y, c.radius)) } else { None })
        .collect();
    assert!(perto(centros[1].0 - centros[0].0, 2.0 * centros[0].1, 1e-9) && perto(centros[2].0 - centros[1].0, 2.0 * centros[0].1, 1e-9));
    // Meia hachura: a metade esquerda do círculo; hachura cheia: o círculo todo.
    let laco = |d: &Drawing, n: &str| -> Vec<(f64, f64, f64)> {
        d.blocks[n]
            .entities
            .iter()
            .find_map(|e| {
                if let EntityKind::Hatch(h) = &e.kind { Some(h.loops[0].vertices.iter().map(|v| (v.p.x, v.p.y, v.bulge)).collect()) } else { None }
            })
            .unwrap()
    };
    let meio = laco(&d, "INT-INT");
    assert_eq!(meio.len(), 2);
    assert!(perto(meio[0].0, 0.0, 1e-9) && perto(meio[1].0, 0.0, 1e-9) && meio[0].2 == -1.0 && meio[1].2 == 0.0, "{meio:?}");
    let cheio = laco(&d, "INT-PAR");
    assert!(cheio[0].2 == 1.0 && cheio[1].2 == 1.0, "{cheio:?}");
    // Tomadas: simples e dupla em 3 alturas, sem hachura, com meia e com ela cheia.
    for (nome, dupla, nivel, h) in [
        ("TOM-SIM-BAIXA", false, Nivel::Baixo, 0.30),
        ("TOM-DUP-BAIXA", true, Nivel::Baixo, 0.30),
        ("TOM-SIM-MEDIA", false, Nivel::Medio, 1.20),
        ("TOM-DUP-MEDIA", true, Nivel::Medio, 1.20),
        ("TOM-SIM-ALTA", false, Nivel::Alto, 2.10),
        ("TOM-DUP-ALTA", true, Nivel::Alto, 2.10),
    ] {
        assert_eq!(tomada_base(nome), Some((dupla, nivel)));
        let triangulos = if dupla { 2 } else { 1 };
        let (_, _, cheios, linhas) = geometria(&d, nome);
        assert_eq!(cheios, if nivel == Nivel::Baixo { 0 } else { triangulos }, "{nome}");
        assert_eq!(linhas, 5 + 3 * triangulos, "{nome}");
        let i = biblioteca::inserir_ponto(&mut d, nome, [1.0, 1.0], 0.0, &[], &setup).unwrap();
        let p = ponto::ler_ponto(&d, i).unwrap();
        assert_eq!(p.tipo, TipoPonto::Tomada);
        assert!(perto(rota::altura(&p, &setup), h, 1e-9), "{nome}");
    }
    // A meia hachura do triângulo é a metade: a área vale metade do triângulo cheio.
    let solido = |n: &str| d.blocks[n].entities.iter().find_map(|e| if let EntityKind::Solid(s) = &e.kind { Some(s.corners) } else { None }).unwrap();
    let area = |c: [Vec3; 4]| ((c[1].x - c[0].x) * (c[2].y - c[0].y) - (c[2].x - c[0].x) * (c[1].y - c[0].y)).abs() / 2.0;
    assert!(perto(area(solido("TOM-SIM-MEDIA")) * 2.0, area(solido("TOM-SIM-ALTA")), 1e-9));
    // Toda família de fábrica tem os dois textos.
    for n in BLOCOS_BASE {
        let f = familia::ler(&d, n).unwrap();
        assert!(!f.legenda.is_empty() && !f.desc_material.is_empty(), "{n}");
    }
    assert_eq!(familia::ler(&d, "TOM-SIM-BAIXA").unwrap().legenda, "Tomada baixa 2P+T 10A - h=0,30m");
    assert_eq!(familia::ler(&d, "TOM-SIM-BAIXA").unwrap().desc_material, "Conjunto 1 Tomada 2P+T 10A + Placa 4x2 + Caixa 4x2");
    assert!(!biblioteca::falta_base(&d));
}

#[test]
fn bloco_base_antigo_e_trocado_e_o_redesenhado_fica() {
    let setup = Setup::default();
    let mut d = metros();
    biblioteca::garantir_base(&mut d, &setup, 1.0);
    // Um desenho antigo: o INT-PAR de antes (círculo com uma linha), sem os blocos novos.
    let antigo = |nome: &str| {
        let mut b = cadcraft_doc::Block::new(nome);
        let l = |a: f64, c: f64, e: f64, g: f64| EntityKind::Line(cadcraft_doc::Line { a: Vec3::new(a, c, 0.0), b: Vec3::new(e, g, 0.0) });
        let mut k = vec![
            l(-0.035, 0.0, 0.035, 0.0),
            l(-0.035, 0.0, -0.035, -0.0475),
            l(0.035, 0.0, 0.035, -0.0475),
            l(0.035, -0.0475, -0.035, -0.0475),
            l(0.0, 0.0, 0.0, 0.06),
        ];
        k.push(EntityKind::Circle(cadcraft_doc::Circle { center: Vec3::new(0.0, 0.135, 0.0), radius: 0.075 }));
        k.push(l(-0.053, 0.082, 0.053, 0.188));
        for (i, x) in k.into_iter().enumerate() {
            b.entities.push(cadcraft_doc::Entity::new(Handle(900_000 + i as u64), x));
        }
        b
    };
    d.blocks.insert("INT-PAR".into(), std::sync::Arc::new(antigo("INT-PAR")));
    // Redesenhado pelo projetista: nada a ver com o padrão.
    let mut meu = cadcraft_doc::Block::new("INT-SIM");
    let circulo = EntityKind::Circle(cadcraft_doc::Circle { center: Vec3::new(0.0, 0.0, 0.0), radius: 0.3 });
    meu.entities.push(cadcraft_doc::Entity::new(Handle(910_000), circulo));
    d.blocks.insert("INT-SIM".into(), std::sync::Arc::new(meu));
    d.blocks.remove("TOM-DUP-ALTA");
    // Uma família antiga, sem os textos, os ganha; a escrita à mão fica.
    let mut f = familia::ler(&d, "QDC").unwrap();
    f.legenda.clear();
    f.desc_material.clear();
    familia::gravar(&mut d, "QDC", Some(&f)).unwrap();
    let mut g = familia::ler(&d, "QDG").unwrap();
    g.legenda = "Meu QDG".into();
    familia::gravar(&mut d, "QDG", Some(&g)).unwrap();
    assert!(biblioteca::garantir_base(&mut d, &setup, 1.0));
    assert!(d.blocks["INT-PAR"].entities.iter().any(|e| matches!(e.kind, EntityKind::Hatch(_))), "o antigo vira o novo");
    assert_eq!(d.blocks["INT-SIM"].entities.len(), 1, "o redesenhado fica");
    assert!(d.blocks.contains_key("TOM-DUP-ALTA"));
    assert!(!familia::ler(&d, "QDC").unwrap().legenda.is_empty());
    assert_eq!(familia::ler(&d, "QDG").unwrap().legenda, "Meu QDG");
}

#[test]
fn legenda_e_descricao_nos_pontos_na_lista_e_na_prancha() {
    let setup = Setup::default();
    let mut d = metros();
    biblioteca::garantir_base(&mut d, &setup, 1.0);
    let a = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [0.0, 0.0], 0.0, &[], &setup).unwrap();
    let b = biblioteca::inserir_ponto(&mut d, "TOM-SIM-BAIXA", [1.0, 0.0], 0.0, &[], &setup).unwrap();
    let c = biblioteca::inserir_ponto(&mut d, "INT-PAR", [2.0, 0.0], 0.0, &[], &setup).unwrap();
    // Herda da família, e o ponto pode ter o seu.
    let p = |d: &Drawing, h: Handle| ponto::ler_ponto(d, h).unwrap();
    assert_eq!(p(&d, a).legenda.as_deref(), Some("Tomada baixa 2P+T 10A - h=0,30m"));
    editar::definir(&mut d, b, ponto::TAGS_LEGENDA, Some("Tomada da bancada")).unwrap();
    editar::definir(&mut d, b, ponto::TAGS_DESC_MATERIAL, Some("Módulo de bancada")).unwrap();
    assert_eq!(p(&d, b).legenda.as_deref(), Some("Tomada da bancada"));
    assert_eq!(p(&d, a).legenda.as_deref(), Some("Tomada baixa 2P+T 10A - h=0,30m"));
    // A lista de materiais conta pela descrição.
    let m = materiais::levantar(&d, &analisar(&d), &setup);
    let qtd = |n: &str| m.conjuntos.iter().find(|i| i.nome == n).map(|i| i.quantidade);
    assert_eq!(qtd("Conjunto 1 Tomada 2P+T 10A + Placa 4x2 + Caixa 4x2"), Some(1));
    assert_eq!(qtd("Módulo de bancada"), Some(1));
    assert_eq!(qtd(&p(&d, c).desc_material.unwrap()), Some(1));
    // Voltar à família.
    editar::definir(&mut d, b, ponto::TAGS_DESC_MATERIAL, None).unwrap();
    assert_eq!(p(&d, b).desc_material, p(&d, a).desc_material);
    // A legenda da prancha: uma linha por tipo, com o símbolo e o nome; regenerar não mexe no lugar.
    let an = analisar(&d);
    assert_eq!(legenda::linhas(&an).len(), 3);
    let h = legenda::inserir(&mut d, &an, [5.0, 5.0], 1.0).unwrap();
    let textos = |d: &Drawing| -> Vec<String> {
        d.blocks[legenda::BLOCO]
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect()
    };
    let t = textos(&d);
    assert!(t.contains(&"LEGENDA DE SÍMBOLOS".to_string()) && t.contains(&"Tomada da bancada".to_string()), "{t:?}");
    let simbolos = d.blocks[legenda::BLOCO].entities.iter().filter(|e| matches!(e.kind, EntityKind::Insert(_))).count();
    assert_eq!(simbolos, 3);
    assert_eq!(analisar(&d).pontos.len(), 3, "a legenda não é ponto elétrico");
    editar::definir(&mut d, b, ponto::TAGS_LEGENDA, Some("Outra")).unwrap();
    let reg = fiacao::regenerar(&mut d, &setup, None).unwrap();
    assert_eq!(reg.legendas, 1);
    assert!(textos(&d).contains(&"Outra".to_string()));
    assert_eq!(legenda::inserida(&d), Some(h));
    assert!(legenda::inserir(&mut d, &analisar(&Drawing::new_metric()), [f64::NAN, 0.0], 1.0).is_err());
    // Texto grande demais na família é recusado.
    let mut f = familia::ler(&d, "QDG").unwrap();
    f.legenda = "x".repeat(201);
    assert!(familia::gravar(&mut d, "QDG", Some(&f)).is_err());
}
