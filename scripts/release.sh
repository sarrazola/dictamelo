#!/usr/bin/env bash
# Publica una versión: compila firmado, arma latest.json y lo sube todo a GitHub Releases.
#
#   ./scripts/release.sh 0.1.1 "Notas de la versión"
#
# La llave privada de firma vive en el llavero (cuenta `updater_private_key`), nunca en el
# repositorio. Sin ella la app no aceptaría la actualización, porque comprueba la firma
# contra la llave pública que va incrustada en el binario.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
NOTES="${2:-}"
if [[ -z "$VERSION" ]]; then
  echo "Uso: $0 <versión> [notas]" >&2
  exit 1
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "La versión debe ser X.Y.Z (recibí «$VERSION»)" >&2
  exit 1
fi

REPO="sarrazola/dictamelo"
TAG="v$VERSION"
STAGE="dist/$TAG"
KEYCHAIN_SERVICE="com.dictamelo.desktop"

if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  echo "El release $TAG ya existe. Sube la versión o bórralo antes." >&2
  exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Hay cambios sin confirmar. Haz commit antes de publicar." >&2
  exit 1
fi

echo "==> Poniendo la versión $VERSION en los manifiestos"
python3 - "$VERSION" <<'PY'
import json, re, sys, collections, pathlib
version = sys.argv[1]
p = pathlib.Path('src-tauri/tauri.conf.json')
conf = json.loads(p.read_text(), object_pairs_hook=collections.OrderedDict)
conf['version'] = version
p.write_text(json.dumps(conf, indent=2, ensure_ascii=False) + '\n')

p = pathlib.Path('src-tauri/Cargo.toml')
p.write_text(re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', p.read_text(), count=1))

p = pathlib.Path('package.json')
pkg = json.loads(p.read_text(), object_pairs_hook=collections.OrderedDict)
pkg['version'] = version
p.write_text(json.dumps(pkg, indent=2) + '\n')
PY

echo "==> Compilando y firmando"
TAURI_SIGNING_PRIVATE_KEY="$(security find-generic-password -s "$KEYCHAIN_SERVICE" -a updater_private_key -w)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
  ./scripts/build-release.sh

BUNDLE="src-tauri/target/release/bundle"
rm -rf "$STAGE" && mkdir -p "$STAGE"

# El .dmg es para quien instala a mano; el .app.tar.gz y su firma son los que usa el actualizador.
DMG=$(ls "$BUNDLE"/dmg/*.dmg | head -1)
TARBALL=$(ls "$BUNDLE"/macos/*.app.tar.gz | head -1)
SIGFILE="$TARBALL.sig"
[[ -f "$SIGFILE" ]] || { echo "Falta la firma $SIGFILE: ¿se compiló sin la llave?" >&2; exit 1; }

# Nombres sin acentos: las URL de descarga se leen y se comparten mejor.
cp "$DMG"     "$STAGE/Dictamelo_${VERSION}_aarch64.dmg"
cp "$TARBALL" "$STAGE/Dictamelo_${VERSION}_aarch64.app.tar.gz"
MAC_SIG=$(cat "$SIGFILE")

echo "==> Armando latest.json"
python3 - "$VERSION" "$TAG" "$REPO" "$STAGE" "$MAC_SIG" "$NOTES" <<'PY'
import json, os, sys, pathlib, datetime
version, tag, repo, stage, mac_sig, notes = sys.argv[1:7]
base = f"https://github.com/{repo}/releases/download/{tag}"
platforms = {
    "darwin-aarch64": {"signature": mac_sig, "url": f"{base}/Dictamelo_{version}_aarch64.app.tar.gz"},
}
# Si alguien dejó los artefactos de Windows en dist/windows, entran en el mismo latest.json.
win_dir = pathlib.Path("dist/windows")
if win_dir.is_dir():
    for sig in sorted(win_dir.glob("*.sig")):
        payload = sig.with_suffix("")
        if not payload.exists():
            continue
        target = pathlib.Path(stage) / payload.name
        target.write_bytes(payload.read_bytes())
        platforms["windows-x86_64"] = {
            "signature": sig.read_text().strip(),
            "url": f"{base}/{payload.name}",
        }
        print(f"    incluido Windows: {payload.name}")

manifest = {
    "version": version,
    "notes": notes or f"Dictámelo {version}",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}
out = pathlib.Path(stage) / "latest.json"
out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
print("    plataformas:", ", ".join(platforms))
PY

echo "==> Publicando $TAG en GitHub"
git add -A
git commit -qm "Versión $VERSION" || true
git tag -a "$TAG" -m "Dictámelo $VERSION"
git push origin main --tags

gh release create "$TAG" "$STAGE"/* \
  --repo "$REPO" \
  --title "Dictámelo $VERSION" \
  --notes "${NOTES:-Dictámelo $VERSION}"

echo
echo "Listo. Las apps instaladas verán la actualización en su próxima comprobación."
gh release view "$TAG" --repo "$REPO" --json url --jq .url
