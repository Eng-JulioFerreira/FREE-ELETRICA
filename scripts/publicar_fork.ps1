# SPDX-License-Identifier: GPL-3.0-only
# Publica a pasta cadcraft/ deste repositorio no fork do CADCraft
# (github.com/Eng-JulioFerreira/cadcraft-free-eletrica), como um commit novo no main do fork.
#
# O commit tem a arvore exata de cadcraft/ e dois pais: o main atual do fork e o commit do
# CADCraft oficial em que o subtree esta (o "git-subtree-split" do ultimo squash). Assim o fork
# continua ligado ao original e o push nao precisa de --force.
#
# Uso: scripts\publicar_fork.bat   (depois dos portoes passarem e do commit no repositorio principal)
param([string]$Fork = "https://github.com/Eng-JulioFerreira/cadcraft-free-eletrica.git")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (git status --porcelain -- cadcraft) { Write-Output "[FREE ELETRICA] Ha mudancas sem commit em cadcraft/: faca o commit antes."; exit 1 }

# O commit oficial em que o subtree esta.
$split = (git log --grep "git-subtree-dir: cadcraft" -1 --format=%B) | Where-Object { $_ -like "git-subtree-split:*" } | Select-Object -First 1
$oficial = ($split -replace "^git-subtree-split:\s*", "").Trim()
if (-not $oficial) { Write-Output "[FREE ELETRICA] Nao achei o commit oficial do subtree."; exit 1 }
git fetch --quiet https://github.com/storytold/cadcraft.git $oficial 2>$null
if ($LASTEXITCODE -ne 0) { git fetch --quiet https://github.com/storytold/cadcraft.git main }

# O main atual do fork.
git fetch --quiet $Fork main
$anterior = (git rev-parse FETCH_HEAD).Trim()
$arvore = (git rev-parse HEAD:cadcraft).Trim()
if ((git rev-parse "$anterior^{tree}").Trim() -eq $arvore) { Write-Output "[FREE ELETRICA] O fork ja tem esta versao: nada a publicar."; exit 0 }

$ultimo = (git log -1 --format=%s).Trim()
$msg = "Free Elétrica: sync with the main repository`n`nMain repository: $ultimo`nCADCraft base: $oficial`n"
$arquivo = Join-Path $env:TEMP "fe_fork_msg.txt"
[IO.File]::WriteAllText($arquivo, $msg, (New-Object System.Text.UTF8Encoding($false)))
$pais = @("-p", $anterior)
git merge-base --is-ancestor $oficial $anterior
if ($LASTEXITCODE -ne 0) { $pais += @("-p", $oficial) }
$novo = (git commit-tree $arvore @pais -F $arquivo).Trim()
Remove-Item $arquivo -ErrorAction SilentlyContinue
git push $Fork "${novo}:refs/heads/main"
if ($LASTEXITCODE -ne 0) { Write-Output "[FREE ELETRICA] O push para o fork falhou."; exit 1 }
Write-Output "[FREE ELETRICA] Fork atualizado: $novo"
