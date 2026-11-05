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
//! desfazer, e o que muda o quadro atualiza as tags e as tabelas que já existem.
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
            .params("{de: hex | [x, y], para: hex | [x, y], instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\", flecha?: number (arc bulge as a fraction of the chord: + left, - right, 0 straight; default 0.15)} (a point on a conduit splits it with a pull box) → {eletroduto}")
            .interactive(|_| Ok(Box::new(EletrodutoM::default()))),
        CommandSpec::new("eleponto", "Insert Electrical Point", run_ponto)
            .params("{bloco, pontos: [[x, y]], rotacao?: degrees, rotacaoAuto?: bool (align to the nearest wall), valores?: {TAG: value}} → {pontos: [hex]}")
            .interactive(|_| Ok(Box::new(PontoM::default()))),
        CommandSpec::new("eletrica.bootstrap", "Prepare Electrical Drawing", run_bootstrap).params("{} → creates the missing base layers and blocks: {mudou}"),
        CommandSpec::new("eletrica.blocos", "Electrical Point Blocks", run_blocos)
            .params("{} → [{nome, tipo, inseridos, atributos: [{tag, padrao, prompt, constante}]}]")
            .noundo(),
        CommandSpec::new("eletrica.ponto.modelo", "Point Template", run_modelo)
            .params("{bloco, rotacao?: degrees, rotacaoAuto?: bool (default true), valores?: {TAG: value}} → what ELEPONTO inserts, also while it runs")
            .noundo(),
        CommandSpec::new("eletrica.ponto.excluir", "Delete Electrical Points", run_excluir_pontos).params("{pontos: [hex]} → {excluidos}"),
        CommandSpec::new("eletrica.eletroduto.modelo", "Conduit Template", run_modelo_eletroduto)
            .params("{instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\", flecha?: number} → what ELEELETRODUTO uses, also while it runs")
            .noundo(),
        CommandSpec::new("eletrica.eletroduto.dividir", "Split Conduit", run_dividir).params("{eletroduto: hex, ponto: [x, y]} → splits it with a pull box: {caixa}"),
        CommandSpec::new("eletrica.eletroduto.definir", "Conduit Properties", run_definir_eletroduto)
            .params("{eletroduto: hex, diametro?: mm | null (a number locks it; null: back to automatic), instalacao?: \"CEILING\" | \"FLOOR\" | \"WALL\"} → {eletroduto}"),
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
            .params("{quadro?: hex | TAG, isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null (null: back to the settings' default)} → {quadro, isolacao}"),
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
            "{quadro?: hex | TAG, circuito, isolacao?: \"PVC\" | \"EPR\" | \"XLPE\" | null, esquema?: \"F+N\" | \"F+N+T\" | \"2F+T\" | \"3F+N+T\" | null, tensao?: V | null, fases?: \"R\" | \"RS\" ... | null (a value locks the phases), descricao?} (null: back to automatic) → {quadro, circuito}. Load type and power factor come from the block attributes TIPO_CARGA and FP.",
        ),
        CommandSpec::new("eletrica.demanda.tabelas", "Demand Tables", run_tabelas_demanda)
            .params("{} → the utilities' demand tables (id, nome, fonte, edificacoes, aparelhos, arCondicionado, bombas)")
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

/// O gatilho das edições: atualiza as tags e as tabelas que já existem, sem mexer na posição delas.
/// Um erro aqui não desfaz a edição: é avisado na linha de comando.
fn atualizar_gerados(s: &mut Session) -> Result<()> {
    let doc = s.doc()?;
    if ele::fiacao::tags(doc).is_empty() && ele::tabela::tabelas(doc).is_empty() {
        return Ok(());
    }
    if let Err(e) = ele::fiacao::regenerar_se_houver(s.doc_mut()?, &ele::setup::atual()) {
        s.echo(format!("Fiação e tabelas não atualizadas: {e}"));
    }
    Ok(())
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
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "circuito": circuito }))
}

fn run_quadro(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let isolacao = match p.get("isolacao") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_str()
                .and_then(ele::nbr5410::Isolacao::de_codigo)
                .ok_or_else(|| bad("eletrica.quadro", "`isolacao` must be \"PVC\", \"EPR\" or \"XLPE\""))?,
        ),
    };
    ele::editar::definir_isolacao(s.doc_mut()?, q, isolacao).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "isolacao": isolacao.map(|i| i.codigo()) }))
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
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_descrever(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto("eletrica.circuito.descrever", p, "circuito")?;
    let descricao = p.get("descricao").and_then(Value::as_str).unwrap_or("");
    ele::editar::definir_descricao(s.doc_mut()?, q, &id, Some(descricao)).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_excluir(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let id = texto("eletrica.circuito.excluir", p, "circuito")?;
    let n = ele::editar::excluir_circuito(s.doc_mut()?, q, &id).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id, "pontos": n }))
}

fn run_juntar(s: &mut Session, p: &Value) -> Result<Value> {
    let q = achar_quadro(&analise(s)?, p.get("quadro"))?;
    let de = texto("eletrica.circuito.juntar", p, "de")?;
    let para = texto("eletrica.circuito.juntar", p, "para")?;
    let n = ele::editar::juntar_circuitos(s.doc_mut()?, q, &de, &para).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "de": de, "para": para, "pontos": n }))
}

/// O que `eletrica.circuito.definir` e `elecircuito` mudam: `None` não mexe, `Some(None)` volta ao
/// automático. Tudo é validado antes de gravar qualquer coisa.
#[derive(Default)]
struct Propriedades {
    isolacao: Option<Option<ele::nbr5410::Isolacao>>,
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
        self.isolacao.is_none() && self.esquema.is_none() && self.tensao.is_none() && self.fases.is_none() && self.descricao.is_none()
    }

    fn gravar(&self, s: &mut Session, q: Handle, id: &str) -> Result<()> {
        let doc = s.doc_mut()?;
        if let Some(i) = self.isolacao {
            ele::editar::definir_isolacao_circuito(doc, q, id, i).map_err(erro)?;
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
        return Err(bad(CMD, "give `isolacao`, `esquema`, `tensao`, `fases` or `descricao`"));
    }
    d.gravar(s, q, &id)?;
    atualizar_gerados(s)?;
    Ok(json!({ "quadro": q.hex(), "circuito": id }))
}

fn run_tabelas_demanda(_s: &mut Session, _p: &Value) -> Result<Value> {
    json_de(&ele::demanda::tabelas())
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
    atualizar_gerados(s)?;
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
        atualizar_gerados(s)?;
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
    atualizar_gerados(s)?;
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
            Input::Cancel => {
                atualizar_gerados(s)?;
                Ok(Step::Done)
            }
            Input::Enter => {
                s.echo(format!("{} ponto(s) sem circuito.", self.feitos));
                atualizar_gerados(s)?;
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
    atualizar_gerados(s)?;
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
            Input::Cancel => {
                atualizar_gerados(s)?;
                Ok(Step::Done)
            }
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
                atualizar_gerados(s)?;
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

fn run_eletroduto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eleeletroduto";
    let modelo = ele::eletroduto::modelo(s.state()?.uid);
    let instalacao = match p.get("instalacao") {
        None | Some(Value::Null) => match p.get("piso").and_then(Value::as_bool) {
            Some(true) => Instalacao::Piso,
            Some(false) => Instalacao::Teto,
            None => modelo.instalacao,
        },
        Some(v) => instalacao_de(CMD, v)?,
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
    let h = ele::eletroduto::inserir(s.doc_mut()?, de, para, instalacao, flecha).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "eletroduto": h.hex(), "de": de.hex(), "para": para.hex() }))
}

fn run_modelo_eletroduto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.modelo";
    let uid = s.state()?.uid;
    let mut m = ele::eletroduto::modelo(uid);
    if let Some(v) = p.get("instalacao").filter(|v| !v.is_null()) {
        m.instalacao = instalacao_de(CMD, v)?;
    }
    if let Some(v) = p.get("flecha").filter(|v| !v.is_null()) {
        m.flecha = numero_finito(CMD, v, "flecha")?.clamp(-ele::eletroduto::FLECHA_MAXIMA, ele::eletroduto::FLECHA_MAXIMA);
    }
    ele::eletroduto::definir_modelo(uid, m);
    json_de(&m)
}

fn run_dividir(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eletrica.eletroduto.dividir";
    let e = handle(CMD, p, "eletroduto")?;
    let q = p.get("ponto").and_then(ponto_de).ok_or_else(|| bad(CMD, "`ponto` ([x, y]) is required"))?;
    let m = metros(s)?;
    let caixa = ele::eletroduto::dividir(s.doc_mut()?, &ele::setup::atual(), e, q, m).map_err(erro)?;
    atualizar_gerados(s)?;
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
    if diametro.is_none() && instalacao.is_none() {
        return Err(bad(CMD, "give `diametro` or `instalacao`"));
    }
    if let Some(d) = diametro {
        ele::eletroduto::travar_diametro(s.doc_mut()?, e, d).map_err(erro)?;
    }
    if let Some(i) = instalacao {
        ele::eletroduto::definir_instalacao(s.doc_mut()?, e, i).map_err(erro)?;
    }
    atualizar_gerados(s)?;
    Ok(json!({ "eletroduto": e.hex() }))
}

/// ELEELETRODUTO digitado: um clique em cada bloco (ou no meio de um eletroduto, que recebe uma
/// caixa de passagem); cada um é ligado ao anterior, até Enter.
struct EletrodutoM {
    anterior: Option<Handle>,
    /// Lado do arco: 1 à esquerda de quem vai do anterior ao próximo, -1 à direita.
    lado: f64,
    pedindo_flecha: bool,
    feitos: usize,
}

impl Default for EletrodutoM {
    fn default() -> Self {
        EletrodutoM { anterior: None, lado: 1.0, pedindo_flecha: false, feitos: 0 }
    }
}

impl EletrodutoM {
    /// Liga o bloco ao anterior.
    fn clicar(&mut self, s: &mut Session, h: Handle) -> Result<Step> {
        if let Some(a) = self.anterior {
            let m = ele::eletroduto::modelo(s.state()?.uid);
            match ele::eletroduto::inserir(s.doc_mut()?, a, h, m.instalacao, self.lado * m.flecha) {
                Ok(_) => self.feitos += 1,
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
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        let m = s.state().map(|st| ele::eletroduto::modelo(st.uid)).unwrap_or_default();
        if self.pedindo_flecha {
            return Prompt::new("Flecha do arco (fração da corda, de 0 a 0,5)", Accept::TEXT).default(ele::ponto::br(m.flecha));
        }
        let onde = m.instalacao.nome().to_lowercase();
        let msg = match self.anterior {
            None => format!("Clique no primeiro bloco ou no meio de um eletroduto ({onde})"),
            Some(_) => format!("Clique no próximo bloco ou no meio de um eletroduto ({onde}; Enter termina)"),
        };
        let chaves: &[&str] =
            if self.anterior.is_some() { &["Teto", "Piso", "Parede", "Inverter", "Flecha"] } else { &["Teto", "Piso", "Parede", "Flecha"] };
        Prompt::new(msg, Accept::POINT).kw(chaves)
    }
    fn preview(&self, s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let Some(a) = self.anterior.and_then(|h| ele::ponto::ler_ponto(s.doc().ok()?, h)).map(|p| p.posicao) else { return Vec::new() };
        if (a[0] - cursor.x).hypot(a[1] - cursor.y) <= 0.0 {
            return Vec::new();
        }
        let flecha = s.state().map(|st| ele::eletroduto::modelo(st.uid).flecha).unwrap_or(ele::eletroduto::FLECHA_PADRAO);
        let pts: Vec<Vec2> =
            ele::eletroduto::pontos_do_arco(a, [cursor.x, cursor.y], self.lado * flecha, 9).into_iter().map(|p| Vec2::new(p[0], p[1])).collect();
        vec![EntityKind::Spline(cadcraft_doc::geom::Spline::from_fit_points(&pts))]
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let uid = s.state()?.uid;
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
                atualizar_gerados(s)?;
                Ok(Step::Done)
            }
            Input::Keyword(k) | Input::Text(k) => {
                let mut m = ele::eletroduto::modelo(uid);
                match k.trim().to_ascii_lowercase().as_str() {
                    "teto" => m.instalacao = Instalacao::Teto,
                    "piso" => m.instalacao = Instalacao::Piso,
                    "parede" => m.instalacao = Instalacao::Parede,
                    "inverter" => self.lado = -self.lado,
                    "flecha" => self.pedindo_flecha = true,
                    _ => s.echo("Opção inválida."),
                }
                ele::eletroduto::definir_modelo(uid, m);
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

fn rotacao_de(cmd: &str, p: &Value) -> Result<f64> {
    match p.get("rotacao") {
        None | Some(Value::Null) => Ok(0.0),
        Some(v) => v.as_f64().filter(|r| r.is_finite()).ok_or_else(|| bad(cmd, "`rotacao` must be a number (degrees)")),
    }
}

fn run_ponto(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "eleponto";
    let bloco = texto(CMD, p, "bloco")?;
    let (rotacao, valores) = (rotacao_de(CMD, p)?, valores_de(CMD, p)?);
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
    let setup = ele::setup::atual();
    let m = metros(s)?;
    let mut feitos = Vec::new();
    for pos in posicoes {
        let r = if auto { ele::biblioteca::alinhar_a_parede(s.doc()?, &setup, pos, m).unwrap_or(rotacao) } else { rotacao };
        feitos.push(ele::biblioteca::inserir(s.doc_mut()?, &bloco, pos, r, &valores).map_err(erro)?.hex());
    }
    atualizar_gerados(s)?;
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
        valores: valores_de(CMD, p)?,
    };
    ele::biblioteca::definir_modelo(s.state()?.uid, Some(modelo.clone()));
    json_de(&modelo)
}

fn run_excluir_pontos(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = handles("eletrica.ponto.excluir", p, "pontos")?;
    let n = ele::biblioteca::excluir(s.doc_mut()?, &hs).map_err(erro)?;
    atualizar_gerados(s)?;
    Ok(json!({ "excluidos": n }))
}

/// ELEPONTO digitado: insere o que o modelo do desenho diz (a barra de cima o muda enquanto o
/// comando corre), um ponto por clique, até Enter. Sem modelo, pergunta o bloco.
#[derive(Default)]
struct PontoM {
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
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        match Self::modelo(s) {
            None => {
                let primeiro = s.doc().ok().and_then(|d| ele::biblioteca::blocos(d).into_iter().next()).map(|b| b.nome).unwrap_or_default();
                Prompt::new("Nome do bloco do ponto", Accept::TEXT).default(primeiro)
            }
            Some(_) if self.pedindo_rotacao => Prompt::new("Rotação (graus)", Accept::TEXT),
            Some(m) => Prompt::new(format!("Ponto de inserção de {} (Tab gira; Enter termina)", m.bloco), Accept::POINT).kw(&["Rotacao", "Girar"]),
        }
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
            atualizar_gerados(s)?;
            return Ok(Step::Done);
        }
        let Some(m) = Self::modelo(s) else {
            let nome = match &i {
                Input::Text(t) | Input::Keyword(t) if !t.trim().is_empty() => t.trim().to_string(),
                Input::Enter | Input::Text(_) => ele::biblioteca::blocos(s.doc()?).into_iter().next().map(|b| b.nome).unwrap_or_default(),
                _ => return Ok(Step::Continue),
            };
            match ele::biblioteca::blocos(s.doc()?).into_iter().find(|b| b.nome.eq_ignore_ascii_case(&nome)) {
                Some(b) => ele::biblioteca::definir_modelo(
                    s.state()?.uid,
                    Some(ele::biblioteca::Modelo { bloco: b.nome, rotacao_auto: true, ..Default::default() }),
                ),
                None => s.echo(format!("O desenho não tem o bloco de ponto \"{nome}\".")),
            }
            return Ok(Step::Continue);
        };
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
                match ele::biblioteca::inserir(s.doc_mut()?, &m.bloco, [p.x, p.y], r, &m.valores) {
                    Ok(_) => self.feitos += 1,
                    Err(e) => s.echo(e),
                }
                Ok(Step::Continue)
            }
            Input::Keyword(k) | Input::Text(k) => {
                match k.trim().to_ascii_lowercase().as_str() {
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
                atualizar_gerados(s)?;
                Ok(Step::Done)
            }
            _ => Ok(Step::Continue),
        }
    }
}
