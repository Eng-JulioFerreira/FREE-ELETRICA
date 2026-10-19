@echo off
setlocal
title FREE ELETRICA - Publicar no fork do CADCraft
rem Publica a pasta cadcraft/ no fork github.com/Eng-JulioFerreira/cadcraft-free-eletrica.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0publicar_fork.ps1" %*
exit /b %errorlevel%
