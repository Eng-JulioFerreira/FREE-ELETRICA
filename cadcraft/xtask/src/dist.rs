// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Julio Ferreira. Free Elétrica: ver LICENSE e AVISO_LEGAL.md na raiz do repositorio.
//! `cargo xtask dist`: compila em release e monta o pacote do Free Elétrica para quem só vai usar:
//! a pasta limpa `dist/FreeEletrica` com o executável, os templates, os manuais e as licenças, e o
//! `.zip` dela. Quem usa o pacote não precisa de Rust nem de script nenhum.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// O que vai no pacote além do executável: (origem relativa à raiz do projeto, destino na pasta).
const ARQUIVOS: &[(&str, &str)] = &[
    ("docs/LEIA-ME.md", "LEIA-ME.md"),
    ("docs/INSTALL.md", "INSTALL.md"),
    ("docs/GETTING_STARTED.md", "GETTING_STARTED.md"),
    ("templates/FREE_ELETRICA_BLOCOS.dxf", "templates/FREE_ELETRICA_BLOCOS.dxf"),
    ("LICENSE", "licencas/LICENSE-FREE-ELETRICA-GPL-3.0"),
    ("AVISO_LEGAL.md", "licencas/AVISO_LEGAL.md"),
    ("cadcraft/LICENSE-MIT", "licencas/CADCRAFT-LICENSE-MIT"),
    ("cadcraft/LICENSE-APACHE", "licencas/CADCRAFT-LICENSE-APACHE"),
    ("cadcraft/NOTICE", "licencas/CADCRAFT-NOTICE"),
    ("cadcraft/ATTRIBUTION.md", "licencas/CADCRAFT-ATTRIBUTION.md"),
];

/// A versão do Free Elétrica, que não é a do CADCraft.
const VERSAO: &str = "0.3.0";
/// A pasta do pacote.
const PASTA: &str = "FreeEletrica";
/// O executável no pacote.
const EXECUTAVEL: &str = "FreeEletrica.exe";

/// O nome do zip.
fn nome_do_zip(versao: &str) -> String {
    format!("FreeEletrica-v{versao}-win64.zip")
}

/// A versão pedida em `--versao`, ou a padrão. Só dígitos e pontos: ela vira nome de arquivo.
fn versao_pedida<'a>(rest: &[&'a str]) -> Result<&'a str, String> {
    let Some(i) = rest.iter().position(|a| *a == "--versao") else { return Ok(VERSAO) };
    let v = rest.get(i + 1).copied().ok_or("--versao pede um valor, como 0.2.0")?;
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(format!("versão \"{v}\" inválida: use algo como 0.2.0"));
    }
    Ok(v)
}

/// O executável de uma pasta de saída do cargo.
fn executavel(saida: &Path) -> PathBuf {
    saida.join("release").join(if cfg!(windows) { "cadcraft.exe" } else { "cadcraft" })
}

/// A pasta de saída do cargo: a de `CARGO_TARGET_DIR` ou `target` no workspace.
fn pasta_de_saida(workspace: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| workspace.join("target"), PathBuf::from)
}

/// Copia tudo para a pasta do pacote, refeita do zero. Falta de arquivo é erro: um pacote sem licença
/// ou sem manual não sai.
fn montar(raiz: &Path, exe: &Path, pasta: &Path) -> Result<(), String> {
    for (origem, _) in ARQUIVOS {
        if !raiz.join(origem).is_file() {
            return Err(format!("falta {origem} no projeto"));
        }
    }
    if !exe.is_file() {
        return Err(format!("não achei o executável em {}", exe.display()));
    }
    if pasta.exists() {
        fs::remove_dir_all(pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
    }
    fs::create_dir_all(pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
    fs::copy(exe, pasta.join(EXECUTAVEL)).map_err(|e| format!("{}: {e}", exe.display()))?;
    for (origem, destino) in ARQUIVOS {
        let alvo = pasta.join(destino);
        if let Some(p) = alvo.parent() {
            fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        fs::copy(raiz.join(origem), &alvo).map_err(|e| format!("{origem}: {e}"))?;
    }
    Ok(())
}

/// Compacta a pasta com o `tar` do sistema, que no Windows 10 e 11 grava zip.
fn compactar(pasta: &Path, zip: &Path) -> Result<(), String> {
    let (Some(mae), Some(nome)) = (pasta.parent(), pasta.file_name()) else { return Err("pasta do pacote sem nome".into()) };
    if zip.exists() {
        fs::remove_file(zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    }
    let status = Command::new("tar").arg("-a").arg("-c").arg("-f").arg(zip).arg("-C").arg(mae).arg(nome).status().map_err(|e| format!("tar: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("tar terminou com {status}")) }
}

/// `cargo xtask dist [--versao 0.2.0] [--sem-compilar]`.
pub fn run(workspace: &Path, rest: &[&str]) -> Result<(), String> {
    let versao = versao_pedida(rest)?;
    let raiz = workspace.parent().ok_or("o workspace do CADCraft fica dentro do projeto")?;
    if !rest.contains(&"--sem-compilar") {
        let mut c = crate::cargo();
        c.args(["build", "--release", "-p", "cadcraft"]);
        let status = c.status().map_err(|e| format!("cargo build: {e}"))?;
        if !status.success() {
            return Err(format!("cargo build terminou com {status}"));
        }
    }
    let exe = executavel(&pasta_de_saida(workspace));
    let saida = raiz.join("dist");
    let pasta = saida.join(PASTA);
    montar(raiz, &exe, &pasta)?;
    let zip = saida.join(nome_do_zip(versao));
    compactar(&pasta, &zip)?;
    println!("pacote: {}", zip.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta_de_teste(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("free-eletrica-dist-{nome}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn projeto_falso(raiz: &Path) -> PathBuf {
        for (origem, _) in ARQUIVOS {
            let f = raiz.join(origem);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, origem).unwrap();
        }
        let exe = raiz.join("saida").join("cadcraft.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "exe").unwrap();
        exe
    }

    #[test]
    fn os_arquivos_do_pacote_existem_no_projeto() {
        let raiz = crate::root();
        let raiz = raiz.parent().unwrap();
        for (origem, _) in ARQUIVOS {
            assert!(raiz.join(origem).is_file(), "falta {origem}");
        }
    }

    #[test]
    fn a_versao_pedida_vira_nome_de_arquivo_com_seguranca() {
        assert_eq!(versao_pedida(&[]), Ok(VERSAO));
        assert_eq!(versao_pedida(&["--versao", "1.2.3"]), Ok("1.2.3"));
        assert!(versao_pedida(&["--versao"]).is_err());
        assert!(versao_pedida(&["--versao", "../x"]).is_err());
        assert!(versao_pedida(&["--versao", ""]).is_err());
    }

    #[test]
    fn monta_a_pasta_limpa_e_recusa_o_que_falta() {
        let raiz = pasta_de_teste("monta");
        let exe = projeto_falso(&raiz);
        let pasta = raiz.join("dist").join(PASTA);
        // O que sobrou de um pacote anterior não vai junto.
        fs::create_dir_all(&pasta).unwrap();
        fs::write(pasta.join("velho.txt"), "x").unwrap();
        montar(&raiz, &exe, &pasta).unwrap();
        assert!(!pasta.join("velho.txt").exists());
        assert!(pasta.join("FreeEletrica.exe").is_file());
        assert!(pasta.join("LEIA-ME.md").is_file());
        assert!(pasta.join("templates/FREE_ELETRICA_BLOCOS.dxf").is_file());
        for (_, destino) in ARQUIVOS {
            assert!(pasta.join(destino).is_file(), "{destino}");
        }
        assert_eq!(nome_do_zip("0.2.0"), "FreeEletrica-v0.2.0-win64.zip");
        // Sem o executável ou sem um arquivo, não sai pacote.
        assert!(montar(&raiz, &raiz.join("nada.exe"), &pasta).is_err());
        fs::remove_file(raiz.join("docs/INSTALL.md")).unwrap();
        assert!(montar(&raiz, &exe, &pasta).unwrap_err().contains("INSTALL"));
        let _ = fs::remove_dir_all(&raiz);
    }
}
