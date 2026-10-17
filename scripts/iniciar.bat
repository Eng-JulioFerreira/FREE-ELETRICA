@echo off
setlocal
title CRAFT ELE - CADCraft com a paleta Eletrica

rem Abre o CADCraft compilado com o CRAFT ELE (o mais recente entre release e desenvolvimento).
set "CADCRAFT_RELEASE=C:\rust\target\release\cadcraft.exe"
set "CADCRAFT_DEBUG=C:\rust\target\debug\cadcraft.exe"

rem Porta de controle: scripts e testes automaticos leem e comandam o desenho aberto.
set "CRAFTELE_PORT=47800"

rem Configuracao separada do CADCraft oficial: o layout salvo com a paleta Eletrica nao seria
rem entendido por ele.
set "CADCRAFT_CONFIG_DIR=%APPDATA%\CADCraft-CRAFTELE"

set "EXE="
for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "Get-Item '%CADCRAFT_RELEASE%','%CADCRAFT_DEBUG%' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName"`) do set "EXE=%%i"
if not defined EXE (
    echo [CRAFT ELE] O CADCraft ainda nao foi compilado. Rode scripts\compilar.bat primeiro.
    pause
    exit /b 1
)

start "" "%EXE%" --control %CRAFTELE_PORT% %*
exit /b 0
