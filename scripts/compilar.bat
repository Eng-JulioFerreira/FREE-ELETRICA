@echo off
setlocal
title CRAFT ELE - Compilar
call "%~dp0ambiente.cmd"
cd /d "%CADCRAFT_DIR%"

rem Uso: compilar.bat          (desenvolvimento, mais rapida de compilar)
rem      compilar.bat release  (otimizada)
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
echo [CRAFT ELE] Pronto. Abra com scripts\iniciar.bat
exit /b 0
