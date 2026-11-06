# SPDX-License-Identifier: GPL-3.0-only
# Gera o pacote de distribuicao: compila em release e monta dist/FreeEletrica e o .zip.
# Uso: powershell -ExecutionPolicy Bypass -File scripts\package_release.ps1 [-Versao 0.3.0]
param([string]$Versao = "0.3.0")
$ErrorActionPreference = "Stop"

# O ambiente local fica em C:\rust; no CI o cargo ja esta no PATH.
# (o mesmo de scripts\ambiente.cmd: o Rust GNU não aceita o "ú" da pasta do usuário).
if (Test-Path "C:\rust\rustup") {
    $env:RUSTUP_HOME = "C:\rust\rustup"
    $env:CARGO_HOME = "C:\rust\cargo"
    $env:CARGO_TARGET_DIR = "C:\rust\target"
    $env:Path = "C:\rust\mingw\mingw64\bin;C:\rust\cargo\bin;$env:USERPROFILE\.cargo\bin;$env:Path"
}

Push-Location (Join-Path $PSScriptRoot "..\cadcraft")
try {
    cargo xtask dist --versao $Versao
    if ($LASTEXITCODE -ne 0) { throw "cargo xtask dist terminou com $LASTEXITCODE" }
} finally {
    Pop-Location
}
