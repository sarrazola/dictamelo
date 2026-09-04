#!/usr/bin/env bash
# Compila el bundle de macOS (.app + .dmg) firmado con Developer ID.
# Firmar con una identidad estable hace que los permisos de macOS (Accesibilidad, Micrófono)
# y el acceso al Llavero se conserven entre compilaciones.
set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="${APPLE_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | grep -o '"Developer ID Application: [^"]*"' | head -1 | tr -d '"')}"
if [[ -z "$IDENTITY" ]]; then
  echo "No se encontró una identidad 'Developer ID Application'; se compila sin firmar (ad-hoc)." >&2
  npx tauri build "$@"
else
  echo "Firmando con: $IDENTITY"
  APPLE_SIGNING_IDENTITY="$IDENTITY" npx tauri build "$@"
fi

echo
echo "Resultados:"
ls -1 src-tauri/target/release/bundle/macos/*.app src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null || true
