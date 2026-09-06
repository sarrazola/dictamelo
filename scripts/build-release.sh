#!/usr/bin/env bash
# Build, Developer ID sign, notarize and staple the macOS app and DMG.
# The updater archive is created AFTER stapling, then signed with the existing Tauri key.
set -euo pipefail
cd "$(dirname "$0")/.."
# Fail before reading signing credentials or building if an offline regression fails.
node --check ui/main.js
node --check ui/i18n.js
npm run test:ui
python3 scripts/check-audio-fixture.py
cargo test --locked --manifest-path src-tauri/Cargo.toml
if [[ "${DICTAMELO_LIVE_REGRESSION:-0}" == "1" ]]; then
  : "${DICTAMELO_TEST_PROJECT_REF:?Set the explicit Supabase project for the live regression}"
  python3 scripts/test-free-cleanup-live.py --live --project-ref "$DICTAMELO_TEST_PROJECT_REF"
fi
IDENTITY="${APPLE_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}"
PROFILE="${NOTARY_KEYCHAIN_PROFILE:-DictameloNotary}"
[[ -n "$IDENTITY" ]] || { echo 'A Developer ID Application certificate is required.' >&2; exit 1; }
xcrun notarytool history --keychain-profile "$PROFILE" --output-format json >/dev/null
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(security find-generic-password -s com.dictamelo.desktop -a updater_private_key -w)"
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
export APPLE_SIGNING_IDENTITY="$IDENTITY"
VERSION=$(node -p 'require("./src-tauri/tauri.conf.json").version')
TARGET="${MACOS_TARGET:-aarch64-apple-darwin}"
case "$TARGET" in
  aarch64-apple-darwin) ARCH=aarch64 ;;
  x86_64-apple-darwin) ARCH=x86_64 ;;
  *) echo 'MACOS_TARGET must be aarch64-apple-darwin or x86_64-apple-darwin' >&2; exit 1 ;;
esac
rustup target add "$TARGET"
npx tauri build --target "$TARGET" --bundles app "$@"
BUNDLE="src-tauri/target/$TARGET/release/bundle"
APP="$BUNDLE/macos/Dictámelo.app"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/dictamelo-notary.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
codesign --verify --deep --strict "$APP"
/usr/bin/ditto -c -k --keepParent "$APP" "$WORK/app.zip"
xcrun notarytool submit "$WORK/app.zip" --keychain-profile "$PROFILE" --wait --output-format json > "$WORK/app-notary.json"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d); sys.exit(0 if d.get("status")=="Accepted" else 1)' "$WORK/app-notary.json"
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=2 "$APP"
# Tauri generated an archive before notarization; replace it with the stapled app.
TARBALL="$BUNDLE/macos/Dictámelo.app.tar.gz"
COPYFILE_DISABLE=1 tar -czf "$TARBALL" -C "$BUNDLE/macos" 'Dictámelo.app'
npx tauri signer sign "$TARBALL" >/dev/null
mkdir -p "$WORK/dmg" "$BUNDLE/dmg"
/usr/bin/ditto "$APP" "$WORK/dmg/Dictámelo.app"
ln -s /Applications "$WORK/dmg/Applications"
DMG="$BUNDLE/dmg/Dictamelo_${VERSION}_${ARCH}.dmg"
hdiutil create -volname 'Dictámelo' -srcfolder "$WORK/dmg" -ov -format UDZO "$DMG"
codesign --force --sign "$IDENTITY" --timestamp "$DMG"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait --output-format json > "$WORK/dmg-notary.json"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d); sys.exit(0 if d.get("status")=="Accepted" else 1)' "$WORK/dmg-notary.json"
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"
spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
printf 'Verified artifacts:\n%s\n%s\n' "$DMG" "$TARBALL"
