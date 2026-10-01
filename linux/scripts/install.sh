#!/usr/bin/env bash
# Builds Coucou and installs it for the current user — no root, nothing outside
# your home directory:
#
#   ~/.local/bin/coucou                       the app
#   ~/.local/lib/coucou/coucou-hook           the relay Claude Code calls
#   ~/.local/share/applications/coucou.desktop
#   ~/.local/share/icons/hicolor/128x128/apps/coucou.png
#
#   scripts/install.sh              build and install
#   scripts/install.sh --uninstall  remove all of the above (settings and keys stay)

set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS="$DATA/applications"
ICONS="$DATA/icons/hicolor/128x128/apps"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$PREFIX/bin/coucou" "$PREFIX/lib/coucou/coucou-hook" "$APPS/coucou.desktop" "$ICONS/coucou.png"
  rmdir "$PREFIX/lib/coucou" 2>/dev/null || true
  echo "Removed. Your settings (~/.config/coucou) and the Claude Code hooks are untouched:"
  echo "remove the hooks from Coucou's settings window first if you want them gone."
  exit 0
fi

cd "$(dirname "$0")/.."

for tool in node npm cargo pkg-config; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
for lib in webkit2gtk-4.1 gtk+-3.0 gtk-layer-shell-0; do
  pkg-config --exists "$lib" || { echo "missing library: $lib (see README.md)" >&2; exit 1; }
done

npm ci
npm run build                                       # type-check, bundle, build coucou-hook
cargo build --release -p coucou --features custom-protocol

install -Dm755 target/release/coucou "$PREFIX/bin/coucou"
install -Dm755 target/release/coucou-hook "$PREFIX/lib/coucou/coucou-hook"
install -Dm644 src-tauri/icons/128x128.png "$ICONS/coucou.png"
install -d "$APPS"
cat > "$APPS/coucou.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Coucou
Comment=Mochi lives on the edge of your screen
Exec=$PREFIX/bin/coucou
Icon=coucou
Categories=Development;
Terminal=false
StartupNotify=false
DESKTOP

echo
echo "Installed. Start it from your launcher, or run: $PREFIX/bin/coucou"
echo "Then open Settings… → Claude Code → Install hooks…"
