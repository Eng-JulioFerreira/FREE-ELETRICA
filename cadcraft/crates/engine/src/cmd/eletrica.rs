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
            .params("{quadro: hex, circuito?: string (default: the next free number), pontos: [hex], descricao?, esquema?: \"F+N\" | \"F+N+T\" | \"2F+T\" | \"2F+N+T\" | \"3F+T\" | \"3F+N+T\", tensao?: V} → {circuito, pontos}")
            .interactive(|_| Ok(Box::new(CircuitoM::default()))),
        CommandSpec::new("eleremoverponto", "Remove Points from Circuit", run_remover).params("{pontos: [hex]} → {removidos}").interactive(|_| Ok(Box::new(RemoverM::default()))),
        CommandSpec::new("elecomando", "Switches and Lamps", run_comando)
            .params("{interruptor: hex | interruptores: [hex] (three-way/four-way: the first gets the phase, the last feeds the lamps), luminarias: [hex], comando?: letter (default: the next free key of the switch)} → {comando, circuito, luminarias, interruptores: [{interruptor, papel}]}")
            .interactive(|_| Ok(Box::new(ComandoM::default()))),
        CommandSpec::new("elefiacao", "Wiring Tags", run_fiacao)
            .params("{quadro?: hex | TAG, eletrodutos?: [hex], limpar?: bool (erase the tags), ancora?: [x, y] (the point on the conduit), modo?: direct | leader (default: direct without `tag`, leader with it), tag?: [x, y] (leader: where the base line starts; the leader takes the angle of the two points), eletroduto?: hex (default: the conduit at `ancora`)} (default: every conduit, at its middle) → {tags, criadas, apagadas, tabelas, avisos} or {tag}")
            .interactive(|_| Ok(Box::new(FiacaoM::default()))),
        CommandSpec::new("eletrica.fiacao.modo", "Wiring Tag Mode", run_modo_fiacao)
            .params("{modo?: direct | leader} → how the next ELEFIACAO clicks place the tag, also while it runs: {modo}")
            .noundo(),
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
        CommandSpec::new("eletrica.ponto.empilhar", "Stack Points", run_empilhar)
            .params("{pontos: [hex], base: hex} → stacks the points on the base's 4x2 box: the plan shows one box with the symbols on top of each other, and the base's conduits serve them all: {base, empilhados}"),
        CommandSpec::new("eletrica.ponto.documentacao", "Point Documentation", run_ponto_documentacao)
            .params("{pontos: [hex], legenda?: text | null, descMaterial?: text | null} (null or empty: back to the family's) → {pontos}"),
        CommandSpec::new("elelegenda", "Symbol Legend", run_legenda)
            .params("{ponto: [x, y]} → draws the electrical symbol legend (electrical points and boxes) as a block at the point (moves it if there is one): {legenda, linhas}")
            .interactive(|_| Ok(Box::new(LegendaM { disciplina: ele::disciplina::Disciplina::Eletrica }))),
        CommandSpec::new("telelegenda", "Telecom Symbol Legend", run_telelegenda)
            .params("{ponto: [x, y]} → draws the telecom symbol legend (telecom points and racks) as a block at the point (moves it if there is one): {legenda, linhas}")
            .interactive(|_| Ok(Box::new(LegendaM { disciplina: ele::disciplina::Disciplina::Telecom }))),
        CommandSpec::new("eletrica.familia.excluir", "Delete Block Family", run_familia_excluir)
            .params("{bloco, comInsercoes?: bool (also deletes its insertions)} → deletes the block and its family; a factory block stays out until restored: {insercoes}"),
        CommandSpec::new("eletrica.familia.ler", "Block Family", run_familia_ler)
            .params("{bloco? (default: the block being edited)} → the family, or null")
            .noundo(),
        CommandSpec::new("eletrica.familia.definir", "Family Properties", run_familia_definir)
            .params("{bloco? (default: the block being edited), categoria?: NONE | ELECTRICAL_FIXTURE | LIGHTING_FIXTURE | LIGHTING_DEVICE | DATA_DEVICE | FIRE_SECURITY_DEVICE | ELECTRICAL_EQUIPMENT | TELECOM_ENCLOSURE | JUNCTION_BOX, dominio?: POWER | DATA_TELECOM | FIRE_SECURITY | INFRA, refAltura?: LOW | MEDIUM | HIGH | CEILING | FLOOR | FIXED, cotaFixa?: m, caixa?: NONE | 4X2 | 4X4 | OCTAGONAL | PULL_15X15 | PULL_20X20 | PULL_30X30, potencia?: VA | null, fp?: 0-1 | null, teclas?: 1-3, classeDemanda?: ILUM_TUG | AQUECIMENTO | AR | MOTORES | ELETRODOMESTICOS | FATOR_MANUAL | null, fatorDemanda?: 0-1 | null (null: automatic, from the utility's table), legenda?: text (name in the symbol legend), descMaterial?: text (description in the materials list), tensao?: V | null (default value of TENSAO; null: the board's), remover?: bool} → the family"),
        CommandSpec::new("eletrica.familia.geometria", "Family Base Geometry", run_familia_geometria)
            .params("{forma: tomada | comando | luminaria | contorno4x2 | contorno4x4} → inserts it into the block being edited: {entidades}")
            .enabled(no_editor_de_blocos),
        CommandSpec::new("eletrica.ponto.tensao", "Point Voltage", run_ponto_tensao)
            .params("{pontos: [hex], tensao: V | \"220V\" | null (null or \"QUADRO\": the family's, or the board's)} → writes TENSAO on the points (attribute, or XDATA when the block has none): {pontos, tensao}"),
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
            .params("{quadro?: hex | TAG (default: every board), compartilharPe?: bool, temperaturaAmbiente?: °C, fcaManual?: number | null, criterioDisjuntor?: correnteProjetoIb | capacidadeCaboIz, metodoInstalacao?: A1 | A2 | B1 | B2 | C | D | E | F, unifilar?: bool (also generates or updates the single-line diagram)} → applies the options, sizes the circuits, stores the result on the boards and updates tags, tables and diagrams: {quadros: [{tag, circuitos, avisos}], circuitos, avisos, unifilares}"),
        CommandSpec::new("eletrica.unifilar", "Single-Line Diagram", run_unifilar)
            .params("{quadro?: hex | TAG, ponto?: [x, y]} → generates or updates the board's single-line diagram; an existing one only moves when `ponto` is given: {unifilar}"),
        CommandSpec::new("eleunifilar", "Single-Line Diagram", run_unifilar)
            .params("{quadro?: hex | TAG, ponto?: [x, y]} → the same as eletrica.unifilar; typed, it asks for the board and the point: {unifilar}")
            .interactive(|_| Ok(Box::new(UnifilarM::default()))),
        CommandSpec::new("eletrica.quadro.frontal", "Board Front View", run_frontal)
            .params("{quadro?: hex | TAG, ponto?: [x, y]} → generates or updates the board's front view (DIN rail mounting layout); an existing one only moves when `ponto` is given: {frontal, trilhos, modulos, dispositivos: [{tipo, modulos, topo, base, trilho, posicao}]}"),
        CommandSpec::new("elequadro_frontal", "Board Front View", run_frontal)
            .params("{quadro?: hex | TAG, ponto?: [x, y]} → the same as eletrica.quadro.frontal; typed, it asks for the board and the point")
            .interactive(|_| Ok(Box::new(UnifilarM { frontal: true, ..UnifilarM::default() }))),
        CommandSpec::new("televincular", "Link Telecom Points", run_televincular)
            .params("{rack?: hex | TAG (default: the only rack, or the nearest one through the conduits), pontos?: [hex] (default: every telecom point), renumerar?: bool (renumber every point, not only the ones without ID)} → writes RACK_PAI, ID_PONTO (D-01, CFTV-01, AP-01, FO-01, TV-01), TIPO_CABO and QTD_CABOS: {pontos: [{ponto, id, rack, cabo, quantidade}], avisos}")
            .interactive(|_| Ok(Box::new(VincularM::default()))),
        CommandSpec::new("telerotear", "Size Telecom", run_telerotear)
            .params("{} → routes every telecom point to its rack in a star (NBR 14565), one whole cable per point through the conduits, sizes each telecom conduit at 40 % fill and stores the result in the drawing; updates tags and rack tables: {racks: [{rack, tag, portas, enlaces: [{ponto, id, descricao, cabo, quantidade, porta, comprimento, estimado, status}]}], semRack, trechos: [{eletroduto, cabos, eletrodutoEscolhido, ocupacao, textoIds, textoCabos}], avisos}"),
        CommandSpec::new("eletrica.telecom", "Telecom Cabling", run_telecom)
            .params("{rack?: hex | TAG} → the same result as telerotear without writing; with `rack`, only that rack: {racks, semRack, trechos, avisos}")
            .noundo(),
        CommandSpec::new("telefiacao", "Telecom Cabling Tags", run_telefiacao)
            .params("{eletrodutos?: [hex], limpar?: bool, ancora?: [x, y], modo?: direct | leader, tag?: [x, y], eletroduto?: hex} → the same as elefiacao; without a point, a tag in the middle of every telecom conduit: point IDs above (D-01..D-04), cables and diameter below (4x Cat6 Ø25)")
            .interactive(|_| Ok(Box::new(FiacaoM { telecom: true, ..FiacaoM::default() }))),
        CommandSpec::new("eletrica.telecom.ponto", "Telecom Point Cable", run_telecom_ponto)
            .params("{pontos: [hex], cabo?: telecom catalog name, quantidade?: 1 to 48, id?: text (the point number, its rack port; empty takes it off), descricao?: text (empty: back to the legend), rack?: hex | TAG | null (null unlinks the point, even with a single rack)} → edits telecom points (TIPO_CABO, QTD_CABOS, ID_PONTO, DESC_PORTA, RACK_PAI) and updates tags and tables: {pontos}"),
        CommandSpec::new("teletabela", "Rack Table", run_teletabela)
            .params("{rack?: hex | TAG (default: the first), ponto: [x, y]} → draws the rack (telecom enclosure) table: port, point ID, description, cable, 3D length and status; an existing one moves: {tabela}")
            .interactive(|_| Ok(Box::new(TabelaRackM::default()))),
        CommandSpec::new("eletrica.quadro.equilibrar", "Balance Phases", run_equilibrar)
            .params("{quadro?: hex | TAG} → unlocks the phases of every circuit, so the balancing applies to all: {destravados, desequilibrio}"),
        CommandSpec::new("eletrica.atributos", "Block Attributes", run_atributos)
            .params("{ponto: hex} → [{tag, valor, visivel, x, y, constante}]")
            .noundo(),
        CommandSpec::new("eletrica.atributo.definir", "Edit Block Attribute", run_atributo_definir)
            .params("{ponto: hex, tag, valor?: text, visivel?: bool, posicao?: [x, y]} (ALTURA follows the project's height rule) → {ponto, tag}"),
        CommandSpec::new("eletrica.bootstrap", "Prepare Electrical Drawing", run_bootstrap)
            .params("{restaurar?: bool (also brings back the deleted factory blocks)} → creates the missing base layers and blocks: {mudou}"),
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
        CommandSpec::new("eletrica.eletroduto.encaixar", "Connect Blocks In Line", run_encaixar)
            .params("{} → splits each conduit at the blocks in its path, without a new box: {divididos}"),
        CommandSpec::new("eletrica.eletroduto.excluir", "Delete Conduits", run_excluir_eletrodutos)
            .params("{eletrodutos: [hex]} → deletes them with their wiring tags: {excluidos}"),
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
            .params("{quadro?: hex | TAG, sistema?: \"F+N+T\" | \"2F+N+T\" | \"2F+T\" | \"3F+N+T\" | null, tensaoNominal?: \"127/220\" | \"220/380\" | null, cabo?: FLEX_PVC_70 | UNI_PVC_70 | HEPR_XLPE_90 | LSZH_70 | LSZH_HEPR_90 | null, alimentadoPor?: \"CONCESSIONARIA\" | hex | TAG | null, montagem?: EMBUTIR | SOBREPOR | null, observacoes?: text | null, modulos?: number | null (null: the estimate), reservas?: number | null (null: the NBR 5410 minimum), isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null (from drawings before the board's cable), disjuntorGeral?: A | null, secaoGeral?: mm² | null (main breaker and feeder section locked by hand), usarDps?: bool | null, classeDps?: \"I\" | \"II\" | \"I+II\" | null, disjuntorDps?: A | null (breaker dedicated to the SPD), metodo?: A1 | A2 | B1 | B2 | C | D | E | F | null (installation method, NBR 5410 table 33), modulosPorTrilho?: 12 | 16 | 18 | 24 | null, barramentoFase?: PENTE | ESCAMA | null, barramentoNt?: TOPO_BASE | LATERAIS | null} (null: back to what the block says) → {quadro, isolacao, config}"),
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
            "{quadro?: hex | TAG, circuito, grupoDr?: text | null, cabo?: FLEX_PVC_70 | UNI_PVC_70 | HEPR_XLPE_90 | LSZH_70 | LSZH_HEPR_90 | null, caboNome?: catalog name | null, curva?: B | C | D | null, isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null, esquema?: \"F+N\" | \"F+N+T\" | \"2F+T\" | \"2F+N+T\" | \"3F+T\" | \"3F+N+T\" | null, tensao?: V | null, fases?: \"R\" | \"RS\" ... | null (a value locks the phases), metodo?: A1 | A2 | B1 | B2 | C | D | E | F | null (installation method of the circuit), descricao?} (null: back to automatic) → {quadro, circuito}. Load type and power factor come from the block attributes TIPO_CARGA and FP.",
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
        CommandSpec::new("eletrica.dr.excluir", "Delete RCD Group", run_excluir_dr)
            .params("{quadro?: hex | TAG, grupo} → its circuits stay, without RCD: {circuitos}"),
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
    let montagem = p.get("montagem").map(|_| codigo_de(CMD, p, "montagem", ele::quadro::Montagem::de_codigo)).transpose()?;
    let observacoes = match p.get("observacoes") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(t)) if t.chars().count() <= 200 => Some(Some(t.clone())),
        Some(_) => return Err(bad(CMD, "`observacoes` must be text of up to 200 characters or null")),
    };
    let contagem = |k: &str, maximo: u32| -> Result<Option<Option<u32>>> {
        match p.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(v) => {
                let n = v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n <= maximo);
                Ok(Some(Some(n.ok_or_else(|| bad(CMD, format!("`{k}` must be a whole number from 0 to {maximo}")))?)))
            }
        }
    };
    let modulos = contagem("modulos", ele::quadro::MAXIMO_MODULOS)?;
    let reservas = contagem("reservas", ele::quadro::MAXIMO_RESERVAS)?;
    // A proteção geral, o DPS, o método e a montagem DIN: o motor confere cada valor.
    use ele::quadro as eq;
    let opcoes = [
        ("disjuntorGeral", eq::CHAVE_DISJ_GERAL),
        ("secaoGeral", eq::CHAVE_SECAO_GERAL),
        ("usarDps", eq::CHAVE_USAR_DPS),
        ("classeDps", eq::CHAVE_CLASSE_DPS),
        ("disjuntorDps", eq::CHAVE_DISJ_DPS),
        ("metodo", eq::CHAVE_METODO_QUADRO),
        ("modulosPorTrilho", eq::CHAVE_MODULOS_TRILHO),
        ("barramentoFase", eq::CHAVE_BARRAMENTO_FASE),
        ("barramentoNt", eq::CHAVE_BARRAMENTO_NT),
    ];
    let mut valores: Vec<(&str, &str, Option<String>)> = Vec::new();
    for (k, chave) in opcoes {
        match p.get(k) {
            None => {}
            Some(Value::Null) => valores.push((k, chave, None)),
            Some(Value::Bool(b)) => valores.push((k, chave, Some(if *b { "1" } else { "0" }.to_string()))),
            Some(Value::Number(n)) => valores.push((k, chave, Some(n.to_string()))),
            Some(Value::String(t)) => valores.push((k, chave, Some(t.clone()))),
            Some(_) => return Err(bad(CMD, format!("`{k}` must be a number, text, true/false or null"))),
        }
    }
    // Confere tudo numa cópia antes de gravar: um valor ruim não deixa o quadro pela metade.
    let mut prova = s.doc()?.clone();
    for (k, chave, v) in &valores {
        ele::editar::definir_opcao_quadro(&mut prova, q, chave, v.as_deref()).map_err(|e| bad(CMD, format!("`{k}`: {e}")))?;
    }
    let doc = s.doc_mut()?;
    for (_, chave, v) in &valores {
        ele::editar::definir_opcao_quadro(doc, q, chave, v.as_deref()).map_err(erro)?;
    }
    if let Some(v) = montagem {
        ele::editar::definir_montagem(doc, q, v).map_err(erro)?;
    }
    if let Some(v) = &observacoes {
        ele::editar::definir_observacoes(doc, q, v.as_deref()).map_err(erro)?;
    }
    if let Some(v) = modulos {
        ele::editar::definir_modulos(doc, q, v).map_err(erro)?;
    }
    if let Some(v) = reservas {
        ele::editar::definir_reservas(doc, q, v).map_err(erro)?;
    }
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
    if p.get("isolacao").is_some()
        || (sistema.is_none()
            && tensao.is_none()
            && cabo.is_none()
            && alimentado.is_none()
            && montagem.is_none()
            && observacoes.is_none()
            && modulos.is_none()
            && reservas.is_none()
            && valores.is_empty())
    {
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
    curva: Option<Option<ele::nbr5410::Curva>>,
    grupo_dr: Option<Option<String>>,
    esquema: Option<Option<ele::nbr5410::Esquema>>,
    tensao: Option<Option<f64>>,
    fases: Option<Option<String>>,
    metodo: Option<Option<ele::nbr5410::MetodoInstalacao>>,
    descricao: Option<String>,
}

impl Propriedades {
    fn ler(cmd: &str, p: &Value) -> Result<Self> {
        let mut d = Propriedades::default();
        if let Some(v) = p.get("metodo") {
            d.metodo = Some(match v {
                Value::Null => None,
                v => Some(
                    v.as_str()
                        .and_then(ele::nbr5410::MetodoInstalacao::de_codigo)
                        .ok_or_else(|| bad(cmd, "`metodo` must be A1, A2, B1, B2, C, D, E or F"))?,
                ),
            });
        }
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
        if let Some(v) = p.get("curva") {
            d.curva = Some(match v {
                Value::Null => None,
                v => Some(v.as_str().and_then(ele::nbr5410::Curva::de_codigo).ok_or_else(|| bad(cmd, "`curva` must be B, C or D"))?),
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
                        .ok_or_else(|| bad(cmd, "`esquema` must be \"F+N\", \"F+N+T\", \"2F+T\", \"2F+N+T\", \"3F+T\" or \"3F+N+T\""))?,
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
            && self.curva.is_none()
            && self.grupo_dr.is_none()
            && self.esquema.is_none()
            && self.tensao.is_none()
            && self.fases.is_none()
            && self.metodo.is_none()
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
        if let Some(c) = self.curva {
            ele::editar::definir_curva(doc, q, id, c).map_err(erro)?;
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
        if let Some(m) = self.metodo {
            ele::editar::definir_metodo(doc, q, id, m).map_err(erro)?;
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
        return Err(bad(CMD, "give `cabo`, `caboNome`, `curva`, `grupoDr`, `isolacao`, `esquema`, `tensao`, `fases`, `metodo` or `descricao`"));
    }
    d.gravar(s, q, &id)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_materiais(s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::materiais::levantar(s.doc()?, &analise(s)?, &ele::setup::atual()))
}

fn run_dimensionar(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.dimensionar";
    run_encaixar(s, &Value::Null)?;
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
    for k in ["compartilharPe", "temperaturaAmbiente", "fcaManual", "criterioDisjuntor", "metodoInstalacao"] {
        if let Some(v) = p.get(k) {
            opcoes.insert(k.to_string(), v.clone());
        }
    }
    if !opcoes.is_empty() {
        ele::setup::json(&Value::Object(opcoes)).map_err(|e| bad(CMD, e))?;
    }
    let setup = ele::setup::atual();
    let feitos = ele::editar::processar_calculo(s.doc_mut()?, &setup, quadro).map_err(erro)?;
    let circuitos: usize = feitos.iter().map(|f| f.circuitos).sum();
    let avisos: usize = feitos.iter().map(|f| f.avisos.len()).sum();
    let mut unifilares = 0;
    if p.get("unifilar").and_then(Value::as_bool).unwrap_or(false) {
        let alvos: Vec<Handle> = quadro.map_or_else(|| a.quadros().iter().map(|q| q.handle).collect(), |q| vec![q]);
        let a = analise(s)?;
        for q in alvos {
            ele::unifilar::gerar(s.doc_mut()?, &a, &setup, q, None).map_err(erro)?;
            unifilares += 1;
        }
    }
    Ok(json!({ "quadros": json_de(&feitos)?, "circuitos": circuitos, "avisos": avisos, "unifilares": unifilares }))
}

fn run_unifilar(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.unifilar";
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    let pos = match p.get("ponto").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(ponto_de(v).ok_or_else(|| bad(CMD, "`ponto` must be [x, y]"))?),
    };
    let h = ele::unifilar::gerar(s.doc_mut()?, &a, &ele::setup::atual(), q, pos).map_err(erro)?;
    Ok(json!({ "unifilar": h.hex() }))
}

fn run_frontal(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.quadro.frontal";
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    let pos = match p.get("ponto").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(ponto_de(v).ok_or_else(|| bad(CMD, "`ponto` must be [x, y]"))?),
    };
    let setup = ele::setup::atual();
    let h = ele::frontal::gerar(s.doc_mut()?, &a, &setup, q, pos).map_err(erro)?;
    let r = ele::quadro::rotear(s.doc()?, &a, &setup, q).map_err(erro)?;
    let mo = ele::frontal::montagem(&r);
    Ok(json!({ "frontal": h.hex(), "trilhos": mo.trilhos, "modulos": mo.modulos(), "dispositivos": json_de(&mo.dispositivos)? }))
}

/// ELEUNIFILAR e ELEQUADRO_FRONTAL digitados: o quadro (Enter: o primeiro) e o ponto de inserção.
#[derive(Default)]
struct UnifilarM {
    quadro: Option<Handle>,
    /// A vista frontal do quadro, e não o unifilar.
    frontal: bool,
}

impl Interactive for UnifilarM {
    fn name(&self) -> &'static str {
        if self.frontal { "ELEQUADRO_FRONTAL" } else { "ELEUNIFILAR" }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
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
            Some(_) if self.frontal => Prompt::new("Ponto de inserção da vista frontal", Accept::POINT),
            Some(_) => Prompt::new("Ponto de inserção do diagrama", Accept::POINT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.quadro, i) {
            (_, Input::Cancel) | (Some(_), Input::Enter) => Ok(Step::Done),
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
                let gerar = if self.frontal { ele::frontal::gerar } else { ele::unifilar::gerar };
                match gerar(s.doc_mut()?, &a, &ele::setup::atual(), q, Some([p.x, p.y])) {
                    Ok(_) => Ok(Step::Done),
                    Err(e) => {
                        s.echo(e);
                        Ok(Step::Continue)
                    }
                }
            }
            _ => Ok(Step::Continue),
        }
    }
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

fn run_bootstrap(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("restaurar").and_then(Value::as_bool).unwrap_or(false) {
        ele::biblioteca::restaurar_base(s.doc_mut()?);
    }
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
    if p.get("ancora").is_some() || p.get("tag").is_some() || p.get("modo").is_some() {
        let ancora = p.get("ancora").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ancora` ([x, y]) is required"))?;
        let modo = match p.get("modo").filter(|v| !v.is_null()) {
            Some(v) => v.as_str().and_then(ele::fiacao::ModoTag::de_codigo).ok_or_else(|| bad(CMD, "`modo` must be direct or leader"))?,
            None if p.get("tag").is_some() => ele::fiacao::ModoTag::Chamada,
            None => ele::fiacao::ModoTag::Direto,
        };
        let tag = match (modo, p.get("tag")) {
            (_, Some(v)) => ponto_de(v).ok_or_else(|| bad(CMD, "`tag` must be [x, y]"))?,
            (ele::fiacao::ModoTag::Chamada, None) => return Err(bad(CMD, "`tag` ([x, y]) is required with a leader")),
            (ele::fiacao::ModoTag::Direto, None) => ancora,
        };
        let e = match p.get("eletroduto").filter(|v| !v.is_null()) {
            Some(_) => handle(CMD, p, "eletroduto")?,
            None => {
                let (setup, m) = (ele::setup::atual(), metros(s)?);
                ele::eletroduto::eletroduto_em(s.doc()?, &setup, ancora, m)
                    .map(|(h, _)| h)
                    .ok_or_else(|| bad(CMD, "there is no conduit at `ancora`"))?
            }
        };
        preparar(s)?;
        let h = ele::fiacao::criar_tag(s.doc_mut()?, &ele::setup::atual(), e, modo, ancora, tag).map_err(|e| bad(CMD, e))?;
        return Ok(json!({ "tag": h.hex(), "modo": modo.codigo() }));
    }
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

/// Os eletrodutos por onde passam cabos de telecom.
fn eletrodutos_de_telecom(s: &Session) -> Result<Vec<Handle>> {
    let a = analise(s)?;
    Ok(ele::telecom::rotear(s.doc()?, &a, &ele::setup::atual()).trechos.iter().map(|t| t.eletroduto).collect())
}

/// O rack por handle ou TAG; sem nada, o primeiro do desenho.
fn achar_rack(a: &ele::Analise, v: Option<&Value>) -> Result<Handle> {
    let racks = ele::telecom::racks(a);
    let achado = match v.filter(|v| !v.is_null()).and_then(Value::as_str) {
        None => racks.first().copied(),
        Some(t) => racks
            .iter()
            .find(|r| Handle::parse_hex(t) == Some(r.handle))
            .or_else(|| racks.iter().find(|r| r.tag.as_deref().is_some_and(|tag| tag.eq_ignore_ascii_case(t))))
            .copied(),
    };
    achado.map(|r| r.handle).ok_or_else(|| erro("nenhum rack / quadro VDI com esse nome no desenho"))
}

fn run_televincular(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "televincular";
    let a = analise(s)?;
    let rack = match p.get("rack").filter(|v| !v.is_null()) {
        Some(v) => Some(achar_rack(&a, Some(v))?),
        None => None,
    };
    let pontos = match p.get("pontos").filter(|v| !v.is_null()) {
        Some(_) => Some(handles(CMD, p, "pontos")?),
        None => None,
    };
    let renumerar = p.get("renumerar").and_then(Value::as_bool).unwrap_or(false);
    let v = ele::telecom::vincular(s.doc_mut()?, &ele::setup::atual(), rack, pontos.as_deref(), renumerar).map_err(|e| bad(CMD, e))?;
    json_de(&v)
}

fn run_telerotear(s: &mut Session, _p: &Value) -> Result<Value> {
    let setup = ele::setup::atual();
    let a = analise(s)?;
    let t = ele::telecom::rotear(s.doc()?, &a, &setup);
    ele::telecom::gravar(s.doc_mut()?, &t);
    ele::fiacao::regenerar_se_houver(s.doc_mut()?, &setup).map_err(erro)?;
    json_de(&t)
}

fn run_telecom(s: &mut Session, p: &Value) -> Result<Value> {
    let a = analise(s)?;
    let mut t = ele::telecom::rotear(s.doc()?, &a, &ele::setup::atual());
    if let Some(v) = p.get("rack").filter(|v| !v.is_null()) {
        let r = achar_rack(&a, Some(v))?;
        t.racks.retain(|x| x.rack == r);
    }
    json_de(&t)
}

fn run_telefiacao(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "telefiacao";
    if p.get("ancora").is_some() || p.get("tag").is_some() || p.get("modo").is_some() {
        return run_fiacao(s, p);
    }
    let lista = match p.get("eletrodutos") {
        Some(_) => handles(CMD, p, "eletrodutos")?,
        None => eletrodutos_de_telecom(s)?,
    };
    if p.get("limpar").and_then(Value::as_bool).unwrap_or(false) {
        let n = ele::fiacao::limpar(s.doc_mut()?, Some(&lista));
        return Ok(json!({ "apagadas": n }));
    }
    if lista.is_empty() {
        return Err(erro("nenhum eletroduto leva cabos de telecom: vincule os pontos ao rack e ligue-os com eletrodutos"));
    }
    preparar(s)?;
    json_de(&regenerar_criando(s, &lista)?)
}

fn run_telecom_ponto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.telecom.ponto";
    let pontos = handles(CMD, p, "pontos")?;
    let cabo = match p.get("cabo").filter(|v| !v.is_null()) {
        Some(v) => Some(v.as_str().ok_or_else(|| bad(CMD, "`cabo` must be text"))?.to_string()),
        None => None,
    };
    let quantidade = match p.get("quantidade").filter(|v| !v.is_null()) {
        Some(v) => Some(v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad(CMD, "`quantidade` must be a whole number"))?),
        None => None,
    };
    let texto = |k: &str| -> Result<Option<String>> {
        match p.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(String::new())),
            Some(Value::String(t)) if t.chars().count() <= 120 => Ok(Some(t.clone())),
            Some(Value::Number(n)) => Ok(Some(n.to_string())),
            Some(_) => Err(bad(CMD, format!("`{k}` must be text of up to 120 characters"))),
        }
    };
    let rack = match p.get("rack") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(achar_rack(&analise(s)?, Some(v))?)),
    };
    let e = ele::telecom::EdicaoPonto { cabo, quantidade, id: texto("id")?, descricao: texto("descricao")?, rack };
    if e.cabo.is_none() && e.quantidade.is_none() && e.id.is_none() && e.descricao.is_none() && e.rack.is_none() {
        return Err(bad(CMD, "give `cabo`, `quantidade`, `id`, `descricao` or `rack`"));
    }
    let setup = ele::setup::atual();
    ele::telecom::editar_pontos(s.doc_mut()?, &setup, &pontos, &e).map_err(|e| bad(CMD, e))?;
    ele::fiacao::regenerar_se_houver(s.doc_mut()?, &setup).map_err(erro)?;
    Ok(json!({ "pontos": pontos.len() }))
}

fn run_teletabela(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "teletabela";
    let a = analise(s)?;
    let r = achar_rack(&a, p.get("rack"))?;
    let pos = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ponto` ([x, y]) is required"))?;
    let h = ele::telecom::inserir_tabela(s.doc_mut()?, &a, &ele::setup::atual(), r, pos).map_err(erro)?;
    Ok(json!({ "tabela": h.hex() }))
}

/// O rack num clique: o handle, se for um rack.
fn rack_valido(s: &Session, h: Handle) -> Result<bool> {
    Ok(ele::telecom::racks(&analise(s)?).iter().any(|r| r.handle == h))
}

/// TELEVINCULAR digitado: com um rack só, vincula e numera tudo de uma vez. Com mais de um: o rack (Enter: o
/// mais perto de cada ponto) e os pontos (Enter: todos).
#[derive(Default)]
struct VincularM {
    rack: Option<Option<Handle>>,
}

impl VincularM {
    fn executar(s: &mut Session, rack: Option<Handle>, pontos: Option<&[Handle]>) -> Result<Step> {
        match ele::telecom::vincular(s.doc_mut()?, &ele::setup::atual(), rack, pontos, false) {
            Ok(v) => {
                s.echo(format!("{} ponto(s) de telecom vinculado(s) e numerado(s).", v.pontos.len()));
                for a in &v.avisos {
                    s.echo(a.clone());
                }
            }
            Err(e) => s.echo(e),
        }
        Ok(Step::Done)
    }
}

impl Interactive for VincularM {
    fn name(&self) -> &'static str {
        "TELEVINCULAR"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        match ele::telecom::racks(&analise(s)?).len() {
            0 => {
                s.echo("Nenhum rack / quadro VDI no desenho: insira um com Inserir rack.");
                Ok(Step::Done)
            }
            1 => Self::executar(s, None, None),
            _ => Ok(Step::Continue),
        }
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.rack {
            None => Prompt::new("Selecione o rack", Accept::SELECT).default("O mais perto de cada ponto"),
            Some(_) => Prompt::new("Selecione os pontos de telecom", Accept::SELECT).default("Todos"),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.rack, i) {
            (_, Input::Cancel) => Ok(Step::Done),
            (None, Input::Enter) => Self::executar(s, None, None),
            (None, Input::Pick(hs)) => {
                s.set_selection(Vec::new());
                for h in hs {
                    if rack_valido(s, h)? {
                        self.rack = Some(Some(h));
                        return Ok(Step::Continue);
                    }
                }
                s.echo("Selecione um rack / quadro VDI.");
                Ok(Step::Continue)
            }
            (Some(r), Input::Enter) => Self::executar(s, r, None),
            (Some(r), Input::Pick(hs)) => {
                s.set_selection(Vec::new());
                let a = analise(s)?;
                let pontos: Vec<Handle> = ele::telecom::pontos(&a).iter().map(|p| p.handle).filter(|h| hs.contains(h)).collect();
                if pontos.is_empty() {
                    s.echo("Selecione pontos de telecom.");
                    return Ok(Step::Continue);
                }
                Self::executar(s, r, Some(&pontos))
            }
            _ => Ok(Step::Continue),
        }
    }
}

/// TELETABELA digitado: o rack (Enter: o primeiro) e o ponto de inserção.
#[derive(Default)]
struct TabelaRackM {
    rack: Option<Handle>,
}

impl Interactive for TabelaRackM {
    fn name(&self) -> &'static str {
        "TELETABELA"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        let a = analise(s)?;
        let racks = ele::telecom::racks(&a);
        if racks.is_empty() {
            s.echo("Nenhum rack / quadro VDI no desenho: insira um com Inserir rack.");
            return Ok(Step::Done);
        }
        if let [r] = racks.as_slice() {
            self.rack = Some(r.handle);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.rack {
            None => Prompt::new("Selecione o rack", Accept::SELECT).default("Primeiro"),
            Some(_) => Prompt::new("Ponto de inserção da tabela do rack", Accept::POINT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.rack, i) {
            (_, Input::Cancel) | (Some(_), Input::Enter) => Ok(Step::Done),
            (None, Input::Enter) => {
                self.rack = ele::telecom::racks(&analise(s)?).first().map(|r| r.handle);
                Ok(Step::Continue)
            }
            (None, Input::Pick(hs)) => {
                s.set_selection(Vec::new());
                for h in hs {
                    if rack_valido(s, h)? {
                        self.rack = Some(h);
                        return Ok(Step::Continue);
                    }
                }
                s.echo("Selecione um rack / quadro VDI.");
                Ok(Step::Continue)
            }
            (Some(r), Input::Point(p)) => {
                let a = analise(s)?;
                match ele::telecom::inserir_tabela(s.doc_mut()?, &a, &ele::setup::atual(), r, [p.x, p.y]) {
                    Ok(_) => Ok(Step::Done),
                    Err(e) => {
                        s.echo(e);
                        Ok(Step::Continue)
                    }
                }
            }
            _ => Ok(Step::Continue),
        }
    }
}

fn run_modo_fiacao(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.fiacao.modo";
    let uid = s.state()?.uid;
    if let Some(v) = p.get("modo").filter(|v| !v.is_null()) {
        let m = v.as_str().and_then(ele::fiacao::ModoTag::de_codigo).ok_or_else(|| bad(CMD, "`modo` must be direct or leader"))?;
        ele::fiacao::definir_modo(uid, m);
    }
    Ok(json!({ "modo": ele::fiacao::modo(uid).codigo() }))
}

fn run_regen(s: &mut Session, _p: &Value) -> Result<Value> {
    let r = ele::fiacao::regenerar(s.doc_mut()?, &ele::setup::atual(), None).map_err(erro)?;
    let mut v = json_de(&r)?;
    if let Value::Object(m) = &mut v {
        m.insert("message".into(), Value::String(resumo_regeneracao(&r)));
    }
    Ok(v)
}

/// ELEFIACAO digitado, até Enter. Direto no eletroduto: um clique no trecho. Com linha de chamada: o ponto no
/// eletroduto e o início da base. A mini-barra (ou `Direto` e `Chamada` digitados) troca o modo. Enter no
/// começo põe uma tag no meio de todos os eletrodutos. TELEFIACAO é o mesmo, e o Enter vale só para os
/// eletrodutos de telecom.
#[derive(Default)]
struct FiacaoM {
    /// O primeiro clique: o eletroduto e o ponto nele.
    ancora: Option<(Handle, [f64; 2])>,
    feitas: usize,
    telecom: bool,
}

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
        if self.telecom { "TELEFIACAO" } else { "ELEFIACAO" }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        preparar(s)?;
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        let modo = s.state().map(|st| ele::fiacao::modo(st.uid)).unwrap_or_default();
        let onde = match modo {
            ele::fiacao::ModoTag::Direto => "Clique no eletroduto, onde a tag fica",
            ele::fiacao::ModoTag::Chamada => "Clique no eletroduto, onde a chamada nasce",
        };
        match (self.ancora, self.feitas) {
            (Some(_), _) => Prompt::new("Clique onde a base da tag começa", Accept::POINT),
            (None, 0) => Prompt::new(onde, Accept::POINT).default("Todos, no meio").kw(&["Direto", "Chamada"]),
            (None, _) => Prompt::new(format!("{onde} (Enter termina)"), Accept::POINT).kw(&["Direto", "Chamada"]),
        }
    }
    fn preview(&self, s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let m = metros(s).unwrap_or(1.0);
        self.ancora.map_or_else(Vec::new, |(_, a)| {
            ele::fiacao::Chamada { modo: ele::fiacao::ModoTag::Chamada, ancora: a, tag: [cursor.x, cursor.y] }.linhas(m)
        })
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Cancel => Ok(Step::Done),
            Input::Enter if self.ancora.is_some() => {
                self.ancora = None;
                Ok(Step::Continue)
            }
            Input::Enter if self.feitas > 0 => Ok(Step::Done),
            Input::Enter => {
                let todos = if self.telecom { eletrodutos_de_telecom(s)? } else { todos_os_eletrodutos(s)? };
                if todos.is_empty() {
                    s.echo("Nenhum eletroduto leva cabos de telecom: vincule os pontos ao rack e ligue-os com eletrodutos.");
                    return Ok(Step::Done);
                }
                Self::executar(s, todos)
            }
            Input::Keyword(k) | Input::Text(k) => {
                match ele::fiacao::ModoTag::de_codigo(&k).or_else(|| match k.trim().to_ascii_lowercase().as_str() {
                    "direto" => Some(ele::fiacao::ModoTag::Direto),
                    "chamada" => Some(ele::fiacao::ModoTag::Chamada),
                    _ => None,
                }) {
                    Some(m) => {
                        ele::fiacao::definir_modo(s.state()?.uid, m);
                        self.ancora = None;
                    }
                    None => s.echo("Opção inválida."),
                }
                Ok(Step::Continue)
            }
            Input::Point(q) => {
                let q = [q.x, q.y];
                let modo = ele::fiacao::modo(s.state()?.uid);
                let criar = |s: &mut Session, h: Handle, a: [f64; 2], t: [f64; 2]| {
                    ele::fiacao::criar_tag(s.doc_mut()?, &ele::setup::atual(), h, modo, a, t).map_err(erro)
                };
                match self.ancora.take() {
                    None => {
                        let (setup, m) = (ele::setup::atual(), metros(s)?);
                        match (ele::eletroduto::eletroduto_em(s.doc()?, &setup, q, m), modo) {
                            (Some((h, p)), ele::fiacao::ModoTag::Direto) => match criar(s, h, p, p) {
                                Ok(_) => self.feitas += 1,
                                Err(e) => s.echo(e.to_string()),
                            },
                            (Some((h, p)), ele::fiacao::ModoTag::Chamada) => self.ancora = Some((h, p)),
                            (None, _) => s.echo("Clique num eletroduto."),
                        }
                    }
                    Some((h, a)) => match criar(s, h, a, q) {
                        Ok(_) => self.feitas += 1,
                        Err(e) => s.echo(e.to_string()),
                    },
                }
                Ok(Step::Continue)
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

/// Põe outro interruptor no comando de `sw`: a mesma letra e o mesmo circuito (paralelos e intermediários).
fn juntar_interruptor(s: &mut Session, sw: &Interruptor, h: Handle) -> Result<bool> {
    if h == sw.h {
        return Ok(false);
    }
    let Some(outro) = preparar_interruptor(s, h, Some(&sw.letra))? else { return Ok(false) };
    if let (Some(c), Some(q)) = (&sw.circuito, sw.quadro)
        && (outro.circuito.as_deref() != Some(c.as_str()) || outro.quadro != Some(q))
    {
        ele::editar::atribuir_circuito(s.doc_mut()?, h, q, c).map_err(erro)?;
    }
    s.echo(format!("Interruptor {}: comando {}.", outro.rotulo, sw.letra));
    Ok(true)
}

/// Grava os papéis dos interruptores do comando e a lista deles nas luminárias.
fn vincular_comando(
    s: &mut Session,
    sw: &Interruptor,
    outros: &[Handle],
    luminarias: &[Handle],
) -> Result<Vec<(Handle, ele::editar::PapelInterruptor)>> {
    let todos: Vec<Handle> = std::iter::once(sw.h).chain(outros.iter().copied()).collect();
    ele::editar::vincular_interruptores(s.doc_mut()?, &sw.letra, &todos, luminarias).map_err(erro)
}

fn run_comando(s: &mut Session, p: &Value) -> Result<Value> {
    // Um interruptor, ou vários num mesmo comando (paralelos e intermediários).
    let interruptores =
        if p.get("interruptores").is_some() { handles("elecomando", p, "interruptores")? } else { vec![handle("elecomando", p, "interruptor")?] };
    let Some((&h, outros)) = interruptores.split_first() else { return Err(bad("elecomando", "`interruptores` is empty")) };
    let lista = handles("elecomando", p, "luminarias")?;
    let letra = p.get("comando").and_then(Value::as_str).filter(|l| !l.trim().is_empty());
    let mut sw =
        preparar_interruptor(s, h, letra)?.ok_or_else(|| bad("elecomando", "`interruptor` is not a switch block (TIPO_PONTO = INTERRUPTOR)"))?;
    let mut feitos = Vec::new();
    let mut lums = Vec::new();
    for l in lista {
        if ligar_luminaria(s, &mut sw, l)? {
            feitos.push(l.hex());
            lums.push(l);
        }
    }
    let mut juntos = Vec::new();
    for o in outros {
        if !juntar_interruptor(s, &sw, *o)? {
            return Err(bad("elecomando", format!("{} is not another switch block (TIPO_PONTO = INTERRUPTOR)", o.hex())));
        }
        juntos.push(*o);
    }
    let papeis = vincular_comando(s, &sw, &juntos, &lums)?;
    let papeis: Vec<Value> = papeis.iter().map(|(h, r)| json!({ "interruptor": h.hex(), "papel": r.codigo() })).collect();
    Ok(json!({ "comando": sw.letra, "circuito": sw.circuito, "luminarias": feitos, "interruptores": papeis }))
}

#[derive(Default)]
struct ComandoM {
    interruptor: Option<Interruptor>,
    /// Os outros interruptores do mesmo comando (paralelos e intermediários), na ordem escolhida.
    outros: Vec<Handle>,
    luminarias: Vec<Handle>,
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
            Some(sw) => Prompt::new(
                format!("Selecione as luminárias e os outros interruptores (paralelos, intermediários) do comando {}", sw.letra),
                Accept::SELECT,
            )
            .kw(&["Letra"]),
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
                            // Outro interruptor entra no mesmo comando: paralelos e intermediários.
                            let e_interruptor = analise(s)?.ponto(h).is_some_and(|p| p.tipo == TipoPonto::Interruptor);
                            if e_interruptor {
                                if !self.outros.contains(&h) && juntar_interruptor(s, sw, h)? {
                                    self.outros.push(h);
                                }
                            } else if ligar_luminaria(s, sw, h)? {
                                self.feitos += 1;
                                self.luminarias.push(h);
                            }
                        }
                        vincular_comando(s, sw, &self.outros, &self.luminarias)?;
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
    encaixar_trecho(s, h)?;
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

fn run_excluir_eletrodutos(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.excluir";
    let hs = handles(CMD, p, "eletrodutos")?;
    let n = ele::eletroduto::excluir(s.doc_mut()?, &ele::setup::atual(), &hs).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "excluidos": n }))
}

fn run_familia_excluir(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.familia.excluir";
    let bloco = texto(CMD, p, "bloco")?;
    let com = p.get("comInsercoes").and_then(Value::as_bool).unwrap_or(false);
    let n = ele::biblioteca::excluir_bloco(s.doc_mut()?, &bloco, com).map_err(|e| bad(CMD, e))?;
    // A legenda desenha os símbolos: não pode ficar com o que saiu.
    let (a, m) = (analise(s)?, metros(s)?);
    ele::legenda::regenerar(s.doc_mut()?, &a, m);
    Ok(json!({ "insercoes": n }))
}

fn run_excluir_dr(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.dr.excluir";
    let a = analise(s)?;
    let q = achar_quadro(&a, p.get("quadro"))?;
    let grupo = texto(CMD, p, "grupo")?;
    let quadro = a.ponto(q).ok_or_else(|| bad(CMD, "board not found"))?;
    let circuitos: Vec<String> = ele::quadro::propriedades(quadro)
        .into_iter()
        .filter(|(_, v)| v.grupo_dr.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(grupo.trim())))
        .map(|(k, _)| k)
        .collect();
    if circuitos.is_empty() {
        return Err(bad(CMD, format!("there is no RCD group `{grupo}` on this board")));
    }
    for c in &circuitos {
        ele::editar::definir_grupo_dr(s.doc_mut()?, q, c, None).map_err(erro)?;
    }
    Ok(json!({ "circuitos": circuitos.len() }))
}

fn run_encaixar(s: &mut Session, _p: &Value) -> Result<Value> {
    let m = metros(s)?;
    let n = ele::eletroduto::encaixar(s.doc_mut()?, &ele::setup::atual(), m, ele::eletroduto::TOLERANCIA_EM_LINHA, None, None).map_err(erro)?;
    Ok(json!({ "divididos": n }))
}

/// Um trecho novo pode passar por blocos que já existem.
fn encaixar_trecho(s: &mut Session, h: Handle) -> Result<()> {
    let (setup, m) = (ele::setup::atual(), metros(s)?);
    ele::eletroduto::encaixar(s.doc_mut()?, &setup, m, ele::eletroduto::TOLERANCIA_EM_LINHA, None, Some(&[h])).map_err(erro)?;
    Ok(())
}

/// Um ponto novo de parede no lugar de outro empilha na caixa dele; senão, pode cair no caminho de um
/// eletroduto que já existe.
fn assentar_ponto(s: &mut Session, h: Handle) -> Result<()> {
    let m = metros(s)?;
    if ele::biblioteca::empilhar(s.doc_mut()?, h, None, m).map_err(erro)?.is_some() {
        return Ok(());
    }
    encaixar_ponto(s, h)
}

/// Um ponto novo pode cair no caminho de um eletroduto que já existe.
fn encaixar_ponto(s: &mut Session, h: Handle) -> Result<()> {
    let (setup, m) = (ele::setup::atual(), metros(s)?);
    ele::eletroduto::encaixar(s.doc_mut()?, &setup, m, ele::eletroduto::TOLERANCIA_EM_LINHA, Some(&[h]), None).map_err(erro)?;
    Ok(())
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
                Ok(novo) => {
                    encaixar_trecho(s, novo)?;
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

fn run_empilhar(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.empilhar";
    let hs = handles(CMD, p, "pontos")?;
    let base = handle(CMD, p, "base")?;
    let m = metros(s)?;
    let mut feito = None;
    for h in &hs {
        feito = ele::biblioteca::empilhar(s.doc_mut()?, *h, Some(base), m).map_err(|e| bad(CMD, e))?;
    }
    Ok(json!({ "base": feito.map(|b| b.hex()), "empilhados": hs.len() }))
}

fn run_ponto_documentacao(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.documentacao";
    let hs = handles(CMD, p, "pontos")?;
    if p.get("legenda").is_none() && p.get("descMaterial").is_none() {
        return Err(bad(CMD, "give `legenda` or `descMaterial`"));
    }
    // Vazio ou null volta ao texto da família.
    let campo = |k: &str| -> Result<Option<Option<String>>> {
        match p.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(t)) if t.chars().count() <= 200 => Ok(Some(Some(t.trim().to_string()).filter(|t| !t.is_empty()))),
            Some(_) => Err(bad(CMD, format!("`{k}` must be text of up to 200 characters or null"))),
        }
    };
    let (legenda, material) = (campo("legenda")?, campo("descMaterial")?);
    for h in &hs {
        if ele::ponto::ler_ponto(s.doc()?, *h).is_none() {
            return Err(bad(CMD, format!("{} is not an electrical point", h.hex())));
        }
    }
    let doc = s.doc_mut()?;
    for h in &hs {
        if let Some(v) = &legenda {
            ele::editar::definir(doc, *h, ele::ponto::TAGS_LEGENDA, v.as_deref()).map_err(erro)?;
        }
        if let Some(v) = &material {
            ele::editar::definir(doc, *h, ele::ponto::TAGS_DESC_MATERIAL, v.as_deref()).map_err(erro)?;
        }
    }
    Ok(json!({ "pontos": hs.len() }))
}

fn run_legenda(s: &mut Session, p: &Value) -> Result<Value> {
    legenda_da_disciplina(s, p, "elelegenda", ele::disciplina::Disciplina::Eletrica)
}

fn run_telelegenda(s: &mut Session, p: &Value) -> Result<Value> {
    legenda_da_disciplina(s, p, "telelegenda", ele::disciplina::Disciplina::Telecom)
}

fn legenda_da_disciplina(s: &mut Session, p: &Value, cmd: &str, d: ele::disciplina::Disciplina) -> Result<Value> {
    let pos = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(cmd, "`ponto` ([x, y]) is required"))?;
    let (a, m) = (analise(s)?, metros(s)?);
    let h = ele::legenda::inserir(s.doc_mut()?, &a, pos, m, Some(d)).map_err(erro)?;
    Ok(json!({ "legenda": h.hex(), "linhas": ele::legenda::linhas(&a, Some(d)).len() }))
}

/// ELELEGENDA e TELELEGENDA digitados: o ponto de inserção.
struct LegendaM {
    disciplina: ele::disciplina::Disciplina,
}

impl Interactive for LegendaM {
    fn name(&self) -> &'static str {
        if self.disciplina == ele::disciplina::Disciplina::Telecom { "TELELEGENDA" } else { "ELELEGENDA" }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Ponto de inserção da legenda", Accept::POINT)
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Point(q) => {
                let cmd = if self.disciplina == ele::disciplina::Disciplina::Telecom { "telelegenda" } else { "elelegenda" };
                legenda_da_disciplina(s, &json!({ "ponto": [q.x, q.y] }), cmd, self.disciplina)?;
                Ok(Step::Done)
            }
            Input::Cancel | Input::Enter => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
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
                legenda: a.legenda,
                desc_material: a.desc_material,
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
        if let Some(v) = p.get("legenda") {
            f.legenda = v.as_str().map(str::to_string).or_else(|| v.is_null().then(String::new)).ok_or_else(|| bad(CMD, "`legenda` must be text"))?;
        }
        if let Some(v) = p.get("descMaterial") {
            f.desc_material =
                v.as_str().map(str::to_string).or_else(|| v.is_null().then(String::new)).ok_or_else(|| bad(CMD, "`descMaterial` must be text"))?;
        }
        if let Some(v) = p.get("teclas").filter(|v| !v.is_null()) {
            f.teclas = v.as_u64().and_then(|n| u8::try_from(n).ok()).ok_or_else(|| bad(CMD, "`teclas` must be from 1 to 3"))?;
        }
        if let Some(v) = p.get("tensao") {
            f.tensao = tensao_de(CMD, v)?;
        }
        Some(f)
    };
    ele::familia::gravar(s.doc_mut()?, &bloco, familia.as_ref()).map_err(|e| bad(CMD, e))?;
    // A tensão padrão é o valor inicial do TENSAO: no bloco em edição, o ATTDEF acompanha.
    if em_edicao && p.get("tensao").is_some() {
        let texto = familia.as_ref().and_then(|f| f.tensao).map_or(String::new(), |v| format!("{}V", ele::ponto::br(v)));
        ele::biblioteca::definir_padrao_do_atributo(s.doc_mut()?, "TENSAO", &texto);
    }
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
    let tensao = ele::familia::ler(s.doc()?, &bloco).and_then(|f| f.tensao);
    let n = ele::biblioteca::gerar_atributos(s.doc_mut()?, categoria, m, tensao).map_err(|e| bad(CMD, e))?;
    Ok(json!({ "criados": n }))
}

/// Uma tensão em V: número (127), texto ("220V", "127 V") ou `null`/"QUADRO" (a do quadro). De 1 a 1000 V.
fn tensao_de(cmd: &str, v: &Value) -> Result<Option<f64>> {
    let valor = match v {
        Value::Null => return Ok(None),
        Value::Number(n) => n.as_f64(),
        Value::String(t) if t.trim().is_empty() || t.trim().eq_ignore_ascii_case("QUADRO") => return Ok(None),
        Value::String(t) => ele::ponto::numero(t),
        _ => None,
    };
    valor
        .filter(|x| x.is_finite() && *x > 0.0 && *x <= 1000.0)
        .map(Some)
        .ok_or_else(|| bad(cmd, "`tensao` must be a voltage from 1 to 1000 V, or null (the board's)"))
}

fn run_ponto_tensao(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.ponto.tensao";
    let hs = handles(CMD, p, "pontos")?;
    let tensao = tensao_de(CMD, p.get("tensao").ok_or_else(|| bad(CMD, "`tensao` (V or null) is required"))?)?;
    let texto = tensao.map(|v| format!("{}V", ele::ponto::br(v)));
    for h in &hs {
        if ele::ponto::ler_ponto(s.doc()?, *h).is_none() {
            return Err(bad(CMD, format!("the object {} is not an electrical point", h.hex())));
        }
    }
    for h in &hs {
        // No atributo TENSAO, se o bloco tem; senão, no XDATA. Vazio: a da família ou a do quadro.
        ele::editar::definir(s.doc_mut()?, *h, ele::ponto::TAGS_TENSAO, texto.as_deref()).map_err(erro)?;
    }
    Ok(json!({ "pontos": hs.len(), "tensao": tensao }))
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
        assentar_ponto(s, h)?;
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
                        assentar_ponto(s, h)?;
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
