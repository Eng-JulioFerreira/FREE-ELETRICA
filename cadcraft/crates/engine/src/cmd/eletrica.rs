//! CRAFT ELE: as ferramentas de projeto elétrico que trabalham no desenho.
//!
//! ELECIRCUITO (Criar circuito): selecione o quadro de destino, informe o número do circuito
//! (Enter aceita o próximo livre) e selecione os pontos, um a um, até Enter.
//! ELECOMANDO (Comandos e luminárias): selecione o interruptor e depois as luminárias que ele
//! comanda, até Enter. As luminárias recebem a letra do interruptor e o circuito dele.
//!
//! Os valores vão para os atributos do bloco quando existem e, senão, para o XDATA do CRAFT ELE
//! (`cadcraft_ele::editar`). Cada comando é um passo de desfazer.

use cadcraft_doc::Handle;
use cadcraft_ele::{self as ele, PontoEletrico, TipoPonto};
use serde_json::{Value, json};

use super::*;
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("elecircuito", "Electrical Circuit", run_circuito)
            .params("{quadro: hex, circuito?: string (default: the next free number), pontos: [hex]} → {circuito, pontos}")
            .interactive(|_| Ok(Box::new(CircuitoM::default()))),
        CommandSpec::new("elecomando", "Switches and Lamps", run_comando)
            .params("{interruptor: hex, luminarias: [hex], comando?: letter (default: the switch's, or the next free one)} → {comando, circuito, luminarias}")
            .interactive(|_| Ok(Box::new(ComandoM::default()))),
    ]
}

fn erro(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn analise(s: &Session) -> Result<ele::Analise> {
    Ok(ele::analisar(s.doc()?))
}

fn handle(cmd: &str, p: &Value, k: &str) -> Result<Handle> {
    p.get(k).and_then(Value::as_str).and_then(Handle::parse_hex).ok_or_else(|| bad(cmd, format!("`{k}` (handle) is required")))
}

fn handles(cmd: &str, p: &Value, k: &str) -> Result<Vec<Handle>> {
    let lista = p.get(k).and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("`{k}` ([handle]) is required")))?;
    lista.iter().map(|v| v.as_str().and_then(Handle::parse_hex).ok_or_else(|| bad(cmd, format!("`{k}` holds handles, got {v}")))).collect()
}

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
    let quadro = handle("elecircuito", p, "quadro")?;
    if quadro_valido(s, quadro)?.is_none() {
        return Err(bad("elecircuito", "`quadro` is not a distribution board block (TIPO_PONTO = QUADRO)"));
    }
    let circuito = match p.get("circuito").and_then(Value::as_str).map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => c.to_string(),
        None => ele::circuito::proximo_circuito(&analise(s)?.pontos, quadro),
    };
    let mut feitos = Vec::new();
    for h in handles("elecircuito", p, "pontos")? {
        if atribuir(s, quadro, &circuito, h)? {
            feitos.push(h.hex());
        }
    }
    Ok(json!({ "circuito": circuito, "pontos": feitos }))
}

#[derive(Default)]
struct CircuitoM {
    /// O quadro escolhido e a TAG dele.
    quadro: Option<(Handle, String)>,
    circuito: Option<String>,
    /// O próximo número livre, oferecido como padrão.
    sugestao: String,
    feitos: usize,
}

impl Interactive for CircuitoM {
    fn name(&self) -> &'static str {
        "ELECIRCUITO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match (&self.quadro, &self.circuito) {
            (None, _) => Prompt::new("Selecione o quadro de distribuição", Accept::SELECT),
            (Some(_), None) => Prompt::new("Número do circuito", Accept::TEXT).default(self.sugestao.clone()),
            (Some((_, tag)), Some(c)) => Prompt::new(format!("Selecione os pontos do circuito {c} ({tag})"), Accept::SELECT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (&self.quadro, &self.circuito, i) {
            (_, _, Input::Cancel) => Ok(Step::Done),
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
            (None, _, Input::Enter) => Ok(Step::Done),
            (Some(_), None, Input::Text(t) | Input::Keyword(t)) if !t.trim().is_empty() => {
                self.circuito = Some(t.trim().to_string());
                Ok(Step::Continue)
            }
            (Some(_), None, Input::Enter | Input::Text(_)) => {
                self.circuito = Some(self.sugestao.clone());
                Ok(Step::Continue)
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
            (Some(_), Some(c), Input::Enter) => {
                s.echo(format!("{} ponto(s) no circuito {c}.", self.feitos));
                Ok(Step::Done)
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
    circuito: Option<String>,
    quadro: Option<Handle>,
}

/// Lê o interruptor e garante a letra dele (a que tem, ou a próxima livre no circuito).
fn preparar_interruptor(s: &mut Session, h: Handle, letra: Option<&str>) -> Result<Option<Interruptor>> {
    let a = analise(s)?;
    let Some(p) = a.ponto(h).filter(|p| p.tipo == TipoPonto::Interruptor) else { return Ok(None) };
    let letra = match (letra, &p.comando) {
        (Some(l), _) => l.trim().to_string(),
        (None, Some(l)) => l.clone(),
        (None, None) => ele::circuito::proxima_letra(&a.pontos, p.quadro, p.circuito.as_deref()),
    };
    if p.comando.as_deref() != Some(letra.as_str()) {
        ele::editar::definir_comando(s.doc_mut()?, h, &letra).map_err(erro)?;
    }
    Ok(Some(Interruptor { h, rotulo: p.rotulo(), letra, circuito: p.circuito.clone(), quadro: p.quadro }))
}

/// Liga uma luminária ao interruptor: mesma letra e mesmo circuito. Se o interruptor ainda não
/// tem circuito, ele fica com o da luminária.
fn ligar_luminaria(s: &mut Session, sw: &mut Interruptor, h: Handle) -> Result<bool> {
    let a = analise(s)?;
    let Some(lum) = a.ponto(h).filter(|p| p.tipo == TipoPonto::Iluminacao).cloned() else {
        s.echo("O objeto não é uma luminária (TIPO_PONTO = ILUM).");
        return Ok(false);
    };
    ele::editar::definir_comando(s.doc_mut()?, h, &sw.letra).map_err(erro)?;
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
    let letra = p.get("comando").and_then(Value::as_str).filter(|l| !l.trim().is_empty());
    let mut sw =
        preparar_interruptor(s, h, letra)?.ok_or_else(|| bad("elecomando", "`interruptor` is not a switch block (TIPO_PONTO = INTERRUPTOR)"))?;
    let mut feitos = Vec::new();
    for l in handles("elecomando", p, "luminarias")? {
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
}

impl Interactive for ComandoM {
    fn name(&self) -> &'static str {
        "ELECOMANDO"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.set_selection(Vec::new());
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match &self.interruptor {
            None => Prompt::new("Selecione o interruptor", Accept::SELECT),
            Some(sw) => Prompt::new(format!("Selecione as luminárias do comando {}", sw.letra), Accept::SELECT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Cancel => Ok(Step::Done),
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
