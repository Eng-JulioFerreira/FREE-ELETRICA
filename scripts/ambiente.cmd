@echo off
rem Ambiente de compilacao do CRAFT ELE. Usado pelos outros scripts (call ambiente.cmd).
rem
rem O Rust GNU nao aceita o "u" acentuado da pasta do usuario, entao o toolchain, o MinGW e a
rem pasta de saida ficam em C:\rust (so caracteres simples).
set "RUSTUP_HOME=C:\rust\rustup"
set "CARGO_HOME=C:\rust\cargo"
set "CARGO_TARGET_DIR=C:\rust\target"
rem Sem informacao de depuracao: com ela os testes passam de 100 GB no Rust GNU.
set "CARGO_PROFILE_DEV_DEBUG=0"
set "CARGO_INCREMENTAL=0"
set "PATH=C:\rust\mingw\mingw64\bin;%USERPROFILE%\.cargo\bin;%PATH%"
set "CRAFTELE_RAIZ=%~dp0.."
set "CADCRAFT_DIR=%~dp0..\cadcraft"
