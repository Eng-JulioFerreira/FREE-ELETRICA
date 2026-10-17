@echo off
setlocal
title CRAFT ELE - Portoes de qualidade
call "%~dp0ambiente.cmd"
cd /d "%CADCRAFT_DIR%"

rem Os portoes que precisam passar antes de todo commit (ver CLAUDE.md):
rem formatacao, clippy sem avisos, testes e a regra de camadas do CADCraft.
rem Uso: testar.bat        (crates do CRAFT ELE e as que ele toca)
rem      testar.bat tudo   (o workspace inteiro do CADCraft)

echo [1/4] Formatacao
cargo fmt --all -- --check || goto falhou

echo [2/4] Clippy (avisos sao erros)
if /i "%~1"=="tudo" (
    cargo clippy --workspace --all-targets -- -D warnings || goto falhou
) else (
    cargo clippy -p cadcraft-ele -p cadcraft-engine -p cadcraft-ui-egui -p cadcraft --all-targets -- -D warnings || goto falhou
)

echo [3/4] Testes
if /i "%~1"=="tudo" (
    cargo test --workspace || goto falhou
) else (
    cargo test -p cadcraft-ele -p cadcraft-engine -p cadcraft-ui-egui -p cadcraft-mcp -p cadcraft || goto falhou
)

echo [4/4] Camadas
cargo xtask layers || goto falhou

echo.
echo [CRAFT ELE] Todos os portoes passaram.
exit /b 0

:falhou
echo.
echo [CRAFT ELE] Um portao falhou. Nada de commit ate passar.
exit /b 1
