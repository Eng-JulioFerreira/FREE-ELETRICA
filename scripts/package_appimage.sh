#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Gera o pacote do Free Elétrica para Linux: dist/FreeEletrica-v<versão>-x86_64.AppImage.
#
# Uso: scripts/package_appimage.sh [--versao 0.2.0] [--sem-compilar]
#
# Roda num Linux x86_64 (ou no WSL) com o Rust (rustup) e as bibliotecas de desenvolvimento da janela:
#   sudo apt install build-essential pkg-config curl file libxkbcommon-dev libwayland-dev libx11-dev \
#     libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
# Compile num Linux antigo (no CI, Ubuntu 22.04) para o AppImage rodar nos mais novos. O appimagetool é
# baixado na primeira vez. O AppImage leva o executável, os manuais, o template de blocos e as licenças,
# como o .zip do Windows.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CAD="$RAIZ/cadcraft"
VERSAO="$(sed -n 's/^const VERSAO: &str = "\(.*\)";/\1/p' "$CAD/xtask/src/dist.rs")"
COMPILAR=1
while [ $# -gt 0 ]; do
  case "$1" in
    --versao) VERSAO="${2:-}"; shift 2 ;;
    --sem-compilar) COMPILAR=0; shift ;;
    -h | --help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done
# A versão vira nome de arquivo: só dígitos e pontos.
case "$VERSAO" in
  '' | *[!0-9.]*) echo "versão \"$VERSAO\" inválida: use algo como 0.2.0" >&2; exit 2 ;;
esac
ARCH="$(uname -m)"
[ "$ARCH" = x86_64 ] || { echo "o pacote é para x86_64; esta máquina é $ARCH" >&2; exit 2; }

# A saída do cargo fica no disco do Linux: na pasta do Windows (/mnt/c) a compilação fica muito lenta.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/free-eletrica/target}"
DIST="$RAIZ/dist"
TRABALHO="$CARGO_TARGET_DIR/appimage"
APPDIR="$TRABALHO/FreeEletrica.AppDir"
SAIDA="$DIST/FreeEletrica-v$VERSAO-x86_64.AppImage"

echo "==> Free Elétrica $VERSAO para Linux $ARCH"
if [ "$COMPILAR" = 1 ]; then
  (cd "$CAD" && cargo build --release --locked -p cadcraft)
fi
EXE="$CARGO_TARGET_DIR/release/cadcraft"
[ -x "$EXE" ] || { echo "não achei o executável em $EXE" >&2; exit 1; }

# O que vai no pacote além do executável: os mesmos arquivos do .zip do Windows (cadcraft/xtask/src/dist.rs).
ARQUIVOS=(
  "docs/LEIA-ME.md:LEIA-ME.md"
  "docs/INSTALL.md:INSTALL.md"
  "docs/GETTING_STARTED.md:GETTING_STARTED.md"
  "templates/FREE_ELETRICA_BLOCOS.dxf:templates/FREE_ELETRICA_BLOCOS.dxf"
  "LICENSE:licencas/LICENSE-FREE-ELETRICA-GPL-3.0"
  "AVISO_LEGAL.md:licencas/AVISO_LEGAL.md"
  "cadcraft/LICENSE-MIT:licencas/CADCRAFT-LICENSE-MIT"
  "cadcraft/LICENSE-APACHE:licencas/CADCRAFT-LICENSE-APACHE"
  "cadcraft/NOTICE:licencas/CADCRAFT-NOTICE"
  "cadcraft/ATTRIBUTION.md:licencas/CADCRAFT-ATTRIBUTION.md"
)
for par in "${ARQUIVOS[@]}"; do
  [ -f "$RAIZ/${par%%:*}" ] || { echo "falta ${par%%:*} no projeto: um pacote sem manual ou licença não sai" >&2; exit 1; }
done

rm -rf "$TRABALHO"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/free-eletrica" "$DIST"
install -m755 "$EXE" "$APPDIR/usr/bin/FreeEletrica"
strip "$APPDIR/usr/bin/FreeEletrica" 2>/dev/null || true
for par in "${ARQUIVOS[@]}"; do
  destino="$APPDIR/usr/share/free-eletrica/${par#*:}"
  mkdir -p "$(dirname "$destino")"
  cp "$RAIZ/${par%%:*}" "$destino"
done

# O lançador, o ícone e o atalho de menu.
cat >"$APPDIR/AppRun" <<'EOF'
#!/bin/sh
AQUI="$(dirname "$(readlink -f "$0")")"
exec "$AQUI/usr/bin/FreeEletrica" "$@"
EOF
chmod +x "$APPDIR/AppRun"
cp "$CAD/assets/app-icon/hicolor/256x256/apps/ai.storyteller.cadcraft.png" "$APPDIR/free-eletrica.png"
ln -s free-eletrica.png "$APPDIR/.DirIcon"
cat >"$APPDIR/free-eletrica.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Free Elétrica
GenericName=Projeto elétrico e de telecom
Comment=Projeto elétrico de baixa tensão (NBR 5410) e cabeamento estruturado (NBR 14565) no CAD
Exec=FreeEletrica %F
Icon=free-eletrica
Terminal=false
Categories=Graphics;Engineering;
MimeType=image/vnd.dxf;application/x-dxf;
Keywords=cad;dxf;elétrica;eletrica;nbr5410;telecom;
EOF
if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$APPDIR/free-eletrica.desktop"
fi

# O appimagetool, baixado na primeira vez. Extrair e rodar dispensa o FUSE (WSL, contêineres, CI).
FERRAMENTA="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$FERRAMENTA" ]; then
  FERRAMENTA="$CARGO_TARGET_DIR/appimagetool-x86_64.AppImage"
  if [ ! -x "$FERRAMENTA" ]; then
    curl -fsSL -o "$FERRAMENTA" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x "$FERRAMENTA"
  fi
fi
rm -f "$SAIDA"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$FERRAMENTA" --no-appstream "$APPDIR" "$SAIDA"
echo "pacote: $SAIDA"
ls -lh "$SAIDA"
