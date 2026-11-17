#!/usr/bin/env bash
# Instala o Kairo para o usuário atual: binário em ~/.local/bin, atalho e ícone no menu.
set -euo pipefail
cd "$(dirname "$0")/.."
prefix="$HOME/.local"

cargo install --path . --bin kairo --root "$prefix"
install -Dm644 assets/icons/scalable/apps/dev.example.Kairo.svg \
    "$prefix/share/icons/hicolor/scalable/apps/dev.example.Kairo.svg"
# Caminho absoluto: o PATH da sessão gráfica nem sempre inclui ~/.local/bin.
sed "s|^Exec=kairo|Exec=$prefix/bin/kairo|" data/dev.example.Kairo.desktop |
    install -Dm644 /dev/stdin "$prefix/share/applications/dev.example.Kairo.desktop"
update-desktop-database "$prefix/share/applications" 2>/dev/null || true
