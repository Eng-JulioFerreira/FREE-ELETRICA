@echo off
setlocal
title CRAFT ELE - Compilar CADCraft com o painel Eletrica

rem O Rust GNU nao aceita o "u" acentuado da pasta do usuario, entao o toolchain,
rem o MinGW e a pasta de saida ficam em C:\rust (so caracteres simples).
set "RUSTUP_HOME=C:\rust\rustup"
set "CARGO_HOME=C:\rust\cargo"
set "CARGO_TARGET_DIR=C:\rust\target"
set "PATH=C:\rust\mingw\mingw64\bin;%USERPROFILE%\.cargo\bin;%PATH%"

cd /d "%~dp0cadcraft"

rem Uso: COMPILAR_CADCRAFT.bat          (versao de desenvolvimento, mais rapida de compilar)
rem      COMPILAR_CADCRAFT.bat release  (versao otimizada, mais lenta de compilar)
if /i "%~1"=="release" (
    cargo build -p cadcraft --release
) else (
    cargo build -p cadcraft
)
if errorlevel 1 (
    echo.
    echo [CRAFT ELE] A compilacao falhou. Veja as mensagens acima.
    pause
    exit /b 1
)
echo.
echo [CRAFT ELE] Pronto. Abra com INICIAR_CRAFT_ELE.bat
exit /b 0
