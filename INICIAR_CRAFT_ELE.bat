@echo off
setlocal
title CRAFT ELE - CADCraft com o painel Eletrica

rem Abre o CADCraft compilado do fork (branch craft-ele), que ja traz o painel Eletrica.
rem Dentro dele: digite ELETRICA na linha de comando, ou Janela > Eletrica.
set "CADCRAFT_RELEASE=C:\rust\target\release\cadcraft.exe"
set "CADCRAFT_DEBUG=C:\rust\target\debug\cadcraft.exe"

rem A porta de controle deixa scripts e testes automaticos lerem o desenho aberto.
set "CRAFTELE_PORT=47800"

rem Configuracao separada do CADCraft oficial: o layout salvo com o painel Eletrica
rem nao seria entendido pelo CADCraft oficial.
set "CADCRAFT_CONFIG_DIR=%APPDATA%\CADCraft-CRAFTELE"

set "EXE="
if exist "%CADCRAFT_RELEASE%" set "EXE=%CADCRAFT_RELEASE%"
if not defined EXE if exist "%CADCRAFT_DEBUG%" set "EXE=%CADCRAFT_DEBUG%"
if not defined EXE (
    echo [CRAFT ELE] O CADCraft ainda nao foi compilado. Rode COMPILAR_CADCRAFT.bat primeiro.
    pause
    exit /b 1
)

start "" "%EXE%" --control %CRAFTELE_PORT% %*
exit /b 0
