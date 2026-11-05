// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! Escritas do Free Elétrica no desenho. Um valor vai para o atributo do bloco quando ele existe
//! (o projetista vê e edita) e, senão, para o XDATA do Free Elétrica (viaja com o bloco, invisível).

use cadcraft_doc::{Drawing, EntityKind, Handle};

use crate::nbr5410::{Esquema, Isolacao, TipoCabo};
use crate::ponto::{CHAVE_ALTURA, TAGS_ALTURA, TAGS_CIRCUITO, TAGS_COMANDO, TAGS_QUADRO, ler_ponto};
use crate::quadro::{
    CHAVE_ALIMENTADO_POR, CHAVE_CABO, CHAVE_CABO_NOME, CHAVE_CABO_QUADRO, CHAVE_CURVA, CHAVE_DESCRICAO, CHAVE_DIM, CHAVE_DISJUNTOR, CHAVE_DR,
    CHAVE_ESQUEMA, CHAVE_FASE, CHAVE_ISOLACAO, CHAVE_ISOLACAO_CIRCUITO, CHAVE_SECAO, CHAVE_SISTEMA_QUADRO, CHAVE_TENSAO, CHAVE_TENSAO_NOMINAL,
    CONCESSIONARIA, NOMES_FASES, fases_de_texto,
};
use crate::xdata;

/// Grava (ou apaga) uma chave do XDATA do Free Elétrica num bloco.
fn gravar_xdata(doc: &mut Drawing, h: Handle, chave: &str, valor: Option<&str>) -> Result<(), String> {
    let mut e = (**doc.model.get(h).ok_or("objeto não encontrado")?).clone();
    if !matches!(e.kind, EntityKind::Insert(_)) {
        return Err("o objeto não é um bloco".into());
    }
    xdata::gravar(&mut e.common.xdata, chave, valor);
    doc.model.replace(e);
    Ok(())
}

/// Os pontos do circuito `circuito` do quadro `quadro`.
fn pontos_do_circuito(doc: &Drawing, quadro: Handle, circuito: &str) -> Vec<Handle> {
    crate::analisar(doc)
        .pontos
        .iter()
        .filter(|p| p.quadro == Some(quadro) && p.circuito.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(circuito.trim())))
        .map(|p| p.handle)
        .collect()
}

/// Apaga do quadro as escolhas e a descrição de um circuito.
fn esquecer_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str) -> Result<(), String> {
    let id = circuito.trim().to_uppercase();
    for prefixo in [
        CHAVE_SECAO,
        CHAVE_DISJUNTOR,
        CHAVE_DESCRICAO,
        CHAVE_ISOLACAO_CIRCUITO,
        CHAVE_CABO,
        CHAVE_CABO_NOME,
        CHAVE_CURVA,
        CHAVE_ESQUEMA,
        CHAVE_TENSAO,
        CHAVE_FASE,
        CHAVE_DR,
        CHAVE_DIM,
    ] {
        gravar_xdata(doc, quadro, &format!("{prefixo}{id}"), None)?;
    }
    Ok(())
}

fn chave_do_circuito(prefixo: &str, circuito: &str) -> Result<String, String> {
    let id = circuito.trim().to_uppercase();
    if id.is_empty() {
        return Err("informe o circuito".into());
    }
    Ok(format!("{prefixo}{id}"))
}

/// Isolação dos condutores de um circuito; `None` volta à do quadro.
pub fn definir_isolacao_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, isolacao: Option<Isolacao>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CABO, circuito)?, None)?;
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ISOLACAO_CIRCUITO, circuito)?, isolacao.map(Isolacao::codigo))
}

/// Tipo de cabo de um circuito; `None` volta ao padrão.
pub fn definir_cabo_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, cabo: Option<TipoCabo>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ISOLACAO_CIRCUITO, circuito)?, None)?;
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CABO, circuito)?, cabo.map(TipoCabo::codigo))
}

/// Grupo de DR de um circuito; `None` tira.
pub fn definir_grupo_dr(doc: &mut Drawing, quadro: Handle, circuito: &str, grupo: Option<&str>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_DR, circuito)?, grupo.map(str::trim).filter(|g| !g.is_empty()))
}

/// Processa o cálculo: dimensiona os quadros (`None`: todos), grava o resultado de cada circuito no
/// quadro, para a tabela mostrar, e atualiza as tags e tabelas que já existem. Só aqui o desenho
/// recebe o dimensionamento.
pub fn processar_calculo(doc: &mut Drawing, setup: &crate::setup::Setup, quadro: Option<Handle>) -> Result<Vec<crate::quadro::Processado>, String> {
    let analise = crate::analisar(doc);
    let alvos: Vec<Handle> = match quadro {
        Some(q) => vec![q],
        None => analise.quadros().iter().map(|q| q.handle).collect(),
    };
    let mut feitos = Vec::new();
    for q in alvos {
        let r = crate::quadro::rotear(doc, &analise, setup, q)?;
        let antigas: Vec<String> = ler_ponto(doc, q)
            .map(|p| p.extras.iter().filter(|(k, _)| k.to_uppercase().starts_with(CHAVE_DIM)).map(|(k, _)| k.clone()).collect())
            .unwrap_or_default();
        for k in &antigas {
            gravar_xdata(doc, q, k, None)?;
        }
        for l in &r.linhas {
            let texto = crate::quadro::texto_do_calculo(&l.calculo_atual());
            gravar_xdata(doc, q, &chave_do_circuito(CHAVE_DIM, &l.circuito.id)?, Some(&texto))?;
        }
        let mut avisos = r.avisos.clone();
        avisos.extend(r.linhas.iter().flat_map(|l| l.escolha.avisos.iter().map(|a| format!("Circuito {}: {a}", l.circuito.id))));
        feitos.push(crate::quadro::Processado { tag: r.tag.clone(), circuitos: r.linhas.len(), avisos });
    }
    crate::fiacao::regenerar_se_houver(doc, setup)?;
    Ok(feitos)
}

/// Um atributo de uma inserção, como o inspetor mostra.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtributoInstancia {
    pub tag: String,
    pub valor: String,
    pub visivel: bool,
    pub x: f64,
    pub y: f64,
    /// Fica na definição do bloco: não muda por inserção.
    pub constante: bool,
}

/// Os atributos de um bloco inserido. `None`: o objeto não é um bloco.
pub fn atributos_de(doc: &Drawing, h: Handle) -> Option<Vec<AtributoInstancia>> {
    let e = doc.model.get(h)?;
    let EntityKind::Insert(ins) = &e.kind else { return None };
    let mut v: Vec<AtributoInstancia> = ins
        .attribs
        .iter()
        .map(|a| AtributoInstancia {
            tag: a.tag.trim().to_string(),
            valor: a.text.value.clone(),
            visivel: !a.invisible,
            x: a.text.insert.x,
            y: a.text.insert.y,
            constante: false,
        })
        .collect();
    if let Some(def) = doc.blocks.get(&ins.block) {
        for ent in def.entities.iter() {
            if let EntityKind::AttDef(a) = &ent.kind
                && a.constant
            {
                v.push(AtributoInstancia {
                    tag: a.tag.trim().to_string(),
                    valor: a.text.value.clone(),
                    visivel: !a.invisible,
                    x: 0.0,
                    y: 0.0,
                    constante: true,
                });
            }
        }
    }
    Some(v)
}

/// Muda um atributo de um bloco inserido: valor, visibilidade e posição. ALTURA segue a regra da
/// legenda (`definir_altura`).
pub fn editar_atributo(
    doc: &mut Drawing,
    h: Handle,
    tag: &str,
    valor: Option<&str>,
    visivel: Option<bool>,
    posicao: Option<[f64; 2]>,
    setup: &crate::setup::Setup,
) -> Result<(), String> {
    if posicao.is_some_and(|p| !p.iter().all(|v| v.is_finite())) {
        return Err("posição inválida".into());
    }
    let lista = atributos_de(doc, h).ok_or("o objeto não é um bloco")?;
    let atual = lista.iter().find(|a| a.tag.eq_ignore_ascii_case(tag.trim())).ok_or_else(|| format!("o bloco não tem o atributo {}", tag.trim()))?;
    if atual.constante {
        return Err(format!("{} é constante: fica na definição do bloco", atual.tag));
    }
    if let Some(v) = valor
        && TAGS_ALTURA.iter().any(|t| tag.trim().eq_ignore_ascii_case(t))
        && ler_ponto(doc, h).is_some()
    {
        let altura = if v.trim().is_empty() { None } else { Some(crate::ponto::altura_m(v).ok_or_else(|| format!("altura \"{v}\" inválida"))?) };
        definir_altura(doc, h, altura, setup)?;
    } else if let Some(v) = valor {
        definir(doc, h, &[tag.trim()], Some(v))?;
    }
    let mut e = (**doc.model.get(h).ok_or("objeto não encontrado")?).clone();
    if let EntityKind::Insert(ins) = &mut e.kind
        && let Some(a) = ins.attribs.iter_mut().find(|a| a.tag.trim().eq_ignore_ascii_case(tag.trim()))
    {
        if let Some(on) = visivel {
            a.invisible = !on;
        }
        if let Some(p) = posicao {
            let (dx, dy) = (p[0] - a.text.insert.x, p[1] - a.text.insert.y);
            a.text.insert.x = p[0];
            a.text.insert.y = p[1];
            if let Some(alinhado) = &mut a.text.align_pt {
                alinhado.x += dx;
                alinhado.y += dy;
            }
        }
    }
    doc.model.replace(e);
    Ok(())
}

/// Destrava as fases de todos os circuitos do quadro: o balanceamento automático volta a valer para
/// todos. Devolve quantos estavam travados.
pub fn liberar_fases(doc: &mut Drawing, quadro: Handle) -> Result<usize, String> {
    let p = ler_ponto(doc, quadro).ok_or("quadro não encontrado")?;
    let chaves: Vec<String> = p.extras.iter().filter(|(k, _)| k.to_uppercase().starts_with(CHAVE_FASE)).map(|(k, _)| k.clone()).collect();
    for k in &chaves {
        gravar_xdata(doc, quadro, k, None)?;
    }
    Ok(chaves.len())
}

/// Isolação dos condutores do quadro; `None` volta à das configurações.
pub fn definir_isolacao(doc: &mut Drawing, quadro: Handle, isolacao: Option<Isolacao>) -> Result<(), String> {
    gravar_xdata(doc, quadro, CHAVE_ISOLACAO, isolacao.map(Isolacao::codigo))
}

/// A curva do disjuntor de um circuito; `None` volta à sugerida.
pub fn definir_curva(doc: &mut Drawing, quadro: Handle, circuito: &str, curva: Option<crate::nbr5410::Curva>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CURVA, circuito)?, curva.map(crate::nbr5410::Curva::codigo))
}

/// O nome do cabo de um circuito no catálogo; `None` volta ao nome da classe.
pub fn definir_nome_cabo(doc: &mut Drawing, quadro: Handle, circuito: &str, nome: Option<&str>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_CABO_NOME, circuito)?, nome.map(str::trim).filter(|n| !n.is_empty()))
}

/// O sistema do quadro; o número de fases vai junto para o bloco. `None` volta ao que o bloco diz.
pub fn definir_sistema_quadro(doc: &mut Drawing, quadro: Handle, sistema: Option<crate::quadro::SistemaQuadro>) -> Result<(), String> {
    if let Some(s) = sistema {
        definir(doc, quadro, crate::ponto::TAGS_FASES, Some(&s.fases().to_string()))?;
    }
    gravar_xdata(doc, quadro, CHAVE_SISTEMA_QUADRO, sistema.map(crate::quadro::SistemaQuadro::codigo))
}

/// A tensão nominal do quadro; a tensão vai junto para o bloco. `None` volta ao que o bloco diz.
pub fn definir_tensao_nominal(doc: &mut Drawing, quadro: Handle, tensao: Option<crate::quadro::TensaoNominal>) -> Result<(), String> {
    if let Some(t) = tensao {
        definir(doc, quadro, crate::ponto::TAGS_TENSAO, Some(&format!("{}V", t.codigo())))?;
    }
    gravar_xdata(doc, quadro, CHAVE_TENSAO_NOMINAL, tensao.map(crate::quadro::TensaoNominal::codigo))
}

/// O tipo de cabo padrão do quadro; `None` volta ao padrão do programa.
pub fn definir_cabo_quadro(doc: &mut Drawing, quadro: Handle, cabo: Option<TipoCabo>) -> Result<(), String> {
    gravar_xdata(doc, quadro, CHAVE_ISOLACAO, None)?;
    gravar_xdata(doc, quadro, CHAVE_CABO_QUADRO, cabo.map(TipoCabo::codigo))
}

/// A montagem do quadro. `None` tira.
pub fn definir_montagem(doc: &mut Drawing, quadro: Handle, m: Option<crate::quadro::Montagem>) -> Result<(), String> {
    gravar_xdata(doc, quadro, crate::quadro::CHAVE_MONTAGEM, m.map(crate::quadro::Montagem::codigo))
}

/// As observações do quadro. Vazio tira.
pub fn definir_observacoes(doc: &mut Drawing, quadro: Handle, texto: Option<&str>) -> Result<(), String> {
    gravar_xdata(doc, quadro, crate::quadro::CHAVE_OBSERVACOES, texto.map(str::trim).filter(|t| !t.is_empty()))
}

/// O número de módulos do quadro. `None` volta à estimativa.
pub fn definir_modulos(doc: &mut Drawing, quadro: Handle, n: Option<u32>) -> Result<(), String> {
    if n.is_some_and(|n| n > crate::quadro::MAXIMO_MODULOS) {
        return Err(format!("use até {} módulos", crate::quadro::MAXIMO_MODULOS));
    }
    gravar_xdata(doc, quadro, crate::quadro::CHAVE_MODULOS, n.map(|n| n.to_string()).as_deref())
}

/// Quantos circuitos reserva o quadro tem. `None` volta ao mínimo da NBR 5410.
pub fn definir_reservas(doc: &mut Drawing, quadro: Handle, n: Option<u32>) -> Result<(), String> {
    if n.is_some_and(|n| n > crate::quadro::MAXIMO_RESERVAS) {
        return Err(format!("use até {} reservas", crate::quadro::MAXIMO_RESERVAS));
    }
    gravar_xdata(doc, quadro, crate::quadro::CHAVE_RESERVAS, n.map(|n| n.to_string()).as_deref())
}

/// Quem alimenta o quadro: a concessionária ou outro quadro do desenho, sem formar ciclo.
pub fn definir_alimentacao(doc: &mut Drawing, quadro: Handle, alimentador: Option<&str>) -> Result<(), String> {
    let valor = match alimentador.map(str::trim).filter(|t| !t.is_empty()) {
        None => None,
        Some(t) if t.eq_ignore_ascii_case(CONCESSIONARIA) => Some(CONCESSIONARIA.to_string()),
        Some(t) => {
            let analise = crate::analisar(doc);
            let achado = analise
                .quadros()
                .into_iter()
                .find(|q| Handle::parse_hex(t) == Some(q.handle) || q.tag.as_deref().is_some_and(|tag| tag.eq_ignore_ascii_case(t)))
                .ok_or_else(|| format!("\"{t}\" não é a concessionária nem um quadro do desenho"))?;
            // Subindo a partir do novo alimentador, não pode chegar neste quadro.
            let mut atual = Some(achado.handle);
            let mut passos = 0;
            while let Some(h) = atual {
                if h == quadro {
                    return Err("um quadro não pode alimentar a si mesmo, nem por outro quadro".into());
                }
                passos += 1;
                if passos > analise.pontos.len() {
                    break;
                }
                atual = analise.ponto(h).and_then(crate::quadro::quadro_que_alimenta);
            }
            Some(achado.handle.hex())
        }
    };
    gravar_xdata(doc, quadro, CHAVE_ALIMENTADO_POR, valor.as_deref())
}

/// Esquema de condutores de um circuito; `None` volta ao que os pontos dizem.
pub fn definir_esquema(doc: &mut Drawing, quadro: Handle, circuito: &str, esquema: Option<Esquema>) -> Result<(), String> {
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_ESQUEMA, circuito)?, esquema.map(Esquema::codigo))
}

/// Tensão de um circuito (V); `None` volta à dos pontos.
pub fn definir_tensao_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str, tensao: Option<f64>) -> Result<(), String> {
    if let Some(t) = tensao
        && !(t.is_finite() && t > 0.0)
    {
        return Err(format!("tensão {t} inválida"));
    }
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_TENSAO, circuito)?, tensao.map(|t| t.to_string()).as_deref())
}

/// Trava as fases de um circuito ("R", "RS"...); `None` volta ao balanceamento automático.
pub fn travar_fases(doc: &mut Drawing, quadro: Handle, circuito: &str, fases: Option<&str>) -> Result<(), String> {
    let normal = match fases {
        Some(t) => {
            let v = fases_de_texto(t).ok_or_else(|| format!("fases \"{t}\" inválidas: use R, S, T"))?;
            Some(v.iter().filter_map(|f| NOMES_FASES.get(*f).copied()).collect::<String>())
        }
        None => None,
    };
    gravar_xdata(doc, quadro, &chave_do_circuito(CHAVE_FASE, circuito)?, normal.as_deref())
}

/// Tira pontos do circuito deles (ficam sem circuito e sem quadro). Devolve quantos saíram.
pub fn remover_do_circuito(doc: &mut Drawing, pontos: &[Handle]) -> Result<usize, String> {
    let mut n = 0;
    for h in pontos {
        let p = ler_ponto(doc, *h).ok_or_else(|| format!("o objeto {} não é um ponto elétrico", h.hex()))?;
        if p.circuito.is_some() {
            definir(doc, *h, TAGS_CIRCUITO, None)?;
            definir(doc, *h, TAGS_QUADRO, None)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Grava no XDATA do interruptor as luminárias da letra: `target_handles.<letra>`.
pub fn vincular_luminaria(doc: &mut Drawing, interruptor: Handle, letra: &str, luminaria: Handle) -> Result<(), String> {
    let chave = format!("target_handles.{}", letra.trim().to_lowercase());
    let p = ler_ponto(doc, interruptor).ok_or("o interruptor não é um ponto elétrico")?;
    let atual = p.extras.iter().find(|(k, _)| k.eq_ignore_ascii_case(&chave)).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut lista: Vec<String> = atual.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
    if !lista.iter().any(|h| h.eq_ignore_ascii_case(&luminaria.hex())) {
        lista.push(luminaria.hex());
    }
    gravar_xdata(doc, interruptor, &chave, Some(&lista.join(",")))
}

/// Descrição do circuito no quadro de cargas (`None` apaga). Um circuito descrito existe no
/// quadro mesmo sem pontos.
pub fn definir_descricao(doc: &mut Drawing, quadro: Handle, circuito: &str, descricao: Option<&str>) -> Result<(), String> {
    let id = circuito.trim().to_uppercase();
    if id.is_empty() {
        return Err("informe o circuito".into());
    }
    gravar_xdata(doc, quadro, &format!("{CHAVE_DESCRICAO}{id}"), descricao)
}

/// Exclui o circuito do quadro: os pontos dele ficam sem circuito e sem quadro, e as escolhas e a
/// descrição dele somem. Devolve quantos pontos foram liberados.
pub fn excluir_circuito(doc: &mut Drawing, quadro: Handle, circuito: &str) -> Result<usize, String> {
    let pontos = pontos_do_circuito(doc, quadro, circuito);
    for h in &pontos {
        definir(doc, *h, TAGS_CIRCUITO, None)?;
        definir(doc, *h, TAGS_QUADRO, None)?;
    }
    esquecer_circuito(doc, quadro, circuito)?;
    Ok(pontos.len())
}

/// Junta o circuito `de` no circuito `para` do mesmo quadro: os pontos de `de` passam para `para`
/// e as escolhas e a descrição de `de` somem. Devolve quantos pontos mudaram.
pub fn juntar_circuitos(doc: &mut Drawing, quadro: Handle, de: &str, para: &str) -> Result<usize, String> {
    if de.trim().eq_ignore_ascii_case(para.trim()) {
        return Err("escolha dois circuitos diferentes".into());
    }
    if para.trim().is_empty() {
        return Err("informe o circuito de destino".into());
    }
    let pontos = pontos_do_circuito(doc, quadro, de);
    for h in &pontos {
        atribuir_circuito(doc, *h, quadro, para.trim())?;
    }
    esquecer_circuito(doc, quadro, de)?;
    Ok(pontos.len())
}

/// Grava (`Some`) ou limpa (`None`) o valor de `tags` no bloco `h`. A primeira tag é a chave
/// do XDATA.
pub fn definir(doc: &mut Drawing, h: Handle, tags: &[&str], valor: Option<&str>) -> Result<(), String> {
    let mut e = (**doc.model.get(h).ok_or("objeto não encontrado")?).clone();
    let chave = tags.first().copied().ok_or("sem tag")?;
    let EntityKind::Insert(ins) = &mut e.kind else { return Err("o objeto não é um bloco".into()) };
    match ins.attribs.iter_mut().find(|a| tags.iter().any(|t| a.tag.trim().eq_ignore_ascii_case(t))) {
        Some(a) => {
            a.text.value = valor.unwrap_or("").to_string();
            // O atributo manda: um valor antigo no XDATA não pode ficar por trás dele.
            xdata::gravar(&mut e.common.xdata, chave, None);
        }
        None => xdata::gravar(&mut e.common.xdata, chave, valor),
    }
    doc.model.replace(e);
    Ok(())
}

/// O texto que vai no atributo ALTURA de um ponto fora da legenda: `h=1,45m`.
pub fn texto_de_altura(altura: f64) -> String {
    format!("h={altura:.2}m").replace('.', ",")
}

/// A altura de uma inserção. Na legenda do projeto (ou com `None`), o atributo visível fica vazio
/// e a planta não polui; fora dela, a altura exata vai para o XDATA, que o cálculo usa, e o atributo
/// mostra `h=1,45m`.
pub fn definir_altura(doc: &mut Drawing, ponto: Handle, altura: Option<f64>, setup: &crate::setup::Setup) -> Result<(), String> {
    if let Some(a) = altura
        && !(a.is_finite() && (0.0..=50.0).contains(&a))
    {
        return Err(format!("altura {a} inválida: use de 0 a 50 m"));
    }
    let p = ler_ponto(doc, ponto).ok_or("o objeto não é um ponto elétrico")?;
    let legenda = p.altura_legenda(setup);
    let fora = altura.filter(|a| (a - legenda).abs() >= 0.005);
    let texto = fora.map(texto_de_altura);
    definir(doc, ponto, TAGS_ALTURA, if p.tem_atributo(TAGS_ALTURA) { Some(texto.as_deref().unwrap_or("")) } else { None })?;
    gravar_xdata(doc, ponto, CHAVE_ALTURA, fora.map(|a| a.to_string()).as_deref())
}

/// Liga o ponto ao quadro. No XDATA vai o handle do quadro; num atributo QUADRO, a TAG dele.
pub fn ligar_ao_quadro(doc: &mut Drawing, ponto: Handle, quadro: Handle) -> Result<(), String> {
    let p = ler_ponto(doc, ponto).ok_or("o objeto não é um ponto elétrico")?;
    let tag_quadro = ler_ponto(doc, quadro).and_then(|q| q.tag);
    let valor = match (p.tem_atributo(TAGS_QUADRO), tag_quadro) {
        (true, Some(tag)) => tag,
        _ => quadro.hex(),
    };
    definir(doc, ponto, TAGS_QUADRO, Some(&valor))
}

/// Põe o ponto no circuito `circuito` do quadro `quadro`.
pub fn atribuir_circuito(doc: &mut Drawing, ponto: Handle, quadro: Handle, circuito: &str) -> Result<(), String> {
    definir(doc, ponto, TAGS_CIRCUITO, Some(circuito))?;
    ligar_ao_quadro(doc, ponto, quadro)
}

/// Grava a letra de comando do ponto.
pub fn definir_comando(doc: &mut Drawing, ponto: Handle, letra: &str) -> Result<(), String> {
    definir(doc, ponto, TAGS_COMANDO, Some(letra))
}

/// Grava (ou limpa, com `None`) a seção e o disjuntor escolhidos à mão para um circuito, no
/// XDATA do quadro.
pub fn definir_sobrescrita(doc: &mut Drawing, quadro: Handle, circuito: &str, secao: Option<f64>, disjuntor: Option<f64>) -> Result<(), String> {
    let mut e = (**doc.model.get(quadro).ok_or("quadro não encontrado")?).clone();
    if !matches!(e.kind, EntityKind::Insert(_)) {
        return Err("o quadro não é um bloco".into());
    }
    let id = circuito.trim().to_uppercase();
    xdata::gravar(&mut e.common.xdata, &format!("{CHAVE_SECAO}{id}"), secao.map(|s| s.to_string()).as_deref());
    xdata::gravar(&mut e.common.xdata, &format!("{CHAVE_DISJUNTOR}{id}"), disjuntor.map(|d| d.to_string()).as_deref());
    doc.model.replace(e);
    Ok(())
}
