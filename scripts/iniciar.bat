@echo off
setlocal
title FREE ELETRICA - CADCraft com a paleta Eletrica

rem Abre o CADCraft compilado com o FREE ELETRICA (o mais recente entre release e desenvolvimento).
set "CADCRAFT_RELEASE=C:\rust\target\release\cadcraft.exe"
set "CADCRAFT_DEBUG=C:\rust\target\debug\cadcraft.exe"

rem Porta de controle: scripts e testes automaticos leem e comandam o desenho aberto.
set "FREEELETRICA_PORT=47800"

rem Configuracao separada do CADCraft oficial: o layout salvo com a paleta Eletrica nao seria
rem entendido por ele.
set "CADCRAFT_CONFIG_DIR=%APPDATA%\CADCraft-FREEELETRICA"
rem O projeto se chamava CRAFT ELE: na primeira vez, copia a configuracao antiga (layout,
rem configuracoes do projeto eletrico). A pasta antiga fica como estava.
if not exist "%CADCRAFT_CONFIG_DIR%" if exist "%APPDATA%\CADCraft-CRAFTELE" (
    xcopy "%APPDATA%\CADCraft-CRAFTELE" "%CADCRAFT_CONFIG_DIR%" /E /I /Q /Y >nul
)

set "EXE="
for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "Get-Item '%CADCRAFT_RELEASE%','%CADCRAFT_DEBUG%' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName"`) do set "EXE=%%i"
if not defined EXE (
    echo [FREE ELETRICA] O CADCraft ainda nao foi compilado. Rode scripts\compilar.bat primeiro.
    pause
    exit /b 1
)

start "" "%EXE%" --control %FREEELETRICA_PORT% %*
exit /b 0
