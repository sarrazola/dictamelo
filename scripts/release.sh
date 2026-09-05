#!/usr/bin/env bash
# Publish a complete release after the source and all three architectures have been verified.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="${1:-}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Usage: scripts/release.sh X.Y.Z'; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo 'Commit the reviewed source changes first.' >&2; exit 1; }
[[ "$(git branch --show-current)" == main ]] || { echo 'Release from main.' >&2; exit 1; }
TAG="v$VERSION"
NOTES="docs/releases/$VERSION.md"
[[ -f "$NOTES" ]] || { echo "Missing $NOTES" >&2; exit 1; }
if gh release view "$TAG" --json isDraft --jq .isDraft 2>/dev/null | rg -qx false; then
  echo 'This release is already public. Publish a new version instead of overwriting installers.' >&2; exit 1
fi
python3 scripts/stage-release.py "$VERSION"
STAGE="dist/$TAG"
cargo run --quiet --manifest-path src-tauri/Cargo.toml --example verify_release -- "$PWD/$STAGE"
APP='src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Dictámelo.app'
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=2 "$APP"
xcrun stapler validate "$STAGE/Dictamelo_${VERSION}_aarch64.dmg"
codesign --verify --strict "$STAGE/Dictamelo_${VERSION}_aarch64.dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$STAGE/Dictamelo_${VERSION}_aarch64.dmg"
if git rev-parse "$TAG" >/dev/null 2>&1; then
  [[ "$(git rev-list -n 1 "$TAG")" == "$(git rev-parse HEAD)" ]] || { echo 'Tag does not match HEAD.' >&2; exit 1; }
else
  git tag -a "$TAG" -m "Dictámelo $VERSION"
fi
git push origin main "refs/tags/$TAG"
if ! gh release view "$TAG" >/dev/null 2>&1; then
  gh release create "$TAG" --draft --verify-tag --title "Dictámelo $VERSION" --notes-file "$NOTES"
fi
# Draft assets can be refreshed while verification is in progress. Public assets are immutable.
[[ "$(gh release view "$TAG" --json isDraft --jq .isDraft)" == true ]] || {
  echo 'Refusing to replace assets without a confirmed draft release.' >&2; exit 1;
}
gh release upload "$TAG" "$STAGE"/* --clobber
gh release edit "$TAG" --title "Dictámelo $VERSION" --notes-file "$NOTES" --draft=false --latest
DICTAMELO_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml published_release_signature_is_valid -- --nocapture
gh release view "$TAG" --json url --jq .url
