// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Free Elétrica: as ferramentas de projeto elétrico que trabalham no desenho.
//!
//! ELEPONTO: insere os pontos, alinhados à parede mais próxima (Tab gira).
//! ELECIRCUITO: quadro, número do circuito e os pontos, um a um, até Enter.
//! ELECOMANDO: o interruptor e as luminárias que ele comanda, até Enter.
//! ELEREMOVERPONTO: tira pontos do circuito.
//! ELEELETRODUTO: liga blocos elétricos em sequência; clicar no meio de um eletroduto divide o
//! trecho com uma caixa de passagem.
//! ELEFIACAO: cria as tags de fiação nos eletrodutos; ELEREGEN atualiza tudo o que é gerado sem
//! mexer na posição que o projetista deu às tags.
//! ELEQUADRO_TABELA: desenha o quadro de cargas num ponto da planta.
//!
//! Na primeira ferramenta usada num desenho virgem, as camadas e os blocos base entram sem
//! perguntar (`ele::biblioteca::garantir_base`). Os valores vão para os atributos do bloco quando
//! existem e, senão, para o XDATA `FREE_ELETRICA` (`ele::editar`). Cada comando é um passo de
//! desfazer. Lançar não dimensiona: o cálculo, as tags e as tabelas só mudam com
//! `eletrica.dimensionar` e ELEREGEN.
//!
//! Os comandos `eletrica.*` dão em JSON tudo o que a paleta e os painéis mostram: o motor inteiro é
//! acessível sem a interface (linha de comando, scripts, canal de controle, `cadcraft-cli`, MCP).

use cadcraft_doc::{EntityKind, Handle};
use free_eletrica::rota::Instalacao;
use free_eletrica::{self as ele, PontoEletrico, TipoPonto};
use serde_json::{Value, json};

use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("elecircuito", "Electrical Circuit", run_circuito)
            .params("{quadro: hex, circuito?: string (default: the next free number), pontos: [hex], descricao?, esquema?: \"F+N\" | \"F+N+T\" | \"2F+T\" | \"3F+N+T\", tensao?: V} → {circuito, pontos}")
            .interactive(|_| Ok(Box::new(CircuitoM::default()))),
        CommandSpec::new("eleremoverponto", "Remove Points from Circuit", run_remover).params("{pontos: [hex]} → {removidos}").interactive(|_| Ok(Box::new(RemoverM::default()))),
        CommandSpec::new("elecomando", "Switches and Lamps", run_comando)
            .params("{interruptor: hex, luminarias: [hex], comando?: letter (default: the next free key of the switch)} → {comando, circuito, luminarias}")
            .interactive(|_| Ok(Box::new(ComandoM::default()))),
        CommandSpec::new("elefiacao", "Wiring Tags", run_fiacao)
            .params("{quadro?: hex | TAG, eletrodutos?: [hex], limpar?: bool (erase the tags)} (default: every conduit) → {tags, criadas, apagadas, tabelas, avisos}")
            .interactive(|_| Ok(Box::new(FiacaoM))),
        CommandSpec::new("eleregen", "Regenerate Electrical Drawing", run_regen).params("{} → updates tags and tables in place, keeping their positions: {tags, apagadas, tabelas, avisos}"),
        CommandSpec::new("elequadro_tabela", "Load Schedule Table", run_tabela)
            .params("{quadro?: hex | TAG, ponto: [x, y]} → draws the load schedule as a block at the point (moves it if the board already has one): {tabela}")
            .interactive(|_| Ok(Box::new(TabelaM::default()))),
        CommandSpec::new("eleeletroduto", "Insert Conduit", run_eletroduto)
            .params("{de: hex | [x, y], para: hex | [x, y], instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\", tipo?: PVC_CORR_LIGHT | PVC_CORR_HEAVY | PVC_RIGID | HDPE_CORR | STEEL_ELECTRO | STEEL_HOT_DIP, perfil?: profile name (applies its type, route, linetype, color and diameters), tracado?: STRAIGHT | ANGLE45 | ARC, desvios?: [[x, y]], diagonalPrimeiro?: bool, flecha?: number (arc bulge as a fraction of the chord: + left, - right, 0 straight; default 0.15)} (a point on a conduit splits it with a pull box) → {eletroduto}")
            .interactive(|_| Ok(Box::new(EletrodutoM::default()))),
        CommandSpec::new("eleponto", "Insert Electrical Point", run_ponto)
            .params("{bloco, pontos: [[x, y]], rotacao?: degrees, rotacaoAuto?: bool (align to the nearest wall), altura?: m, quadro?: hex | TAG, valores?: {TAG: value}} → {pontos: [hex]}")
            .interactive(|_| Ok(Box::new(PontoM::default()))),
        CommandSpec::new("eletrica.ponto.altura", "Point Elevation", run_altura)
            .params("{pontos: [hex], altura: m | null} (at the project's standard height or null: the visible ALTURA stays empty; otherwise the exact value goes to XDATA and the plan shows h=1,45m) → {pontos}"),
        CommandSpec::new("eletrica.ponto.demanda", "Point Demand", run_ponto_demanda)
            .params("{pontos: [hex], classe?: ILUM_TUG | AQUECIMENTO | AR | MOTORES | ELETRODOMESTICOS | FATOR_MANUAL | null, fator?: 0-1 | null} (null: back to the family's or to the utility's table) → {pontos}"),
        CommandSpec::new("eletrica.familia.ler", "Block Family", run_familia_ler)
            .params("{bloco? (default: the block being edited)} → the family, or null")
            .noundo(),
        CommandSpec::new("eletrica.familia.definir", "Family Properties", run_familia_definir)
            .params("{bloco? (default: the block being edited), categoria?: NONE | ELECTRICAL_FIXTURE | LIGHTING_FIXTURE | LIGHTING_DEVICE | DATA_DEVICE | FIRE_SECURITY_DEVICE | ELECTRICAL_EQUIPMENT | TELECOM_ENCLOSURE | JUNCTION_BOX, dominio?: POWER | DATA_TELECOM | FIRE_SECURITY | INFRA, refAltura?: LOW | MEDIUM | HIGH | CEILING | FLOOR | FIXED, cotaFixa?: m, caixa?: NONE | 4X2 | 4X4 | OCTAGONAL | PULL_15X15 | PULL_20X20 | PULL_30X30, potencia?: VA | null, fp?: 0-1 | null, teclas?: 1-3, classeDemanda?: ILUM_TUG | AQUECIMENTO | AR | MOTORES | ELETRODOMESTICOS | FATOR_MANUAL | null, fatorDemanda?: 0-1 | null (null: automatic, from the utility's table), remover?: bool} → the family"),
        CommandSpec::new("eletrica.familia.geometria", "Family Base Geometry", run_familia_geometria)
            .params("{forma: tomada | comando | luminaria | contorno4x2 | contorno4x4} → inserts it into the block being edited: {entidades}")
            .enabled(no_editor_de_blocos),
        CommandSpec::new("eletrica.familia.atributos", "Family Attributes", run_familia_atributos)
            .params("{categoria? (default: the family's)} → creates the standard ATTDEFs missing from the block being edited: {criados}")
            .enabled(no_editor_de_blocos),
        CommandSpec::new("elefamilia_conector", "Family Connector", run_familia_conector)
            .params("{ponto: [x, y]} → moves the block being edited so the point is its origin, the insertion and conduit connection point: {conector}")
            .enabled(no_editor_de_blocos)
            .interactive(|_| Ok(Box::new(ConectorM))),
        CommandSpec::new("eletrica.materiais", "Materials List", run_materiais)
            .params("{} → {cabos: [{secao, tipo, metros}], eletrodutos: [{tipo, diametro, metros}], caixas, pontos, disjuntores, pontosSemCaixa, eletrodutosSoltos, avisos}")
            .noundo(),
        CommandSpec::new("eletrica.dimensionar", "Size Board", run_dimensionar)
            .params("{quadro?: hex | TAG (default: every board), compartilharPe?: bool, temperaturaAmbiente?: °C, fcaManual?: number | null} → applies the options, sizes the circuits, stores the result on the boards and updates tags and tables: {quadros: [{tag, circuitos, avisos}], circuitos, avisos}"),
        CommandSpec::new("eletrica.quadro.equilibrar", "Balance Phases", run_equilibrar)
            .params("{quadro?: hex | TAG} → unlocks the phases of every circuit, so the balancing applies to all: {destravados, desequilibrio}"),
        CommandSpec::new("eletrica.atributos", "Block Attributes", run_atributos)
            .params("{ponto: hex} → [{tag, valor, visivel, x, y, constante}]")
            .noundo(),
        CommandSpec::new("eletrica.atributo.definir", "Edit Block Attribute", run_atributo_definir)
            .params("{ponto: hex, tag, valor?: text, visivel?: bool, posicao?: [x, y]} (ALTURA follows the project's height rule) → {ponto, tag}"),
        CommandSpec::new("eletrica.bootstrap", "Prepare Electrical Drawing", run_bootstrap).params("{} → creates the missing base layers and blocks: {mudou}"),
        CommandSpec::new("eletrica.blocos", "Electrical Point Blocks", run_blocos)
            .params("{} → [{nome, tipo, inseridos, atributos: [{tag, padrao, prompt, constante}]}]")
            .noundo(),
        CommandSpec::new("eletrica.ponto.modelo", "Point Template", run_modelo)
            .params("{bloco, rotacao?: degrees, rotacaoAuto?: bool (default true), altura?: m, quadro?: hex | TAG | null, valores?: {TAG: value}} → what ELEPONTO inserts, also while it runs")
            .noundo(),
        CommandSpec::new("eletrica.ponto.excluir", "Delete Electrical Points", run_excluir_pontos).params("{pontos: [hex]} → {excluidos}"),
        CommandSpec::new("eletrica.eletroduto.modelo", "Conduit Template", run_modelo_eletroduto)
            .params("{instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\", tipo?: PVC_CORR_LIGHT | PVC_CORR_HEAVY | PVC_RIGID | HDPE_CORR | STEEL_ELECTRO | STEEL_HOT_DIP, perfil?: profile name | null, tracado?: STRAIGHT | ANGLE45 | ARC, flecha?: number} → what ELEELETRODUTO uses, also while it runs")
            .noundo(),
        CommandSpec::new("eletrica.eletroduto.dividir", "Split Conduit", run_dividir).params("{eletroduto: hex, ponto: [x, y]} → splits it with a pull box: {caixa}"),
        CommandSpec::new("eletrica.eletroduto.definir", "Conduit Properties", run_definir_eletroduto)
            .params("{eletroduto: hex, diametro?: mm | null (a number locks it; null: back to automatic), instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\", tipo?: conduit type code, perfil?: profile name} → {eletroduto}"),
        CommandSpec::new("eletrica.eletrodutos", "Conduit Summary", run_eletrodutos).params("{} → {trechos, soltos, avisos}").noundo(),
        CommandSpec::new("eletrica.inspecionar", "Inspect Electrical Project", run_inspecionar)
            .params("{} → [{handle, pos, nivel: vermelho | laranja | amarelo | verde, motivo, eletroduto}]")
            .noundo(),
        CommandSpec::new("eletrica.analisar", "Electrical Analysis", run_analisar).params("{} → {pontos, circuitos}").noundo(),
        CommandSpec::new("eletrica.rotear", "Wiring Summary", run_rotear)
            .params("{quadro?: hex | TAG (default: the first board)} → the routing and sizing summary of the board's circuits")
            .noundo(),
        CommandSpec::new("eletrica.sobrescrever", "Override Circuit", run_sobrescrever)
            .params("{quadro?: hex | TAG, circuito, secao?: mm², disjuntor?: A} (no value: back to the automatic choice) → {quadro, circuito}"),
        CommandSpec::new("eletrica.quadro", "Board Settings", run_quadro)
            .params("{quadro?: hex | TAG, sistema?: \"F+N+T\" | \"2F+N+T\" | \"2F+T\" | \"3F+N+T\" | null, tensaoNominal?: \"127/220\" | \"220/380\" | null, cabo?: FLEX_PVC_70 | UNI_PVC_70 | HEPR_XLPE_90 | LSZH_70 | LSZH_HEPR_90 | null, alimentadoPor?: \"CONCESSIONARIA\" | hex | TAG | null, isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null (from drawings before the board's cable)} (null: back to what the block says) → {quadro, isolacao, config}"),
        CommandSpec::new("eletrica.cargas", "Load Schedule", run_cargas)
            .params("{quadro?: hex | TAG} → [{id, descricao, circuito}] (circuits with points and the ones created empty)")
            .noundo(),
        CommandSpec::new("eletrica.circuito.criar", "New Circuit", run_criar)
            .params("{quadro?: hex | TAG, circuito?: string (default: the next free number), descricao?} → {quadro, circuito}"),
        CommandSpec::new("eletrica.circuito.descrever", "Describe Circuit", run_descrever).params("{quadro?: hex | TAG, circuito, descricao} → {quadro, circuito}"),
        CommandSpec::new("eletrica.circuito.excluir", "Delete Circuit", run_excluir)
            .params("{quadro?: hex | TAG, circuito} (its points lose the circuit) → {quadro, circuito, pontos}"),
        CommandSpec::new("eletrica.circuito.juntar", "Merge Circuits", run_juntar)
            .params("{quadro?: hex | TAG, de, para} (the points of `de` go to `para`) → {quadro, de, para, pontos}"),
        CommandSpec::new("eletrica.circuito.definir", "Circuit Properties", run_definir).params(
            "{quadro?: hex | TAG, circuito, grupoDr?: text | null, cabo?: FLEX_PVC_70 | UNI_PVC_70 | HEPR_XLPE_90 | LSZH_70 | LSZH_HEPR_90 | null, caboNome?: catalog name | null, isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null, esquema?: \"F+N\" | \"F+N+T\" | \"2F+T\" | \"3F+N+T\" | null, tensao?: V | null, fases?: \"R\" | \"RS\" ... | null (a value locks the phases), descricao?} (null: back to automatic) → {quadro, circuito}. Load type and power factor come from the block attributes TIPO_CARGA and FP.",
        ),
        CommandSpec::new("eletrica.demanda.tabelas", "Demand Tables", run_tabelas_demanda)
            .params("{} → the factory and custom demand tables (id, nome, fonte, edificacoes, aparelhos, aquecimento, arCondicionado, bombas, motores, propria)")
            .enabled(always)
            .noundo(),
        CommandSpec::new("eletrica.demanda.salvar", "Save Demand Table", run_salvar_tabela)
            .params("{tabela: {id?, nome, fonte?, edificacoes: [{nome, modo: acumulada | totalNaFaixa, faixas: [{ateKw, fator}]}], aparelhos?, limiteAparelhoKw?, aquecimento?, arCondicionado?, bombas?, motores?}, usar?: bool} → creates or updates a custom table: {id}")
            .enabled(always)
            .noundo(),
        CommandSpec::new("eletrica.demanda.excluir", "Delete Demand Table", run_excluir_tabela)
            .params("{id} → deletes a custom table: {excluida}")
            .enabled(always)
            .noundo(),
        CommandSpec::new("eletrica.circuito.alvo", "Circuit Target", run_alvo)
            .params("{quadro?: hex | TAG, circuito} → the next ELECIRCUITO goes straight to picking points for this circuit")
            .noundo(),
        CommandSpec::new("eletrica.setup", "Electrical Settings", run_setup)
            .params("{} reads; {sistema?, temperaturaAmbiente?, peDireito?, quedaAdmissivel?, ...} writes → the settings")
            .enabled(always)
            .noundo(),
    ]
}

// ---------------------------------------------------------------- utilitários

fn erro(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn analise(s: &Session) -> Result<ele::Analise> {
    Ok(ele::analisar(s.doc()?))
}

fn json_de<T: serde::Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|e| erro(e.to_string()))
}

/// Metros por unidade do desenho.
fn metros(s: &Session) -> Result<f64> {
    let (m, _) = ele::setup::atual().unidade.metros(s.doc()?.header.i64("INSUNITS", 0));
    Ok(if m > 0.0 { m } else { 1.0 })
}

/// Auto-bootstrap: as camadas e os blocos base entram sem perguntar, num desenho que não os tem.
fn preparar(s: &mut Session) -> Result<bool> {
    if !ele::biblioteca::falta_base(s.doc()?) {
        return Ok(false);
    }
    let m = metros(s)?;
    Ok(ele::biblioteca::garantir_base(s.doc_mut()?, &ele::setup::atual(), m))
}

/// O quadro por handle ou TAG; sem nada, o primeiro do desenho.
fn achar_quadro(a: &ele::Analise, v: Option<&Value>) -> Result<Handle> {
    let quadros = a.quadros();
    let achado = match v.and_then(Value::as_str) {
        None => quadros.first().copied(),
        Some(t) => quadros
            .iter()
            .find(|q| Handle::parse_hex(t) == Some(q.handle))
            .or_else(|| quadros.iter().find(|q| q.tag.as_deref().is_some_and(|tag| tag.eq_ignore_ascii_case(t))))
            .copied(),
    };
    achado.map(|q| q.handle).ok_or_else(|| erro("nenhum quadro (TIPO_PONTO = QUADRO) com esse nome no desenho"))
}

/// Um texto obrigatório (aceita número: `"circuito": 3`).
fn texto(cmd: &str, p: &Value, k: &str) -> Result<String> {
    p.get(k)
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad(cmd, format!("`{k}` is required")))
}

fn handle(cmd: &str, p: &Value, k: &str) -> Result<Handle> {
    p.get(k).and_then(Value::as_str).and_then(Handle::parse_hex).ok_or_else(|| bad(cmd, format!("`{k}` (handle) is required")))
}

fn handles(cmd: &str, p: &Value, k: &str) -> Result<Vec<Handle>> {
    let lista = p.get(k).and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("`{k}` ([handle]) is required")))?;
    lista.iter().map(|v| v.as_str().and_then(Handle::parse_hex).ok_or_else(|| bad(cmd, format!("`{k}` holds handles, got {v}")))).collect()
}

/// `[x, y]` com números finitos.
fn ponto_de(v: &Value) -> Option<[f64; 2]> {
    let (x, y) = (v.get(0)?.as_f64()?, v.get(1)?.as_f64()?);
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

fn instalacao_de(cmd: &str, v: &Value) -> Result<Instalacao> {
    v.as_str().and_then(Instalacao::de_codigo).ok_or_else(|| bad(cmd, "`instalacao` must be \"CEILING\", \"FLOOR\" or \"WALL\""))
}

fn tipo_de(cmd: &str, v: &Value) -> Result<ele::rota::TipoEletroduto> {
    v.as_str()
        .and_then(ele::rota::TipoEletroduto::de_codigo)
        .ok_or_else(|| bad(cmd, "`tipo` must be PVC_CORR_LIGHT, PVC_CORR_HEAVY, PVC_RIGID, HDPE_CORR, STEEL_ELECTRO or STEEL_HOT_DIP"))
}

fn tracado_de(cmd: &str, v: &Value) -> Result<ele::eletroduto::Tracado> {
    v.as_str().and_then(ele::eletroduto::Tracado::de_codigo).ok_or_else(|| bad(cmd, "`tracado` must be STRAIGHT, ANGLE45 or ARC"))
}

fn numero_finito(cmd: &str, v: &Value, k: &str) -> Result<f64> {
    v.as_f64().filter(|f| f.is_finite()).ok_or_else(|| bad(cmd, format!("`{k}` must be a number")))
}

// ---------------------------------------------------------------- leituras e quadro

fn run_analisar(s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&analise(s)?)
}

fn run_rotear(s: &mut Session, p: &Value) -> Result<Value> {
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    json_de(&ele::rotear_quadro(s.doc()?, &a, &ele::setup::atual(), q).map_err(erro)?)
}

fn run_inspecionar(s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::inspecao::inspecionar(s.doc()?, &analise(s)?, &ele::setup::atual()))
}

fn run_eletrodutos(s: &mut Session, _p: &Value) -> Result<Value> {
    let (trechos, soltos, avisos) = ele::fiacao::todos_os_trechos(s.doc()?, &analise(s)?, &ele::setup::atual());
    Ok(json!({ "trechos": json_de(&trechos)?, "soltos": json_de(&soltos)?, "avisos": avisos }))
}

fn run_sobrescrever(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let circuito = p
        .get("circuito")
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())))
        .ok_or_else(|| bad("eletrica.sobrescrever", "`circuito` is required"))?;
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    if let Some(sec) = num("secao")
        && ele::nbr5410::indice_secao(sec).is_none()
    {
        return Err(bad("eletrica.sobrescrever", format!("section {sec} mm² is not in the table")));
    }
    ele::editar::definir_sobrescrita(s.doc_mut()?, q, &circuito, num("secao"), num("disjuntor")).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "circuito": circuito }))
}

fn run_quadro(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.quadro";
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let isolacao = match p.get("isolacao") {
        None | Some(Value::Null) => None,
        Some(v) => {
            Some(v.as_str().and_then(ele::nbr5410::Isolacao::de_codigo).ok_or_else(|| bad(CMD, "`isolacao` must be \"PVC\", \"EPR\" or \"XLPE\""))?)
        }
    };
    // Tudo é conferido antes de gravar qualquer coisa. `None` não mexe, `Some(None)` volta ao do bloco.
    let sistema = p.get("sistema").map(|_| codigo_de(CMD, p, "sistema", ele::quadro::SistemaQuadro::de_codigo)).transpose()?;
    let tensao = p.get("tensaoNominal").map(|_| codigo_de(CMD, p, "tensaoNominal", ele::quadro::TensaoNominal::de_codigo)).transpose()?;
    let cabo = p.get("cabo").map(|_| codigo_de(CMD, p, "cabo", ele::nbr5410::TipoCabo::de_codigo)).transpose()?;
    let alimentado = match p.get("alimentadoPor") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_str().ok_or_else(|| bad(CMD, "`alimentadoPor` must be text or null"))?.to_string())),
    };
    let doc = s.doc_mut()?;
    if let Some(a) = &alimentado {
        ele::editar::definir_alimentacao(doc, q, a.as_deref()).map_err(|e| bad(CMD, e))?;
    }
    if let Some(v) = sistema {
        ele::editar::definir_sistema_quadro(doc, q, v).map_err(erro)?;
    }
    if let Some(v) = tensao {
        ele::editar::definir_tensao_nominal(doc, q, v).map_err(erro)?;
    }
    if let Some(v) = cabo {
        ele::editar::definir_cabo_quadro(doc, q, v).map_err(erro)?;
    }
    if p.get("isolacao").is_some() || (sistema.is_none() && tensao.is_none() && cabo.is_none() && alimentado.is_none()) {
        ele::editar::definir_isolacao(doc, q, isolacao).map_err(erro)?;
    }
    let config = ele::ponto::ler_ponto(s.doc()?, q).map(|x| ele::quadro::config_do_quadro(&x)).unwrap_or_default();
    Ok(json!({ "quadro": q.hex(), "isolacao": isolacao.map(|i| i.codigo()), "config": json_de(&config)? }))
}

fn run_cargas(s: &mut Session, p: &Value) -> Result<Value> {
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    json_de(&ele::quadro::circuitos_do_quadro(&a, q))
}

fn run_criar(s: &mut Session, p: &Value) -> Result<Value> {
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    let id = match p.get("circuito") {
        None | Some(Value::Null) => ele::circuito::proximo_circuito(&a.pontos, q),
        _ => texto("eletrica.circuito.criar", p, "circuito")?,
    };
    if ele::quadro::circuitos_do_quadro(&a, q).iter().any(|c| c.id.eq_ignore_ascii_case(&id)) {
        return Err(bad("eletrica.circuito.criar", format!("circuit {id} already exists on this board")));
    }
    let descricao = p.get("descricao").and_then(Value::as_str).unwrap_or("");
    ele::editar::definir_descricao(s.doc_mut()?, q, &id, Some(descricao)).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_descrever(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto("eletrica.circuito.descrever", p, "circuito")?;
    let descricao = p.get("descricao").and_then(Value::as_str).unwrap_or("");
    ele::editar::definir_descricao(s.doc_mut()?, q, &id, Some(descricao)).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_excluir(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto("eletrica.circuito.excluir", p, "circuito")?;
    let n = ele::editar::excluir_circuito(s.doc_mut()?, q, &id).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id, "pontos": n }))
}

fn run_juntar(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let de = texto("eletrica.circuito.juntar", p, "de")?;
    let para = texto("eletrica.circuito.juntar", p, "para")?;
    let n = ele::editar::juntar_circuitos(s.doc_mut()?, q, &de, &para).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "de": de, "para": para, "pontos": n }))
}

/// O que `eletrica.circuito.definir` e `elecircuito` mudam: `None` não mexe, `Some(None)` volta ao
/// automático. Tudo é validado antes de gravar qualquer coisa.
#[derive(Default)]
struct Propriedades {
    isolacao: Option<Option<ele::nbr5410::Isolacao>>,
    cabo: Option<Option<ele::nbr5410::TipoCabo>>,
    cabo_nome: Option<Option<String>>,
    grupo_dr: Option<Option<String>>,
    esquema: Option<Option<ele::nbr5410::Esquema>>,
    tensao: Option<Option<f64>>,
    fases: Option<Option<String>>,
    descricao: Option<String>,
}

impl Propriedades {
    fn ler(cmd: &str, p: &Value) -> Result<Self> {
        let mut d = Propriedades::default();
        if let Some(v) = p.get("isolacao") {
            d.isolacao = Some(match v {
                Value::Null => None,
                v => Some(
                    v.as_str()
                        .and_then(ele::nbr5410::Isolacao::de_codigo)
                        .ok_or_else(|| bad(cmd, "`isolacao` must be \"PVC\", \"EPR\" or \"XLPE\""))?,
                ),
            });
        }
        if let Some(v) = p.get("cabo") {
            d.cabo = Some(match v {
                Value::Null => None,
                v => Some(
                    v.as_str()
                        .and_then(ele::nbr5410::TipoCabo::de_codigo)
                        .ok_or_else(|| bad(cmd, "`cabo` must be FLEX_PVC_70, UNI_PVC_70, HEPR_XLPE_90, LSZH_70 or LSZH_HEPR_90"))?,
                ),
            });
        }
        if let Some(v) = p.get("caboNome") {
            d.cabo_nome = Some(match v {
                Value::Null => None,
                v => Some(v.as_str().ok_or_else(|| bad(cmd, "`caboNome` must be text or null"))?.to_string()),
            });
        }
        if let Some(v) = p.get("grupoDr") {
            d.grupo_dr = Some(match v {
                Value::Null => None,
                v => Some(v.as_str().ok_or_else(|| bad(cmd, "`grupoDr` must be text or null"))?.to_string()),
            });
        }
        if let Some(v) = p.get("esquema") {
            d.esquema = Some(match v {
                Value::Null => None,
                v => Some(
                    v.as_str()
                        .and_then(ele::nbr5410::Esquema::de_codigo)
                        .ok_or_else(|| bad(cmd, "`esquema` must be \"F+N\", \"F+N+T\", \"2F+T\" or \"3F+N+T\""))?,
                ),
            });
        }
        if let Some(v) = p.get("tensao") {
            d.tensao = Some(match v {
                Value::Null => None,
                v => Some(v.as_f64().filter(|t| t.is_finite() && *t > 0.0).ok_or_else(|| bad(cmd, "`tensao` must be a positive number"))?),
            });
        }
        if let Some(v) = p.get("fases") {
            d.fases = Some(match v {
                Value::Null => None,
                v => {
                    let t =
                        v.as_str().filter(|t| ele::quadro::fases_de_texto(t).is_some()).ok_or_else(|| bad(cmd, "`fases` must be made of R, S, T"))?;
                    Some(t.to_string())
                }
            });
        }
        d.descricao = p.get("descricao").and_then(Value::as_str).map(str::to_string);
        Ok(d)
    }

    fn vazio(&self) -> bool {
        self.isolacao.is_none()
            && self.cabo.is_none()
            && self.cabo_nome.is_none()
            && self.grupo_dr.is_none()
            && self.esquema.is_none()
            && self.tensao.is_none()
            && self.fases.is_none()
            && self.descricao.is_none()
    }

    fn gravar(&self, s: &mut Session, q: Handle, id: &str) -> Result<()> {
        let doc = s.doc_mut()?;
        if let Some(i) = self.isolacao {
            ele::editar::definir_isolacao_circuito(doc, q, id, i).map_err(erro)?;
        }
        if let Some(c) = self.cabo {
            ele::editar::definir_cabo_circuito(doc, q, id, c).map_err(erro)?;
            // O nome acompanha a classe: sem um novo, volta ao da classe.
            if self.cabo_nome.is_none() {
                ele::editar::definir_nome_cabo(doc, q, id, None).map_err(erro)?;
            }
        }
        if let Some(n) = &self.cabo_nome {
            ele::editar::definir_nome_cabo(doc, q, id, n.as_deref()).map_err(erro)?;
        }
        if let Some(g) = &self.grupo_dr {
            ele::editar::definir_grupo_dr(doc, q, id, g.as_deref()).map_err(erro)?;
        }
        if let Some(e) = self.esquema {
            ele::editar::definir_esquema(doc, q, id, e).map_err(erro)?;
        }
        if let Some(t) = self.tensao {
            ele::editar::definir_tensao_circuito(doc, q, id, t).map_err(erro)?;
        }
        if let Some(f) = &self.fases {
            ele::editar::travar_fases(doc, q, id, f.as_deref()).map_err(erro)?;
        }
        if let Some(d) = &self.descricao {
            ele::editar::definir_descricao(doc, q, id, Some(d.trim())).map_err(erro)?;
        }
        Ok(())
    }
}

fn run_definir(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.circuito.definir";
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto(CMD, p, "circuito")?;
    // Tipo de carga e FP são atributos dos blocos (TIPO_CARGA e FP), não do circuito.
    if p.get("carga").is_some() || p.get("fp").is_some() {
        return Err(bad(CMD, "load type and power factor are block attributes (TIPO_CARGA, FP): edit the points"));
    }
    let d = Propriedades::ler(CMD, p)?;
    if d.vazio() {
        return Err(bad(CMD, "give `cabo`, `grupoDr`, `isolacao`, `esquema`, `tensao`, `fases` or `descricao`"));
    }
    d.gravar(s, q, &id)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_materiais(s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::materiais::levantar(s.doc()?, &analise(s)?, &ele::setup::atual()))
}

fn run_dimensionar(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.dimensionar";
    let a = analise(s)?;
    let quadro = match p.get("quadro").filter(|q| !q.is_null()) {
        Some(q) => Some(achar_quadro(&a, Some(q))?),
        None => None,
    };
    if a.quadros().is_empty() {
        return Err(erro("nenhum quadro no desenho: nada a dimensionar"));
    }
    // As premissas valem daqui em diante, como as das configurações.
    let mut opcoes = serde_json::Map::new();
    for k in ["compartilharPe", "temperaturaAmbiente", "fcaManual"] {
        if let Some(v) = p.get(k) {
            opcoes.insert(k.to_string(), v.clone());
        }
    }
    if !opcoes.is_empty() {
        ele::setup::json(&Value::Object(opcoes)).map_err(|e| bad(CMD, e))?;
    }
    let feitos = ele::editar::processar_calculo(s.doc_mut()?, &ele::setup::atual(), quadro).map_err(erro)?;
    let circuitos: usize = feitos.iter().map(|f| f.circuitos).sum();
    let avisos: usize = feitos.iter().map(|f| f.avisos.len()).sum();
    Ok(json!({ "quadros": json_de(&feitos)?, "circuitos": circuitos, "avisos": avisos }))
}

fn run_atributos(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.atributos";
    let h = handle(CMD, p, "ponto")?;
    json_de(&ele::editar::atributos_de(s.doc()?, h).ok_or_else(|| bad(CMD, "`ponto` is not a block"))?)
}

fn run_atributo_definir(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.atributo.definir";
    let h = handle(CMD, p, "ponto")?;
    let tag = texto(CMD, p, "tag")?;
    let valor = match p.get("valor") {
        None => None,
        Some(v) => Some(
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.as_f64().map(|n| n.to_string()))
                .ok_or_else(|| bad(CMD, "`valor` must be text or a number"))?,
        ),
    };
    let visivel = match p.get("visivel") {
        None => None,
        Some(v) => Some(v.as_bool().ok_or_else(|| bad(CMD, "`visivel` must be true or false"))?),
    };
    let posicao = match p.get("posicao") {
        None => None,
        Some(v) => Some(ponto_de(v).ok_or_else(|| bad(CMD, "`posicao` must be [x, y]"))?),
    };
    if valor.is_none() && visivel.is_none() && posicao.is_none() {
        return Err(bad(CMD, "give `valor`, `visivel` or `posicao`"));
    }
    ele::editar::editar_atributo(s.doc_mut()?, h, &tag, valor.as_deref(), visivel, posicao, &ele::setup::atual()).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "ponto": h.hex(), "tag": tag }))
}

fn run_equilibrar(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let n = ele::editar::liberar_fases(s.doc_mut()?, q).map_err(erro)?;
    let a = analise(s)?;
    let r = ele::rotear_quadro(s.doc()?, &a, &ele::setup::atual(), q).map_err(erro)?;
    Ok(json!({ "quadro": q.hex(), "destravados": n, "desequilibrio": r.desequilibrio }))
}

fn run_tabelas_demanda(_s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::setup::atual().tabelas_demanda())
}

fn run_salvar_tabela(_s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.demanda.salvar";
    let v = p.get("tabela").cloned().ok_or_else(|| bad(CMD, "`tabela` is required"))?;
    let tabela: ele::demanda::TabelaDemanda = serde_json::from_value(v).map_err(|e| bad(CMD, e.to_string()))?;
    let mut setup = ele::setup::atual();
    let id = setup.salvar_tabela(tabela).map_err(|e| bad(CMD, e))?;
    if p.get("usar").and_then(Value::as_bool).unwrap_or(false) {
        setup.concessionaria = id.clone();
    }
    ele::setup::definir(setup);
    Ok(json!({ "id": id }))
}

fn run_excluir_tabela(_s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.demanda.excluir";
    let id = texto(CMD, p, "id")?;
    let mut setup = ele::setup::atual();
    let excluida = setup.remover_tabela(&id);
    ele::setup::definir(setup);
    Ok(json!({ "excluida": excluida }))
}

fn run_alvo(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto("eletrica.circuito.alvo", p, "circuito")?;
    ele::circuito::definir_alvo(s.state()?.uid, Some((q, id.clone())));
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_setup(_s: &mut Session, p: &Value) -> Result<Value> {
    ele::setup::json(p).map_err(|e| bad("eletrica.setup", e))
}

fn run_bootstrap(s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({ "mudou": preparar(s)? }))
}

// ---------------------------------------------------------------- fiação e tabelas

fn resumo_regeneracao(r: &ele::fiacao::Regeneracao) -> String {
    format!("Fiação: {} tag(s) nova(s), {} atualizada(s), {} apagada(s); {} tabela(s) refeita(s).", r.criadas, r.tags, r.apagadas, r.tabelas)
}

/// Todos os eletrodutos do desenho, ligados ou soltos.
fn todos_os_eletrodutos(s: &Session) -> Result<Vec<Handle>> {
    let (trechos, soltos, _) = ele::fiacao::todos_os_trechos(s.doc()?, &analise(s)?, &ele::setup::atual());
    Ok(trechos.iter().map(|t| t.handle).chain(soltos.iter().map(|x| x.eletroduto)).collect())
}

/// Os eletrodutos por onde passam os circuitos de um quadro.
fn eletrodutos_do_quadro(s: &Session, q: Handle) -> Result<Vec<Handle>> {
    let f = ele::fiacao::calcular(s.doc()?, &analise(s)?, &ele::setup::atual(), q).map_err(erro)?;
    Ok(f.trechos.iter().filter(|t| !t.circuitos.is_empty()).map(|t| t.handle).collect())
}

fn regenerar_criando(s: &mut Session, criar: &[Handle]) -> Result<ele::fiacao::Regeneracao> {
    preparar(s)?;
    ele::fiacao::regenerar(s.doc_mut()?, &ele::setup::atual(), Some(criar)).map_err(erro)
}

fn run_fiacao(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elefiacao";
    let lista: Option<Vec<Handle>> = match (p.get("eletrodutos"), p.get("quadro")) {
        (Some(_), _) => Some(handles(CMD, p, "eletrodutos")?),
        (None, Some(q)) if !q.is_null() => {
            let q = achar_quadro(&analise(s)?, Some(q))?;
            Some(eletrodutos_do_quadro(s, q)?)
        }
        _ => None,
    };
    if p.get("limpar").and_then(Value::as_bool).unwrap_or(false) {
        let n = ele::fiacao::limpar(s.doc_mut()?, lista.as_deref());
        return Ok(json!({ "apagadas": n }));
    }
    let lista = match lista {
        Some(l) => l,
        None => todos_os_eletrodutos(s)?,
    };
    if lista.is_empty() {
        return Err(erro("nenhum eletroduto para receber tags: use Inserir eletroduto"));
    }
    json_de(&regenerar_criando(s, &lista)?)
}

fn run_regen(s: &mut Session, _p: &Value) -> Result<Value> {
    let r = ele::fiacao::regenerar(s.doc_mut()?, &ele::setup::atual(), None).map_err(erro)?;
    let mut v = json_de(&r)?;
    if let Value::Object(m) = &mut v {
        m.insert("message".into(), Value::String(resumo_regeneracao(&r)));
    }
    Ok(v)
}

/// ELEFIACAO digitado: os quadros ou eletrodutos selecionados, ou Enter para todos.
struct FiacaoM;

impl FiacaoM {
    fn executar(s: &mut Session, lista: Vec<Handle>) -> Result<Step> {
        match regenerar_criando(s, &lista) {
            Ok(r) => {
                s.echo(resumo_regeneracao(&r));
                for a in &r.avisos {
                    s.echo(a.clone());
                }
            }
            Err(e) => s.echo(e.to_string()),
        }
        Ok(Step::Done)
    }
}

impl Interactive for FiacaoM {
    fn name(&self) -> &'static str {
        "ELEFIACAO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Selecione o quadro ou os eletrodutos", Accept::SELECT).default("Todos")
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Cancel => Ok(Step::Done),
            Input::Enter => {
                let todos = todos_os_eletrodutos(s)?;
                Self::executar(s, todos)
            }
            Input::Pick(hs) => {
                s.set_selection(Vec::new());
                let todos = todos_os_eletrodutos(s)?;
                let mut lista: Vec<Handle> = hs.iter().copied().filter(|h| todos.contains(h)).collect();
                for h in hs {
                    if quadro_valido(s, h)?.is_some() {
                        lista.extend(eletrodutos_do_quadro(s, h)?);
                    }
                }
                if lista.is_empty() {
                    s.echo("Selecione um quadro ou um eletroduto.");
                    return Ok(Step::Continue);
                }
                Self::executar(s, lista)
            }
            _ => Ok(Step::Continue),
        }
    }
}

fn run_tabela(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elequadro_tabela";
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    let pos = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ponto` ([x, y]) is required"))?;
    preparar(s)?;
    let a = analise(s)?;
    let h = ele::tabela::inserir(s.doc_mut()?, &a, &ele::setup::atual(), q, pos).map_err(erro)?;
    Ok(json!({ "tabela": h.hex(), "quadro": q.hex() }))
}

/// ELEQUADRO_TABELA digitado: o quadro (Enter: o primeiro) e o ponto de inserção.
#[derive(Default)]
struct TabelaM {
    quadro: Option<Handle>,
}

impl Interactive for TabelaM {
    fn name(&self) -> &'static str {
        "ELEQUADRO_TABELA"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        let a = analise(s)?;
        let quadros = a.quadros();
        if quadros.is_empty() {
            s.echo("Nenhum quadro no desenho: insira um com Inserir pontos.");
            return Ok(Step::Done);
        }
        if let [q] = quadros.as_slice() {
            self.quadro = Some(q.handle);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.quadro {
            None => Prompt::new("Selecione o quadro", Accept::SELECT).default("Primeiro"),
            Some(_) => Prompt::new("Ponto de inserção da tabela", Accept::POINT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.quadro, i) {
            (_, Input::Cancel) => Ok(Step::Done),
            (None, Input::Enter) => {
                self.quadro = analise(s)?.quadros().first().map(|q| q.handle);
                Ok(Step::Continue)
            }
            (None, Input::Pick(hs)) => {
                s.set_selection(Vec::new());
                for h in hs {
                    if quadro_valido(s, h)?.is_some() {
                        self.quadro = Some(h);
                        return Ok(Step::Continue);
                    }
                }
                s.echo("Selecione um bloco de quadro.");
                Ok(Step::Continue)
            }
            (Some(q), Input::Point(p)) => {
                let a = analise(s)?;
                match ele::tabela::inserir(s.doc_mut()?, &a, &ele::setup::atual(), q, [p.x, p.y]) {
                    Ok(_) => Ok(Step::Done),
                    Err(e) => {
                        s.echo(e);
                        Ok(Step::Continue)
                    }
                }
            }
            (Some(_), Input::Enter) => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
}

// ---------------------------------------------------------------- circuitos

/// Põe um ponto no circuito, avisando na linha de comando. `false`: não era um ponto válido.
fn atribuir(s: &mut Session, quadro: Handle, circuito: &str, h: Handle) -> Result<bool> {
    let Some(p) = ele::ponto::ler_ponto(s.doc()?, h) else {
        s.echo("O objeto não é um ponto elétrico (bloco com TIPO_PONTO, POTENCIA, CIRCUITO...).");
        return Ok(false);
    };
    if p.tipo == TipoPonto::Quadro {
        s.echo("Um quadro não entra num circuito.");
        return Ok(false);
    }
    ele::editar::atribuir_circuito(s.doc_mut()?, h, quadro, circuito).map_err(erro)?;
    s.echo(format!("{}: circuito {circuito}", p.rotulo()));
    Ok(true)
}

fn quadro_valido(s: &Session, h: Handle) -> Result<Option<PontoEletrico>> {
    Ok(ele::ponto::ler_ponto(s.doc()?, h).filter(|p| p.tipo == TipoPonto::Quadro))
}

fn run_circuito(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elecircuito";
    let quadro = handle(CMD, p, "quadro")?;
    if quadro_valido(s, quadro)?.is_none() {
        return Err(bad(CMD, "`quadro` is not a distribution board block (TIPO_PONTO = QUADRO)"));
    }
    let propriedades = Propriedades::ler(CMD, p)?;
    let pontos = handles(CMD, p, "pontos")?;
    let circuito = match p
        .get("circuito")
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())))
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
    {
        Some(c) => c,
        None => ele::circuito::proximo_circuito(&analise(s)?.pontos, quadro),
    };
    propriedades.gravar(s, quadro, &circuito)?;
    let mut feitos = Vec::new();
    for h in pontos {
        if atribuir(s, quadro, &circuito, h)? {
            feitos.push(h.hex());
        }
    }
    Ok(json!({ "circuito": circuito, "pontos": feitos }))
}

/// O campo que ELECIRCUITO está perguntando.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Campo {
    Descricao,
    Esquema,
    Tensao,
}

#[derive(Default)]
struct CircuitoM {
    /// O quadro escolhido e a TAG dele.
    quadro: Option<(Handle, String)>,
    circuito: Option<String>,
    /// O próximo número livre, oferecido como padrão.
    sugestao: String,
    feitos: usize,
    pedindo: Option<Campo>,
}

impl CircuitoM {
    fn escolher_circuito(&mut self, s: &mut Session, id: String) -> Result<Step> {
        if let Some((q, _)) = &self.quadro {
            ele::circuito::definir_ativo(s.state()?.uid, Some((*q, id.clone())));
        }
        self.circuito = Some(id);
        Ok(Step::Continue)
    }

    fn terminar(s: &mut Session) -> Result<Step> {
        ele::circuito::definir_ativo(s.state()?.uid, None);
        Ok(Step::Done)
    }

    /// A resposta de uma pergunta de campo. Texto vazio ou Enter não muda nada.
    fn responder(&mut self, s: &mut Session, campo: Campo, t: &str) -> Result<()> {
        let (Some((q, _)), Some(id)) = (&self.quadro, &self.circuito) else { return Ok(()) };
        let (q, id) = (*q, id.clone());
        let t = t.trim();
        if t.is_empty() {
            return Ok(());
        }
        let r = match campo {
            Campo::Descricao => ele::editar::definir_descricao(s.doc_mut()?, q, &id, Some(t)),
            Campo::Esquema => match ele::nbr5410::Esquema::de_codigo(t) {
                Some(e) => ele::editar::definir_esquema(s.doc_mut()?, q, &id, Some(e)),
                None => Err("use F+N, F+N+T, 2F+T ou 3F+N+T".to_string()),
            },
            Campo::Tensao => match ele::ponto::numero(t) {
                Some(v) => ele::editar::definir_tensao_circuito(s.doc_mut()?, q, &id, Some(v)),
                None => Err("tensão inválida".to_string()),
            },
        };
        if let Err(e) = r {
            s.echo(e);
        }
        Ok(())
    }
}

impl Interactive for CircuitoM {
    fn name(&self) -> &'static str {
        "ELECIRCUITO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        let uid = s.state()?.uid;
        ele::circuito::definir_ativo(uid, None);
        // Escolhido antes (eletrica.circuito.alvo, o "adicionar pontos" do quadro de cargas):
        // vai direto à seleção dos pontos.
        if let Some((q, c)) = ele::circuito::tomar_alvo(uid)
            && let Some(p) = quadro_valido(s, q)?
        {
            self.quadro = Some((q, p.rotulo()));
            ele::circuito::definir_ativo(uid, Some((q, c.clone())));
            self.circuito = Some(c);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if let Some(c) = self.pedindo {
            return match c {
                Campo::Descricao => Prompt::new("Descrição do circuito", Accept::TEXT),
                Campo::Esquema => Prompt::new("Esquema (F+N, F+N+T, 2F+T, 3F+N+T)", Accept::TEXT),
                Campo::Tensao => Prompt::new("Tensão do circuito (V)", Accept::TEXT),
            };
        }
        match (&self.quadro, &self.circuito) {
            (None, _) => Prompt::new("Selecione o quadro de distribuição", Accept::SELECT),
            (Some(_), None) => Prompt::new("Número do circuito", Accept::TEXT).default(self.sugestao.clone()),
            (Some((_, tag)), Some(c)) => {
                Prompt::new(format!("Selecione os pontos do circuito {c} ({tag})"), Accept::SELECT).kw(&["Descricao", "Esquema", "Tensao"])
            }
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Some(c) = self.pedindo.take() {
            if let Input::Text(t) | Input::Keyword(t) = &i {
                self.responder(s, c, t)?;
            }
            return Ok(Step::Continue);
        }
        match (&self.quadro, &self.circuito, i) {
            (_, _, Input::Cancel) => Self::terminar(s),
            (None, _, Input::Pick(hs)) => {
                s.set_selection(Vec::new());
                for h in hs {
                    if let Some(q) = quadro_valido(s, h)? {
                        self.sugestao = ele::circuito::proximo_circuito(&analise(s)?.pontos, h);
                        s.echo(format!("Quadro {}.", q.rotulo()));
                        self.quadro = Some((h, q.rotulo()));
                        return Ok(Step::Continue);
                    }
                }
                s.echo("Selecione um bloco de quadro (TIPO_PONTO = QUADRO).");
                Ok(Step::Continue)
            }
            (None, _, Input::Enter) => Self::terminar(s),
            (Some(_), None, Input::Text(t) | Input::Keyword(t)) if !t.trim().is_empty() => self.escolher_circuito(s, t.trim().to_string()),
            (Some(_), None, Input::Enter | Input::Text(_)) => {
                let id = self.sugestao.clone();
                self.escolher_circuito(s, id)
            }
            (Some((q, _)), Some(c), Input::Pick(hs)) => {
                let (q, c) = (*q, c.clone());
                s.set_selection(Vec::new());
                for h in hs {
                    if atribuir(s, q, &c, h)? {
                        self.feitos += 1;
                    }
                }
                Ok(Step::Continue)
            }
            (Some(_), Some(_), Input::Keyword(k) | Input::Text(k)) => {
                self.pedindo = match k.trim().to_ascii_lowercase().as_str() {
                    "descricao" | "descrição" => Some(Campo::Descricao),
                    "esquema" => Some(Campo::Esquema),
                    "tensao" | "tensão" => Some(Campo::Tensao),
                    _ => {
                        s.echo("Opção inválida.");
                        None
                    }
                };
                Ok(Step::Continue)
            }
            (Some(_), Some(c), Input::Enter) => {
                s.echo(format!("{} ponto(s) no circuito {c}.", self.feitos));
                Self::terminar(s)
            }
            _ => Ok(Step::Continue),
        }
    }
}

fn run_remover(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = handles("eleremoverponto", p, "pontos")?;
    let n = ele::editar::remover_do_circuito(s.doc_mut()?, &hs).map_err(erro)?;
    Ok(json!({ "removidos": n }))
}

/// ELEREMOVERPONTO digitado: os pontos que saem do circuito, até Enter.
#[derive(Default)]
struct RemoverM {
    feitos: usize,
}

impl Interactive for RemoverM {
    fn name(&self) -> &'static str {
        "ELEREMOVERPONTO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Selecione os pontos que saem do circuito", Accept::SELECT)
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Cancel => Ok(Step::Done),
            Input::Enter => {
                s.echo(format!("{} ponto(s) sem circuito.", self.feitos));
                Ok(Step::Done)
            }
            Input::Pick(hs) => {
                s.set_selection(Vec::new());
                for h in hs {
                    match ele::editar::remover_do_circuito(s.doc_mut()?, &[h]) {
                        Ok(n) => self.feitos += n,
                        Err(e) => s.echo(e),
                    }
                }
                Ok(Step::Continue)
            }
            _ => Ok(Step::Continue),
        }
    }
}

/// O interruptor escolhido em ELECOMANDO.
struct Interruptor {
    h: Handle,
    rotulo: String,
    letra: String,
    /// O COMANDO antes da letra em uso: trocar a letra só troca a desta tecla.
    anteriores: String,
    teclas: usize,
    circuito: Option<String>,
    quadro: Option<Handle>,
}

/// Lê o interruptor e dá uma letra à tecla em uso: a pedida, a próxima livre enquanto houver tecla
/// sem letra, ou a última que ele tem.
fn preparar_interruptor(s: &mut Session, h: Handle, letra: Option<&str>) -> Result<Option<Interruptor>> {
    let a = analise(s)?;
    let Some(p) = a.ponto(h).filter(|p| p.tipo == TipoPonto::Interruptor) else { return Ok(None) };
    let teclas = p.tipo_interruptor.map_or(1, |t| t.teclas());
    let atual = p.comando.clone().unwrap_or_default();
    let tem = ele::fiacao::letras(&atual);
    let escolhida = match letra {
        Some(l) => l.trim().to_lowercase(),
        None if tem.len() < teclas => ele::circuito::proxima_letra(&a.pontos, p.quadro, p.circuito.as_deref()),
        None => tem.last().cloned().unwrap_or_else(|| "a".into()),
    };
    let comando = ele::circuito::comando_com_letra(&atual, &escolhida, teclas);
    if p.comando.as_deref() != Some(comando.as_str()) {
        ele::editar::definir_comando(s.doc_mut()?, h, &comando).map_err(erro)?;
    }
    Ok(Some(Interruptor { h, rotulo: p.rotulo(), letra: escolhida, anteriores: atual, teclas, circuito: p.circuito.clone(), quadro: p.quadro }))
}

/// Liga uma luminária ao interruptor: mesma letra e mesmo circuito, e o vínculo no XDATA. Se o
/// interruptor ainda não tem circuito, ele fica com o da luminária.
fn ligar_luminaria(s: &mut Session, sw: &mut Interruptor, h: Handle) -> Result<bool> {
    let a = analise(s)?;
    let Some(lum) = a.ponto(h).filter(|p| p.tipo == TipoPonto::Iluminacao).cloned() else {
        s.echo("O objeto não é uma luminária (TIPO_PONTO = ILUM).");
        return Ok(false);
    };
    ele::editar::definir_comando(s.doc_mut()?, h, &sw.letra).map_err(erro)?;
    ele::editar::vincular_luminaria(s.doc_mut()?, sw.h, &sw.letra, h).map_err(erro)?;
    match (&sw.circuito, sw.quadro, &lum.circuito, lum.quadro) {
        (Some(c), Some(q), lc, lq) => {
            if lc.as_deref() != Some(c.as_str()) || lq != Some(q) {
                ele::editar::atribuir_circuito(s.doc_mut()?, h, q, c).map_err(erro)?;
            }
        }
        (None, _, Some(c), Some(q)) => {
            ele::editar::atribuir_circuito(s.doc_mut()?, sw.h, q, c).map_err(erro)?;
            sw.circuito = Some(c.clone());
            sw.quadro = Some(q);
            s.echo(format!("{} entrou no circuito {c}.", sw.rotulo));
        }
        _ => s.echo("Interruptor e luminária ainda sem circuito: use Criar circuito."),
    }
    s.echo(format!("{}: comando {}", lum.rotulo(), sw.letra));
    Ok(true)
}

fn run_comando(s: &mut Session, p: &Value) -> Result<Value> {
    let h = handle("elecomando", p, "interruptor")?;
    let lista = handles("elecomando", p, "luminarias")?;
    let letra = p.get("comando").and_then(Value::as_str).filter(|l| !l.trim().is_empty());
    let mut sw =
        preparar_interruptor(s, h, letra)?.ok_or_else(|| bad("elecomando", "`interruptor` is not a switch block (TIPO_PONTO = INTERRUPTOR)"))?;
    let mut feitos = Vec::new();
    for l in lista {
        if ligar_luminaria(s, &mut sw, l)? {
            feitos.push(l.hex());
        }
    }
    Ok(json!({ "comando": sw.letra, "circuito": sw.circuito, "luminarias": feitos }))
}

#[derive(Default)]
struct ComandoM {
    interruptor: Option<Interruptor>,
    feitos: usize,
    pedindo_letra: bool,
}

impl Interactive for ComandoM {
    fn name(&self) -> &'static str {
        "ELECOMANDO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.pedindo_letra {
            return Prompt::new("Letra do comando", Accept::TEXT);
        }
        match &self.interruptor {
            None => Prompt::new("Selecione o interruptor", Accept::SELECT),
            Some(sw) => Prompt::new(format!("Selecione as luminárias do comando {}", sw.letra), Accept::SELECT).kw(&["Letra"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.pedindo_letra {
            self.pedindo_letra = false;
            if let (Input::Text(t) | Input::Keyword(t), Some(sw)) = (&i, &mut self.interruptor)
                && let [letra] = ele::fiacao::letras(t).as_slice()
            {
                let comando = ele::circuito::comando_com_letra(&sw.anteriores, letra, sw.teclas);
                ele::editar::definir_comando(s.doc_mut()?, sw.h, &comando).map_err(erro)?;
                sw.letra = letra.clone();
            }
            return Ok(Step::Continue);
        }
        match i {
            Input::Cancel => Ok(Step::Done),
            Input::Keyword(k) | Input::Text(k) if self.interruptor.is_some() && k.trim().eq_ignore_ascii_case("letra") => {
                self.pedindo_letra = true;
                Ok(Step::Continue)
            }
            Input::Pick(hs) => {
                s.set_selection(Vec::new());
                match &mut self.interruptor {
                    None => {
                        for h in hs {
                            if let Some(sw) = preparar_interruptor(s, h, None)? {
                                s.echo(format!("Interruptor {}: comando {}.", sw.rotulo, sw.letra));
                                self.interruptor = Some(sw);
                                return Ok(Step::Continue);
                            }
                        }
                        s.echo("Selecione um interruptor (TIPO_PONTO = INTERRUPTOR).");
                    }
                    Some(sw) => {
                        for h in hs {
                            if ligar_luminaria(s, sw, h)? {
                                self.feitos += 1;
                            }
                        }
                    }
                }
                Ok(Step::Continue)
            }
            Input::Enter => {
                if let Some(sw) = &self.interruptor {
                    s.echo(format!("{} luminária(s) no comando {}.", self.feitos, sw.letra));
                }
                Ok(Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}

// ---------------------------------------------------------------- eletrodutos

/// O bloco de uma ponta num ponto da planta: o bloco elétrico que está ali ou, no meio de um
/// eletroduto, a caixa de passagem que divide o trecho.
fn bloco_no_ponto(s: &mut Session, q: [f64; 2]) -> Result<std::result::Result<Handle, String>> {
    let setup = ele::setup::atual();
    let m = metros(s)?;
    if let Some(h) = ele::eletroduto::bloco_em(s.doc()?, &setup, q, m) {
        return Ok(Ok(h));
    }
    let Some((e, p)) = ele::eletroduto::eletroduto_em(s.doc()?, &setup, q, m) else {
        return Ok(Err("Clique num bloco elétrico ou no meio de um eletroduto.".to_string()));
    };
    Ok(ele::eletroduto::dividir(s.doc_mut()?, &setup, e, p, m))
}

/// Uma ponta do eletroduto no JSON: o handle de um bloco elétrico ou `[x, y]`.
fn ponta(s: &mut Session, cmd: &str, p: &Value, k: &str) -> Result<Handle> {
    match p.get(k) {
        Some(Value::String(t)) => Handle::parse_hex(t).ok_or_else(|| bad(cmd, format!("`{k}` is not a handle"))),
        Some(v @ Value::Array(_)) => {
            let q = ponto_de(v).ok_or_else(|| bad(cmd, format!("`{k}` must be [x, y]")))?;
            bloco_no_ponto(s, q)?.map_err(|e| bad(cmd, format!("`{k}`: {e}")))
        }
        _ => Err(bad(cmd, format!("`{k}` (handle or [x, y]) is required"))),
    }
}

/// Um perfil de eletroduto pelo nome: a escolha que não existe é um erro, para não desenhar sem ele.
fn perfil_de(cmd: &str, v: &Value) -> Result<ele::setup::PerfilEletroduto> {
    let nome = v.as_str().ok_or_else(|| bad(cmd, "`perfil` must be the name of a conduit profile"))?;
    ele::setup::atual().perfil(nome).cloned().ok_or_else(|| bad(cmd, format!("there is no conduit profile named `{nome}`")))
}

fn run_eletroduto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eleeletroduto";
    let modelo = ele::eletroduto::modelo(s.state()?.uid);
    // O pedido no JSON ou o da mini-barra, a não ser que o JSON peça outro tipo ou outra instalação.
    let perfil = match p.get("perfil").filter(|v| !v.is_null()) {
        Some(v) => Some(perfil_de(CMD, v)?),
        None if ["tipo", "instalacao", "piso"].iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null())) => None,
        None => modelo.perfil.as_deref().and_then(|n| ele::setup::atual().perfil(n).cloned()),
    };
    let instalacao = match p.get("instalacao") {
        None | Some(Value::Null) => match p.get("piso").and_then(Value::as_bool) {
            Some(true) => Instalacao::Piso,
            Some(false) => Instalacao::Teto,
            None => modelo.instalacao,
        },
        Some(v) => instalacao_de(CMD, v)?,
    };
    let tipo = match p.get("tipo").filter(|v| !v.is_null()) {
        Some(v) => tipo_de(CMD, v)?,
        None => modelo.tipo,
    };
    let flecha = match p.get("flecha") {
        None | Some(Value::Null) => modelo.flecha,
        Some(v) => numero_finito(CMD, v, "flecha")?,
    };
    if !matches!(p.get("de"), Some(Value::String(_) | Value::Array(_))) || !matches!(p.get("para"), Some(Value::String(_) | Value::Array(_))) {
        return Err(bad(CMD, "`de` and `para` (handle or [x, y]) are required"));
    }
    preparar(s)?;
    let de = ponta(s, CMD, p, "de")?;
    let para = ponta(s, CMD, p, "para")?;
    let tracado = match p.get("tracado").filter(|v| !v.is_null()) {
        Some(v) => tracado_de(CMD, v)?,
        // Com flecha zero pedida, o arco de sempre vira reta.
        None if p.get("flecha").is_some() && flecha.abs() < 1e-9 => ele::eletroduto::Tracado::Reto,
        None => modelo.tracado,
    };
    let desvios = match p.get("desvios").filter(|v| !v.is_null()) {
        None => Vec::new(),
        Some(v) => v
            .as_array()
            .ok_or_else(|| bad(CMD, "`desvios` must be [[x, y]]"))?
            .iter()
            .map(|q| ponto_de(q).ok_or_else(|| bad(CMD, "`desvios` must be [[x, y]]")))
            .collect::<Result<Vec<_>>>()?,
    };
    let diagonal_primeiro = p.get("diagonalPrimeiro").or_else(|| p.get("verticalPrimeiro")).and_then(Value::as_bool).unwrap_or(false);
    let percurso = ele::eletroduto::Percurso { tracado, flecha, desvios, diagonal_primeiro };
    let h = match &perfil {
        Some(f) => ele::eletroduto::inserir_com_perfil(s.doc_mut()?, de, para, &percurso, f).map_err(erro)?,
        None => ele::eletroduto::inserir_percurso(s.doc_mut()?, de, para, instalacao, &percurso, tipo).map_err(erro)?,
    };
    Ok(json!({ "eletroduto": h.hex(), "de": de.hex(), "para": para.hex() }))
}

fn run_modelo_eletroduto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.modelo";
    let uid = s.state()?.uid;
    let mut m = ele::eletroduto::modelo(uid);
    // Escolher outro tipo ou outra instalação à mão sai do perfil.
    if let Some(v) = p.get("instalacao").filter(|v| !v.is_null()) {
        m.instalacao = instalacao_de(CMD, v)?;
        m.perfil = None;
    }
    if let Some(v) = p.get("tipo").filter(|v| !v.is_null()) {
        m.tipo = tipo_de(CMD, v)?;
        m.perfil = None;
    }
    match p.get("perfil") {
        None => {}
        Some(Value::Null) => m.perfil = None,
        Some(v) => m = m.com_perfil(&perfil_de(CMD, v)?),
    }
    if let Some(v) = p.get("tracado").filter(|v| !v.is_null()) {
        m.tracado = tracado_de(CMD, v)?;
    }
    if let Some(v) = p.get("flecha").filter(|v| !v.is_null()) {
        m.flecha = numero_finito(CMD, v, "flecha")?.clamp(-ele::eletroduto::FLECHA_MAXIMA, ele::eletroduto::FLECHA_MAXIMA);
    }
    ele::eletroduto::definir_modelo(uid, m.clone());
    json_de(&m)
}

fn run_dividir(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.dividir";
    let e = handle(CMD, p, "eletroduto")?;
    let q = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ponto` ([x, y]) is required"))?;
    let m = metros(s)?;
    let caixa = ele::eletroduto::dividir(s.doc_mut()?, &ele::setup::atual(), e, q, m).map_err(erro)?;
    Ok(json!({ "caixa": caixa.hex() }))
}

fn run_definir_eletroduto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.definir";
    let e = handle(CMD, p, "eletroduto")?;
    if !s.doc()?.model.get(e).is_some_and(|x| ele::setup::atual().e_camada_eletroduto(&x.common.layer)) {
        return Err(bad(CMD, "`eletroduto` is not on a conduit layer"));
    }
    let diametro = match p.get("diametro") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_f64().filter(|d| d.is_finite() && *d > 0.0).ok_or_else(|| bad(CMD, "`diametro` must be a positive number"))?)),
    };
    let instalacao = p.get("instalacao").filter(|v| !v.is_null()).map(|v| instalacao_de(CMD, v)).transpose()?;
    let tipo = p.get("tipo").filter(|v| !v.is_null()).map(|v| tipo_de(CMD, v)).transpose()?;
    let perfil = p.get("perfil").filter(|v| !v.is_null()).map(|v| perfil_de(CMD, v)).transpose()?;
    if diametro.is_none() && instalacao.is_none() && tipo.is_none() && perfil.is_none() {
        return Err(bad(CMD, "give `diametro`, `instalacao`, `tipo` or `perfil`"));
    }
    if let Some(f) = &perfil {
        ele::eletroduto::aplicar_perfil(s.doc_mut()?, e, f).map_err(erro)?;
    }
    if let Some(t) = tipo {
        ele::eletroduto::definir_tipo(s.doc_mut()?, e, t).map_err(erro)?;
    }
    if let Some(d) = diametro {
        ele::eletroduto::travar_diametro(s.doc_mut()?, e, d).map_err(erro)?;
    }
    if let Some(i) = instalacao {
        ele::eletroduto::definir_instalacao(s.doc_mut()?, e, i).map_err(erro)?;
    }
    Ok(json!({ "eletroduto": e.hex() }))
}

/// ELEELETRODUTO digitado: um clique em cada bloco (ou no meio de um eletroduto, que recebe uma
/// caixa de passagem); cada um é ligado ao anterior, até Enter.
struct EletrodutoM {
    pedindo_tipo: bool,
    /// Os pontos de passagem do trecho em andamento.
    desvios: Vec<[f64; 2]>,
    /// No 45°, a diagonal vem antes do trecho reto.
    diagonal_primeiro: bool,
    anterior: Option<Handle>,
    /// Lado do arco: 1 à esquerda de quem vai do anterior ao próximo, -1 à direita.
    lado: f64,
    pedindo_flecha: bool,
    feitos: usize,
}

impl Default for EletrodutoM {
    fn default() -> Self {
        EletrodutoM {
            pedindo_tipo: false,
            desvios: Vec::new(),
            diagonal_primeiro: false,
            anterior: None,
            lado: 1.0,
            pedindo_flecha: false,
            feitos: 0,
        }
    }
}

impl EletrodutoM {
    /// O percurso do trecho em andamento, pelo modelo.
    fn percurso(&self, m: &ele::eletroduto::ModeloEletroduto) -> ele::eletroduto::Percurso {
        ele::eletroduto::Percurso {
            tracado: m.tracado,
            flecha: self.lado * m.flecha,
            desvios: self.desvios.clone(),
            diagonal_primeiro: self.diagonal_primeiro,
        }
    }

    /// O clique não acertou nem bloco nem eletroduto: é um ponto de passagem.
    fn ponto_vazio(&self, s: &Session, q: [f64; 2]) -> Result<bool> {
        let (setup, m) = (ele::setup::atual(), metros(s)?);
        let doc = s.doc()?;
        Ok(ele::eletroduto::bloco_em(doc, &setup, q, m).is_none() && ele::eletroduto::eletroduto_em(doc, &setup, q, m).is_none())
    }

    /// Liga o bloco ao anterior.
    fn clicar(&mut self, s: &mut Session, h: Handle) -> Result<Step> {
        if let Some(a) = self.anterior {
            let m = ele::eletroduto::modelo(s.state()?.uid);
            let percurso = self.percurso(&m);
            let perfil = m.perfil.as_deref().and_then(|n| ele::setup::atual().perfil(n).cloned());
            let feito = match &perfil {
                Some(f) => ele::eletroduto::inserir_com_perfil(s.doc_mut()?, a, h, &percurso, f),
                None => ele::eletroduto::inserir_percurso(s.doc_mut()?, a, h, m.instalacao, &percurso, m.tipo),
            };
            match feito {
                Ok(_) => {
                    self.feitos += 1;
                    self.desvios.clear();
                }
                Err(e) => {
                    s.echo(e);
                    return Ok(Step::Continue);
                }
            }
        }
        self.anterior = Some(h);
        Ok(Step::Continue)
    }
}

impl Interactive for EletrodutoM {
    fn name(&self) -> &'static str {
        "ELEELETRODUTO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        // Na primeira vez no desenho, começa no primeiro perfil do projeto.
        let uid = s.state()?.uid;
        if !ele::eletroduto::modelo_definido(uid)
            && let Some(p) = ele::setup::atual().perfis_eletroduto.first()
        {
            ele::eletroduto::definir_modelo(uid, ele::eletroduto::modelo(uid).com_perfil(p));
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        let m = s.state().map(|st| ele::eletroduto::modelo(st.uid)).unwrap_or_default();
        if self.pedindo_tipo {
            return Prompt::new("Tipo de eletroduto (PVC-CL, PVC-CR, PVC-R, PEAD, AG-E, AG-F)", Accept::TEXT);
        }
        if self.pedindo_flecha {
            return Prompt::new("Flecha do arco (fração da corda, de 0 a 0,5)", Accept::TEXT).default(ele::ponto::br(m.flecha));
        }
        let onde = m.perfil.clone().unwrap_or_else(|| format!("{}, {}", m.instalacao.nome().to_lowercase(), m.tipo.abreviado()));
        let msg = match self.anterior {
            None => format!("Clique no primeiro bloco ou no meio de um eletroduto ({onde})"),
            Some(_) => format!("Clique no próximo bloco ou no meio de um eletroduto ({onde}; Enter termina)"),
        };
        let chaves: &[&str] = if self.anterior.is_some() {
            &["Reto", "Angulo45", "Arco", "Teto", "Piso", "Parede", "Tipo", "Inverter", "Flecha"]
        } else {
            &["Reto", "Angulo45", "Arco", "Teto", "Piso", "Parede", "Tipo", "Flecha"]
        };
        Prompt::new(msg, Accept::POINT).kw(chaves)
    }
    fn preview(&self, s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let Some(a) = self.anterior.and_then(|h| ele::ponto::ler_ponto(s.doc().ok()?, h)).map(|p| p.posicao) else { return Vec::new() };
        if (a[0] - cursor.x).hypot(a[1] - cursor.y) <= 0.0 {
            return Vec::new();
        }
        let m = s.state().map(|st| ele::eletroduto::modelo(st.uid)).unwrap_or_default();
        vec![self.percurso(&m).entidade(a, [cursor.x, cursor.y])]
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let uid = s.state()?.uid;
        if self.pedindo_tipo {
            self.pedindo_tipo = false;
            if let Input::Text(t) | Input::Keyword(t) = &i {
                match ele::rota::TipoEletroduto::de_codigo(t) {
                    Some(tipo) => {
                        ele::eletroduto::definir_modelo(uid, ele::eletroduto::ModeloEletroduto { tipo, perfil: None, ..ele::eletroduto::modelo(uid) })
                    }
                    None if t.trim().is_empty() => {}
                    None => s.echo("Tipo inválido."),
                }
            }
            return Ok(Step::Continue);
        }
        if self.pedindo_flecha {
            self.pedindo_flecha = false;
            if let Input::Text(t) | Input::Keyword(t) = &i
                && let Some(f) = ele::ponto::numero(t)
            {
                let mut m = ele::eletroduto::modelo(uid);
                m.flecha = f.clamp(0.0, ele::eletroduto::FLECHA_MAXIMA);
                ele::eletroduto::definir_modelo(uid, m);
            }
            return Ok(Step::Continue);
        }
        match i {
            Input::Cancel | Input::Enter => {
                if self.feitos > 0 {
                    s.echo(format!("{} eletroduto(s) inserido(s).", self.feitos));
                }
                Ok(Step::Done)
            }
            Input::Keyword(k) | Input::Text(k) => {
                let mut m = ele::eletroduto::modelo(uid);
                match k.trim().to_ascii_lowercase().as_str() {
                    "teto" => (m.instalacao, m.perfil) = (Instalacao::Teto, None),
                    "piso" => (m.instalacao, m.perfil) = (Instalacao::Piso, None),
                    "parede" => (m.instalacao, m.perfil) = (Instalacao::Parede, None),
                    "inverter" => {
                        self.lado = -self.lado;
                        self.diagonal_primeiro = !self.diagonal_primeiro;
                    }
                    "reto" => m.tracado = ele::eletroduto::Tracado::Reto,
                    "angulo45" | "45" | "ortogonal" => m.tracado = ele::eletroduto::Tracado::Angulo45,
                    "arco" => m.tracado = ele::eletroduto::Tracado::Arco,
                    "flecha" => self.pedindo_flecha = true,
                    "tipo" => self.pedindo_tipo = true,
                    _ => s.echo("Opção inválida."),
                }
                ele::eletroduto::definir_modelo(uid, m);
                Ok(Step::Continue)
            }
            Input::Point(p) if self.anterior.is_some() && self.ponto_vazio(s, [p.x, p.y])? => {
                self.desvios.push([p.x, p.y]);
                s.echo(format!("Desvio {} no trajeto.", self.desvios.len()));
                Ok(Step::Continue)
            }
            Input::Point(p) => match bloco_no_ponto(s, [p.x, p.y])? {
                Ok(h) => self.clicar(s, h),
                Err(e) => {
                    s.echo(e);
                    Ok(Step::Continue)
                }
            },
            Input::Pick(hs) => {
                s.set_selection(Vec::new());
                let bloco = hs.iter().copied().find(|h| s.doc().ok().and_then(|d| ele::ponto::ler_ponto(d, *h)).is_some());
                match bloco {
                    Some(h) => self.clicar(s, h),
                    None => {
                        s.echo("Clique num bloco elétrico ou no meio de um eletroduto.");
                        Ok(Step::Continue)
                    }
                }
            }
            _ => Ok(Step::Continue),
        }
    }
}

// ---------------------------------------------------------------- inserir pontos

fn valores_de(cmd: &str, p: &Value) -> Result<Vec<(String, String)>> {
    match p.get("valores") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(m)) => m
            .iter()
            .map(|(k, v)| match v {
                Value::String(t) => Ok((k.clone(), t.clone())),
                Value::Number(n) => Ok((k.clone(), n.to_string())),
                _ => Err(bad(cmd, format!("value of `{k}` must be text or a number"))),
            })
            .collect(),
        Some(_) => Err(bad(cmd, "`valores` must be an object {TAG: value}")),
    }
}

/// O quadro pai pedido: handle ou TAG de um quadro; sem ele ou `null`, nenhum.
fn quadro_pai(s: &Session, cmd: &str, p: &Value) -> Result<Option<Handle>> {
    match p.get("quadro") {
        None | Some(Value::Null) => Ok(None),
        Some(v) if v.is_string() => achar_quadro(&analise(s)?, Some(v)).map(Some).map_err(|_| bad(cmd, "`quadro` is not a board of the drawing")),
        Some(_) => Err(bad(cmd, "`quadro` must be a handle or a TAG")),
    }
}

/// Liga o ponto recém-inserido ao quadro pai; um quadro não tem quadro pai.
fn ligar_ao_quadro_pai(s: &mut Session, h: Handle, pai: Option<Handle>) -> Result<()> {
    if let Some(q) = pai
        && ele::ponto::ler_ponto(s.doc()?, h).is_some_and(|p| p.tipo != TipoPonto::Quadro)
    {
        ele::editar::ligar_ao_quadro(s.doc_mut()?, h, q).map_err(erro)?;
    }
    Ok(())
}

/// Os valores, com `altura` (m) no lugar do atributo ALTURA quando vier.
fn valores_com_altura(cmd: &str, p: &Value) -> Result<Vec<(String, String)>> {
    let mut v = valores_de(cmd, p)?;
    if let Some(a) = p.get("altura").filter(|a| !a.is_null()) {
        let a = a.as_f64().filter(|a| a.is_finite() && (0.0..=50.0).contains(a)).ok_or_else(|| bad(cmd, "`altura` must be a number from 0 to 50"))?;
        v.retain(|(t, _)| !t.eq_ignore_ascii_case("ALTURA"));
        v.push(("ALTURA".to_string(), format!("{a} m")));
    }
    Ok(v)
}

fn run_ponto_demanda(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.demanda";
    let hs = handles(CMD, p, "pontos")?;
    if p.get("classe").is_none() && p.get("fator").is_none() {
        return Err(bad(CMD, "give `classe` or `fator`"));
    }
    // Tudo é conferido antes de gravar qualquer coisa.
    let classe = match p.get("classe") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(
            v.as_str().and_then(ele::demanda::TipoCarga::de_codigo).ok_or_else(|| bad(CMD, format!("`classe` has an unknown value: {v}")))?,
        )),
    };
    let fator = match p.get("fator") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => {
            Some(Some(v.as_f64().filter(|f| f.is_finite() && (0.0..=1.0).contains(f)).ok_or_else(|| bad(CMD, "`fator` must be from 0 to 1"))?))
        }
    };
    for h in &hs {
        if ele::ponto::ler_ponto(s.doc()?, *h).is_none() {
            return Err(bad(CMD, format!("{} is not an electrical point", h.hex())));
        }
    }
    let doc = s.doc_mut()?;
    for h in &hs {
        if let Some(c) = classe {
            ele::editar::definir(doc, *h, ele::ponto::TAGS_CARGA, c.map(|c| c.codigo())).map_err(erro)?;
        }
        if let Some(f) = fator {
            ele::editar::definir(doc, *h, ele::ponto::TAGS_FATOR_DEMANDA, f.map(ele::ponto::br).as_deref()).map_err(erro)?;
        }
    }
    Ok(json!({ "pontos": hs.len() }))
}

fn run_altura(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.altura";
    let hs = handles(CMD, p, "pontos")?;
    let altura = match p.get("altura") {
        None => return Err(bad(CMD, "`altura` (m or null) is required")),
        Some(Value::Null) => None,
        Some(v) => Some(numero_finito(CMD, v, "altura")?),
    };
    let setup = ele::setup::atual();
    for h in &hs {
        ele::editar::definir_altura(s.doc_mut()?, *h, altura, &setup).map_err(erro)?;
    }
    Ok(json!({ "pontos": hs.len() }))
}

// ---------------------------------------------------------------- famílias

fn no_editor_de_blocos(s: &Session) -> std::result::Result<(), String> {
    match s.state() {
        Ok(st) if st.block_edit.as_ref().is_some_and(|b| !b.current) => Ok(()),
        Ok(_) => Err("Abra o editor de blocos em um bloco (BEDIT).".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// O bloco de um comando de família: o pedido ou o que está em edição.
fn bloco_da_familia(s: &Session, cmd: &str, p: &Value) -> Result<String> {
    if p.get("bloco").is_some_and(|b| !b.is_null()) {
        return texto(cmd, p, "bloco");
    }
    let st = s.state()?;
    match st.block_edit.as_ref().filter(|b| !b.current) {
        Some(b) => Ok(b.name.clone()),
        None => Err(bad(cmd, "`bloco` is required outside the Block Editor")),
    }
}

fn run_familia_ler(s: &mut Session, p: &Value) -> Result<Value> {
    let bloco = bloco_da_familia(s, "eletrica.familia.ler", p)?;
    json_de(&ele::familia::ler(s.doc()?, &bloco))
}

/// Um campo de texto com código: `None` quando não veio.
fn codigo_de<T>(cmd: &str, p: &Value, k: &str, de_codigo: fn(&str) -> Option<T>) -> Result<Option<T>> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(de_codigo).map(Some).ok_or_else(|| bad(cmd, format!("`{k}` has an unknown value: {v}"))),
    }
}

fn run_familia_definir(s: &mut Session, p: &Value) -> Result<Value> {
    use ele::familia::{CaixaFisica, Familia, FamilyCategory, RefAltura, SystemDomain};
    const CMD: &str = "eletrica.familia.definir";
    let bloco = bloco_da_familia(s, CMD, p)?;
    let em_edicao = s.state()?.block_edit.as_ref().is_some_and(|b| b.name.eq_ignore_ascii_case(&bloco));
    if !em_edicao && s.doc()?.block(&bloco).is_none() {
        return Err(bad(CMD, format!("the drawing has no block `{bloco}`")));
    }
    let familia = if p.get("remover").and_then(Value::as_bool).unwrap_or(false) {
        None
    } else {
        let categoria = codigo_de(CMD, p, "categoria", FamilyCategory::de_codigo)?;
        let atual = ele::familia::ler(s.doc()?, &bloco);
        // Trocar a categoria traz o que é natural dela; os valores padrão do projetista ficam.
        let mut f = match (atual, categoria) {
            (Some(a), Some(c)) if a.categoria != c => Familia {
                potencia: a.potencia,
                fp: a.fp,
                teclas: a.teclas,
                classe_demanda: a.classe_demanda,
                fator_demanda: a.fator_demanda,
                ..Familia::da_categoria(c)
            },
            (Some(a), _) => a,
            (None, Some(c)) => Familia::da_categoria(c),
            (None, None) => return Err(bad(CMD, "the block has no family yet: give `categoria`")),
        };
        if let Some(d) = codigo_de(CMD, p, "dominio", SystemDomain::de_codigo)? {
            f.dominio = d;
        }
        if let Some(r) = codigo_de(CMD, p, "refAltura", RefAltura::de_codigo)? {
            f.ref_altura = r;
        }
        if let Some(c) = codigo_de(CMD, p, "caixa", CaixaFisica::de_codigo)? {
            f.caixa = c;
        }
        if let Some(v) = p.get("cotaFixa").filter(|v| !v.is_null()) {
            f.cota_fixa = numero_finito(CMD, v, "cotaFixa")?;
        }
        if let Some(v) = p.get("potencia") {
            f.potencia = if v.is_null() { None } else { Some(numero_finito(CMD, v, "potencia")?) };
        }
        if let Some(v) = p.get("fp") {
            f.fp = if v.is_null() { None } else { Some(numero_finito(CMD, v, "fp")?) };
        }
        if let Some(v) = p.get("classeDemanda") {
            f.classe_demanda = if v.is_null() { None } else { codigo_de(CMD, p, "classeDemanda", ele::demanda::TipoCarga::de_codigo)? };
        }
        if let Some(v) = p.get("fatorDemanda") {
            f.fator_demanda = if v.is_null() { None } else { Some(numero_finito(CMD, v, "fatorDemanda")?) };
        }
        if let Some(v) = p.get("teclas").filter(|v| !v.is_null()) {
            f.teclas = v.as_u64().and_then(|n| u8::try_from(n).ok()).ok_or_else(|| bad(CMD, "`teclas` must be from 1 to 3"))?;
        }
        Some(f)
    };
    ele::familia::gravar(s.doc_mut()?, &bloco, familia.as_ref()).map_err(|e| bad(CMD, e))?;
    // No editor, o desenho de trás recebe junto: a família não depende de salvar a geometria.
    if let Some(b) = s.state_mut()?.block_edit.as_mut() {
        ele::familia::gravar(std::sync::Arc::make_mut(&mut b.host.doc), &bloco, familia.as_ref()).map_err(erro)?;
    }
    json_de(&familia)
}

fn run_familia_geometria(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.familia.geometria";
    let forma = texto(CMD, p, "forma")?;
    let m = metros(s)?;
    let n = ele::biblioteca::inserir_geometria(s.doc_mut()?, &forma, m).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "entidades": n }))
}

fn run_familia_atributos(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.familia.atributos";
    let bloco = bloco_da_familia(s, CMD, p)?;
    let categoria = match codigo_de(CMD, p, "categoria", ele::familia::FamilyCategory::de_codigo)? {
        Some(c) => c,
        None => ele::familia::ler(s.doc()?, &bloco).map(|f| f.categoria).ok_or_else(|| bad(CMD, "the block has no family yet: give `categoria`"))?,
    };
    let m = metros(s)?;
    let n = ele::biblioteca::gerar_atributos(s.doc_mut()?, categoria, m).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "criados": n }))
}

fn run_familia_conector(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elefamilia_conector";
    let q = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ponto` ([x, y]) is required"))?;
    let m = metros(s)?;
    let h = ele::biblioteca::definir_conector(s.doc_mut()?, q, m).map_err(erro)?;
    Ok(json!({ "conector": h.hex() }))
}

/// ELEFAMILIA_CONECTOR digitado: o ponto que vira a origem do bloco em edição.
struct ConectorM;

impl Interactive for ConectorM {
    fn name(&self) -> &'static str {
        "ELEFAMILIA_CONECTOR"
    }
    fn begin(&mut self, _s: &mut Session) -> Result<Step> {
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Ponto de inserção e conexão do bloco", Accept::POINT)
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Point(p) => {
                let m = metros(s)?;
                match ele::biblioteca::definir_conector(s.doc_mut()?, [p.x, p.y], m) {
                    Ok(_) => Ok(Step::Done),
                    Err(e) => {
                        s.echo(e);
                        Ok(Step::Continue)
                    }
                }
            }
            Input::Cancel | Input::Enter => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
}

fn rotacao_de(cmd: &str, p: &Value) -> Result<f64> {
    match p.get("rotacao") {
        None | Some(Value::Null) => Ok(0.0),
        Some(v) => v.as_f64().filter(|r| r.is_finite()).ok_or_else(|| bad(cmd, "`rotacao` must be a number (degrees)")),
    }
}

fn run_ponto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eleponto";
    let bloco = texto(CMD, p, "bloco")?;
    let rotacao = rotacao_de(CMD, p)?;
    let valores = valores_com_altura(CMD, p)?;
    let auto = p.get("rotacaoAuto").and_then(Value::as_bool).unwrap_or(false);
    let lista = p.get("pontos").and_then(Value::as_array).ok_or_else(|| bad(CMD, "`pontos` ([[x, y]]) is required"))?;
    let mut posicoes = Vec::new();
    for v in lista {
        match ponto_de(v) {
            Some(q) => posicoes.push(q),
            None => return Err(bad(CMD, format!("`pontos` holds [x, y], got {v}"))),
        }
    }
    preparar(s)?;
    let pai = quadro_pai(s, CMD, p)?;
    let setup = ele::setup::atual();
    let m = metros(s)?;
    let mut feitos = Vec::new();
    for pos in posicoes {
        let r = if auto { ele::biblioteca::alinhar_a_parede(s.doc()?, &setup, pos, m).unwrap_or(rotacao) } else { rotacao };
        let h = ele::biblioteca::inserir_ponto(s.doc_mut()?, &bloco, pos, r, &valores, &setup).map_err(erro)?;
        ligar_ao_quadro_pai(s, h, pai)?;
        feitos.push(h.hex());
    }
    Ok(json!({ "pontos": feitos }))
}

fn run_blocos(s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::biblioteca::blocos(s.doc()?))
}

fn run_modelo(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.modelo";
    let bloco = texto(CMD, p, "bloco")?;
    let blocos = ele::biblioteca::blocos(s.doc()?);
    let b =
        blocos.iter().find(|b| b.nome.eq_ignore_ascii_case(&bloco)).ok_or_else(|| bad(CMD, format!("the drawing has no point block `{bloco}`")))?;
    let modelo = ele::biblioteca::Modelo {
        bloco: b.nome.clone(),
        rotacao: rotacao_de(CMD, p)?,
        rotacao_auto: p.get("rotacaoAuto").and_then(Value::as_bool).unwrap_or(true),
        valores: valores_com_altura(CMD, p)?,
        quadro: quadro_pai(s, CMD, p)?,
    };
    ele::biblioteca::definir_modelo(s.state()?.uid, Some(modelo.clone()));
    json_de(&modelo)
}

fn run_excluir_pontos(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = handles("eletrica.ponto.excluir", p, "pontos")?;
    let n = ele::biblioteca::excluir(s.doc_mut()?, &hs).map_err(erro)?;
    Ok(json!({ "excluidos": n }))
}

/// ELEPONTO digitado: insere o que o modelo do desenho diz (a barra de cima o muda enquanto o
/// comando corre), um ponto por clique, até Enter. Sem modelo, pergunta o bloco.
#[derive(Default)]
struct PontoM {
    pedindo_bloco: bool,
    pedindo_rotacao: bool,
    /// O que Girar somou à rotação, nesta execução.
    giro: f64,
    feitos: usize,
}

impl PontoM {
    fn modelo(s: &Session) -> Option<ele::biblioteca::Modelo> {
        let m = ele::biblioteca::modelo(s.state().ok()?.uid)?;
        ele::biblioteca::blocos(s.doc().ok()?).iter().any(|b| b.nome.eq_ignore_ascii_case(&m.bloco)).then_some(m)
    }

    /// A rotação em `q`: a da parede, se for automática e houver uma ali, mais o giro.
    fn rotacao(&self, s: &Session, m: &ele::biblioteca::Modelo, q: [f64; 2]) -> f64 {
        let parede = m
            .rotacao_auto
            .then(|| s.doc().ok().and_then(|d| ele::biblioteca::alinhar_a_parede(d, &ele::setup::atual(), q, metros(s).unwrap_or(1.0))))
            .flatten();
        parede.unwrap_or(m.rotacao) + self.giro
    }
}

impl Interactive for PontoM {
    fn name(&self) -> &'static str {
        "ELEPONTO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        preparar(s)?;
        // Sem nada escolhido, começa pelo primeiro bloco base: o clique já insere.
        if Self::modelo(s).is_none() {
            let primeiro = ele::biblioteca::BLOCOS_BASE[0].to_string();
            let nome = ele::biblioteca::blocos(s.doc()?).into_iter().map(|b| b.nome).find(|n| n.eq_ignore_ascii_case(&primeiro));
            let Some(bloco) = nome.or_else(|| ele::biblioteca::blocos(s.doc().ok()?).into_iter().next().map(|b| b.nome)) else {
                s.echo("O desenho não tem blocos de ponto elétrico.");
                return Ok(Step::Done);
            };
            ele::biblioteca::definir_modelo(s.state()?.uid, Some(ele::biblioteca::Modelo { bloco, rotacao_auto: true, ..Default::default() }));
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        if self.pedindo_bloco {
            return Prompt::new("Nome do bloco do ponto", Accept::TEXT);
        }
        if self.pedindo_rotacao {
            return Prompt::new("Rotação (graus)", Accept::TEXT);
        }
        let nome = Self::modelo(s).map_or(String::new(), |m| m.bloco);
        Prompt::new(format!("Ponto de inserção de {nome} (Tab gira; Enter termina)"), Accept::POINT).kw(&["Bloco", "Rotacao", "Girar"])
    }
    fn preview(&self, s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let Some(m) = Self::modelo(s) else { return Vec::new() };
        if self.pedindo_rotacao {
            return Vec::new();
        }
        vec![EntityKind::Insert(cadcraft_doc::Insert {
            block: m.bloco.clone(),
            insert: cadcraft_doc::geom::Vec3::new(cursor.x, cursor.y, 0.0),
            scale: cadcraft_doc::geom::Vec3::new(1.0, 1.0, 1.0),
            rotation: self.rotacao(s, &m, [cursor.x, cursor.y]).to_radians(),
            attribs: Vec::new(),
            cols: 1,
            rows: 1,
            col_spacing: 0.0,
            row_spacing: 0.0,
        })]
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if matches!(i, Input::Cancel) {
            return Ok(Step::Done);
        }
        if self.pedindo_bloco {
            self.pedindo_bloco = false;
            if let Input::Text(n) | Input::Keyword(n) = &i {
                let n = n.trim();
                match ele::biblioteca::blocos(s.doc()?).into_iter().find(|b| b.nome.eq_ignore_ascii_case(n)) {
                    Some(b) => {
                        let quadro = Self::modelo(s).and_then(|m| m.quadro);
                        ele::biblioteca::definir_modelo(
                            s.state()?.uid,
                            Some(ele::biblioteca::Modelo { bloco: b.nome, rotacao_auto: true, quadro, ..Default::default() }),
                        );
                    }
                    None if n.is_empty() => {}
                    None => s.echo(format!("O desenho não tem o bloco de ponto \"{n}\".")),
                }
            }
            return Ok(Step::Continue);
        }
        let Some(m) = Self::modelo(s) else { return Ok(Step::Done) };
        if self.pedindo_rotacao {
            self.pedindo_rotacao = false;
            if let Input::Text(t) | Input::Keyword(t) = &i
                && let Some(r) = ele::ponto::numero(t)
                && r.is_finite()
            {
                ele::biblioteca::definir_modelo(s.state()?.uid, Some(ele::biblioteca::Modelo { rotacao: r, rotacao_auto: false, ..m }));
                self.giro = 0.0;
            }
            return Ok(Step::Continue);
        }
        match i {
            Input::Point(p) => {
                let r = self.rotacao(s, &m, [p.x, p.y]);
                match ele::biblioteca::inserir_ponto(s.doc_mut()?, &m.bloco, [p.x, p.y], r, &m.valores, &ele::setup::atual()) {
                    Ok(h) => {
                        ligar_ao_quadro_pai(s, h, m.quadro)?;
                        self.feitos += 1;
                    }
                    Err(e) => s.echo(e),
                }
                Ok(Step::Continue)
            }
            Input::Keyword(k) | Input::Text(k) => {
                match k.trim().to_ascii_lowercase().as_str() {
                    "bloco" | "b" => self.pedindo_bloco = true,
                    "rotacao" | "rotação" | "r" => self.pedindo_rotacao = true,
                    "girar" | "g" => self.giro = (self.giro + 90.0) % 360.0,
                    _ => s.echo("Opção inválida."),
                }
                Ok(Step::Continue)
            }
            Input::Enter => {
                if self.feitos > 0 {
                    s.echo(format!("{} ponto(s) {} inserido(s).", self.feitos, m.bloco));
                }
                Ok(Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}
