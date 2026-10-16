use std::sync::Arc;

use cadcraft_doc::*;
use cadcraft_geom::Vec3;

use crate::nbr5410::{self, Config};
use crate::ponto::{fases, numero, potencia_va};
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

/// Uma sala com quadro, luz, interruptor, duas TUG, um chuveiro, uma caixa e uma porta.
fn planta() -> Drawing {
    let mut d = Drawing::new_metric();
    inserir(&mut d, "QD", 0.0, 0.0, &[("TIPO_PONTO", "QUADRO"), ("TAG", "QD-01"), ("TENSAO", "220/127V"), ("NUMERO_FASES", "2")]);
    #[rustfmt::skip]
    let luz = [("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a"), ("TAG", "LUM-01")];
    inserir(&mut d, "CAIXA_LUZ", 4.0, 0.0, &luz);
    #[rustfmt::skip]
    let int = [("TIPO_PONTO", "INTERRUPTOR"), ("POTENCIA", "0W"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "1"), ("COMANDO", "a")];
    inserir(&mut d, "INT_SIMPLES", 4.0, 3.0, &int);
    for x in [0.0, 1.0] {
        inserir(
            &mut d,
            "TOMADA",
            x,
            3.0,
            &[("TIPO_PONTO", "TUG"), ("POTENCIA", "600VA"), ("TENSAO", "127V"), ("NUMERO_FASES", "1"), ("CIRCUITO", "2")],
        );
    }
    #[rustfmt::skip]
    let chuveiro = [("TIPO_PONTO", "TUE"), ("POTENCIA", "5,4kW"), ("TENSAO", "220V"), ("NUMERO_FASES", "2"), ("CIRCUITO", "3")];
    inserir(&mut d, "TUE_CHUVEIRO", 6.0, 3.0, &chuveiro);
    inserir(&mut d, "CAIXA_PASSAGEM", 2.0, 1.5, &[("TIPO_PONTO", "CAIXA DE PASSAGEM")]);
    // Um bloco de arquitetura sem atributos elétricos não entra, mesmo com nome parecido.
    inserir(&mut d, "PORTA_INTERNA", 9.0, 9.0, &[]);
    d
}

#[test]
fn classifica_pelo_tipo_e_pelo_nome() {
    let c = TipoPonto::classificar;
    assert_eq!(c("TUG"), Some(TipoPonto::Tomada));
    assert_eq!(c("Tomada"), Some(TipoPonto::Tomada));
    assert_eq!(c("TOMADA_TUE"), Some(TipoPonto::TomadaEspecifica));
    assert_eq!(c("TUE"), Some(TipoPonto::TomadaEspecifica));
    assert_eq!(c("ILUM"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("caixa de luz"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("Luminária"), Some(TipoPonto::Iluminacao));
    assert_eq!(c("INT_SIMPLES"), Some(TipoPonto::Interruptor));
    assert_eq!(c("Interruptor paralelo"), Some(TipoPonto::Interruptor));
    assert_eq!(c("QUADRO"), Some(TipoPonto::Quadro));
    assert_eq!(c("QD-01"), Some(TipoPonto::Quadro));
    assert_eq!(c("CAIXA_PASSAGEM"), Some(TipoPonto::Caixa));
    assert_eq!(c("MESA"), None);
}

#[test]
fn le_numeros_potencias_e_fases() {
    assert_eq!(numero("127V"), Some(127.0));
    assert_eq!(numero("220/127V"), Some(220.0));
    assert_eq!(numero("1,5"), Some(1.5));
    assert_eq!(numero("1.500 W"), Some(1500.0));
    assert_eq!(numero("2.5"), Some(2.5));
    assert_eq!(numero("sem número"), None);
    assert_eq!(potencia_va("600VA"), Some(600.0));
    assert_eq!(potencia_va("100 W"), Some(100.0));
    assert_eq!(potencia_va("5,4kW"), Some(5400.0));
    assert_eq!(potencia_va("1.5 KVA"), Some(1500.0));
    assert_eq!(potencia_va("0W"), Some(0.0));
    assert_eq!(fases("2"), Some(2));
    assert_eq!(fases("TRIFÁSICO"), Some(3));
    assert_eq!(fases("mono"), Some(1));
    assert_eq!(fases("7"), None);
}

#[test]
fn le_os_pontos_da_planta() {
    let a = analisar(&planta());
    assert_eq!(a.pontos.len(), 7, "a porta não é ponto elétrico");
    assert_eq!(a.contar(TipoPonto::Quadro), 1);
    assert_eq!(a.contar(TipoPonto::Tomada), 2);
    assert_eq!(a.contar(TipoPonto::TomadaEspecifica), 1);
    assert_eq!(a.contar(TipoPonto::Caixa), 1);
    // A luminária tem COMANDO e continua sendo carga, não interruptor.
    let luz = a.pontos.iter().find(|p| p.bloco == "CAIXA_LUZ").unwrap();
    assert_eq!(luz.tipo, TipoPonto::Iluminacao);
    assert_eq!(luz.potencia, Some(100.0));
    assert!(luz.avisos().is_empty(), "{:?}", luz.avisos());
    let int = a.pontos.iter().find(|p| p.tipo == TipoPonto::Interruptor).unwrap();
    assert!(int.avisos().is_empty(), "{:?}", int.avisos());
}

#[test]
fn agrupa_os_circuitos() {
    let a = analisar(&planta());
    let ids: Vec<&str> = a.circuitos.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["1", "2", "3"]);
    let c1 = &a.circuitos[0];
    assert_eq!((c1.natureza, c1.potencia, c1.cargas, c1.interruptores), (Natureza::Iluminacao, 100.0, 1, 1));
    let c2 = &a.circuitos[1];
    assert_eq!((c2.natureza, c2.potencia, c2.cargas), (Natureza::Tug, 1200.0, 2));
    let c3 = &a.circuitos[2];
    assert_eq!((c3.natureza, c3.potencia, c3.fases, c3.tensao), (Natureza::Tue, 5400.0, 2, Some(220.0)));
    assert!(a.circuitos.iter().all(|c| c.avisos.is_empty()));
}

#[test]
fn dimensiona_condutor_e_disjuntor() {
    let a = analisar(&planta());
    let cfg = Config::default();
    let d: Vec<_> = a.circuitos.iter().map(|c| nbr5410::dimensionar(c, &cfg)).collect();
    // Iluminação: 1,5 mm² mínimo.
    assert_eq!((d[0].secao, d[0].disjuntor), (Some(1.5), Some(6.0)));
    // TUG: 1200 VA / 127 V = 9,45 A, 2,5 mm² mínimo.
    assert!((d[1].ib - 9.449).abs() < 0.01);
    assert_eq!((d[1].secao, d[1].disjuntor), (Some(2.5), Some(10.0)));
    // Chuveiro: 5400 VA / 220 V = 24,5 A passa dos 24 A do 2,5 mm²: vai a 4 mm² com 25 A.
    assert!((d[2].ib - 24.545).abs() < 0.01);
    assert_eq!((d[2].secao, d[2].disjuntor, d[2].iz), (Some(4.0), Some(25.0), Some(32.0)));
    // Com agrupamento 0,7 o 4 mm² cai para 22,4 A e o circuito sobe para 6 mm².
    let agrupado = nbr5410::dimensionar(&a.circuitos[2], &Config { fca: 0.7, ..cfg });
    assert_eq!(agrupado.secao, Some(6.0));
    assert_eq!(agrupado.disjuntor, Some(25.0));
}

#[test]
fn trifasico_usa_tensao_de_linha() {
    assert!((nbr5410::corrente_projeto(10_000.0, 380.0, 3) - 15.193).abs() < 0.01);
    assert_eq!(nbr5410::condutores_carregados(3), 3);
}

#[test]
fn monta_e_balanceia_o_quadro() {
    let a = analisar(&planta());
    let q = dimensionar_quadro(&a, &Config::default());
    assert_eq!(q.quadro.as_deref(), Some("QD-01"));
    assert_eq!(q.fases_quadro, 2);
    assert_eq!(q.potencia_total, 6700.0);
    // O chuveiro bifásico divide 5400 VA em A e B; a TUG vai para A, a luz para B.
    assert_eq!(q.linhas[2].fases_texto(), "A+B");
    assert_eq!(q.linhas[1].fases_texto(), "A");
    assert_eq!(q.linhas[0].fases_texto(), "B");
    assert_eq!(q.carga_por_fase, [3900.0, 2800.0, 0.0]);
    assert!((q.desequilibrio - 28.2).abs() < 0.1);
    assert!(q.avisos.is_empty(), "{:?}", q.avisos);
    // O resultado vira JSON para o canal de controle.
    let json = serde_json::to_value(&q).unwrap();
    assert_eq!(json["linhas"][2]["dimensionamento"]["disjuntor"], 25.0);
}

#[test]
fn atributo_constante_vem_da_definicao_do_bloco() {
    let mut d = Drawing::new_metric();
    let mut def = Block::new("TOMADA_ALTA");
    def.entities.push(Entity::new(Handle(900), EntityKind::AttDef(atributo("TIPO_PONTO", "TUG", true))));
    d.blocks.insert("TOMADA_ALTA".into(), Arc::new(def));
    inserir(&mut d, "TOMADA_ALTA", 0.0, 0.0, &[("POTENCIA", "100VA"), ("CIRCUITO", "4")]);
    let a = analisar(&d);
    assert_eq!(a.pontos.len(), 1);
    assert_eq!(a.pontos[0].tipo, TipoPonto::Tomada);
    assert!(a.pontos[0].tipo_informado);
}

#[test]
fn avisa_circuito_misto_e_ponto_incompleto() {
    let mut d = Drawing::new_metric();
    inserir(&mut d, "LUZ", 0.0, 0.0, &[("TIPO_PONTO", "ILUM"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    inserir(&mut d, "TUG", 1.0, 0.0, &[("TIPO_PONTO", "TUG"), ("POTENCIA", "100VA"), ("CIRCUITO", "1")]);
    let a = analisar(&d);
    assert_eq!(a.circuitos[0].natureza, Natureza::Misto);
    assert!(a.circuitos[0].avisos.iter().any(|w| w.contains("9.5.3.2")));
    // Sem TENSAO nem NUMERO_FASES nos pontos.
    assert_eq!(a.avisos_pontos().len(), 4);
    // E o dimensionamento cai na tensão padrão avisando.
    let dim = nbr5410::dimensionar(&a.circuitos[0], &Config::default());
    assert_eq!(dim.tensao, 127.0);
    assert_eq!(dim.avisos.len(), 1);
}
