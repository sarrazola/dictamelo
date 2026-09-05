# Release runbook

Release artifacts live in GitHub Releases. Source, release notes, and these instructions live in Git. `.gitignore` only excludes generated files and credentials; `AGENTS.md` tells coding assistants to follow this runbook.

## 1. Prepare the version

Start from current `main` and preserve any unrelated work. Choose a new version; never replace an already-public installer's bytes.

```sh
git pull --ff-only
python3 scripts/set-version.py 0.2.1
```

This updates `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Add `docs/releases/0.2.1.md` in English, add a CHANGELOG entry, and review the README's platform support, features, plan limits, installation steps, and download filenames. Update the UI preview version if needed. Do not claim a platform or signing status based only on configuration.

## 2. Validate and deploy backend changes

```sh
npm ci
npm test
npm run test:backend
npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
deno check supabase/functions/{transcribe,cleanup,usage}/index.ts
supabase link --project-ref iburiyhhfodndqgmsaot
supabase db push --dry-run
supabase db push
supabase db query --linked --file supabase/tests/free_quota.sql
supabase functions deploy transcribe cleanup usage --project-ref iburiyhhfodndqgmsaot
```

For a repeatable hosted test on macOS with Python `requests` installed, run `python3 scripts/verify-free-backend.py --live`. It creates its own temporary account, redeems an admin-generated email code without sending a message, transcribes one short synthesized recording, verifies quota/auth/refresh behavior, and removes that account. This incurs one small provider request.

The manually triggered **Windows x64 verification** GitHub Actions workflow runs tests and builds an unsigned installer on a native x64 Windows runner. Its artifact is for verification only; release installers still need the existing Tauri signing key. Trigger it with `gh workflow run windows-check.yml --ref main`.

Deploy compatible schema before dependent functions, and functions before distributing clients. The quota SQL test rolls back its temporary account and usage. Verify invalid authentication, real free transcription, word accounting, quota exhaustion, session refresh, and existing Pro requests. Use a temporary test account, never a customer's account.

For email-code login, configure a production SMTP sender in the project's **Authentication → Email → SMTP settings**. Supabase's default email service does not deliver to arbitrary users. Both Magic Link and Confirmation templates must include `{{ .Token }}`; their source is `supabase/templates/sign-in.html`. Keep email confirmations enabled. Verify actual delivery and code redemption with your own mailbox before announcing the free plan.

`supabase config push` applies more than templates: it can change Auth/API/Storage defaults. Inspect the diff and explicitly preserve production settings. Prefer a targeted Auth Management API patch for SMTP changes. Never put SMTP passwords in `config.toml`; use environment references or the dashboard. The configured sender and required secrets must be recorded without secret values.

Required Edge Function secret: `GROQ_API_KEY`. Optional Pro safeguard: `MONTHLY_SECONDS` (default 72,000 seconds). The free allowance is 2,000 words in the migration functions. SQL functions and quota tables are server-only; do not grant authenticated clients access to the reservation/finish functions.

## 3. Build and verify macOS

The Mac needs a valid **Developer ID Application** certificate and a `notarytool` keychain profile for its Apple Developer team. The existing build machine has profile `SnipHaloNotary` for team `2V4GNZ89F6`; the script's portable default profile name is `DictameloNotary`. Use the existing profile or create one with `xcrun notarytool store-credentials DictameloNotary` interactively. Do not put Apple passwords or private keys in the repository.

The existing Tauri updater private key is stored in Keychain, service `com.dictamelo.desktop`, account `updater_private_key`. Back it up in a secure password manager. **All build machines must use this same key.** Do not generate a replacement for an ordinary release.

```sh
NOTARY_KEYCHAIN_PROFILE=SnipHaloNotary ./scripts/build-release.sh
```

The script explicitly targets `aarch64-apple-darwin`, signs with Developer ID, submits the app to Apple, staples and verifies it, rebuilds the updater archive from the stapled app, signs that archive with Tauri, then creates/signs/notarizes/staples/verifies the DMG. It refuses to proceed without the required identity, key, or notarization profile.

Outputs are under `src-tauri/target/aarch64-apple-darwin/release/bundle/`. Keep the `.app.tar.gz.sig` beside the archive. A successful build without an `Accepted` notarization response is not a finished release.

## 4. Build both Windows targets

Use the same committed source and version on Windows. Ensure MSVC x64/ARM64 toolchains, Windows SDK, Clang, and NASM are installed. Set `TAURI_SIGNING_PRIVATE_KEY` securely in the build process, using the same key as macOS. An empty updater-key password needs an actual empty environment entry on Windows; the build script handles this with `ProcessStartInfo`.

Run the Windows build script once per explicit Rust target:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target aarch64-pc-windows-msvc
```

Verify the installed application executable's PE machine type and product version for each target. The NSIS bootstrap itself may be x86, so inspecting only the installer header does not prove payload architecture. Install and run each build, test clipboard restoration, microphone capture, file conversion, credentials, and updating from the previous installed version. Record whether x64 was tested on physical Intel/AMD hardware or under ARM emulation.

Copy the signed outputs to the Mac's ignored `dist/windows/` folder, named exactly:

```text
Dictamelo_0.2.1_x86_64-setup.exe
Dictamelo_0.2.1_x86_64-setup.exe.sig
Dictamelo_0.2.1_aarch64-setup.exe
Dictamelo_0.2.1_aarch64-setup.exe.sig
```

A private/draft GitHub Release can transfer artifacts between machines (`gh release upload` / `gh release download`). Keep it draft until all platforms are verified. Do not allow two machines to rewrite `latest.json` simultaneously. The existing Windows publishing helper can append a platform to a draft/staged release; the full-release procedure below regenerates the final manifest from all three verified artifacts.

## 5. Commit, stage, and publish

Update the testing record with actual results and limitations. Review `git diff` and stage only intended paths. Commit and push the source. Ensure source files did not change after the final build; rebuild affected artifacts if they did.

```sh
python3 scripts/stage-release.py 0.2.1
cargo run --quiet --manifest-path src-tauri/Cargo.toml --example verify_release -- "$PWD/dist/v0.2.1"
./scripts/release.sh 0.2.1
```

The release script requires a clean `main`, the correct macOS bundle version, signed artifacts for all three platforms, valid updater signatures, and stapled Apple artifacts. It creates/pushes only this release's tag, uploads a draft's artifacts, and then publishes it as latest. It does not stage arbitrary source changes or push every local tag.

The release contains the macOS DMG, macOS updater archive and `.sig`, both Windows `.exe` installers and `.sig`, `latest.json`, and `SHA256SUMS.txt`. The manifest must contain `darwin-aarch64`, `windows-x86_64`, and `windows-aarch64`, with exact case-sensitive asset URLs. The Windows updater consumes the signed `.exe` directly, not a renamed `.zip`.

## 6. Verify the public release

Download the assets again into a fresh directory using `gh release download v0.2.1`. Compare SHA-256 checksums and verify every updater signature against the app's public key. Run:

```sh
DICTAMELO_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml published_release_signature_is_valid -- --nocapture
xcrun stapler validate /path/to/downloaded.dmg
spctl --assess --type open --context context:primary-signature --verbose=2 /path/to/downloaded.dmg
```

Mount the downloaded DMG and check the contained app with `codesign --verify --deep --strict`, `xcrun stapler validate`, and `spctl --assess --type execute`. Confirm `source=Notarized Developer ID`. Verify a real old-version-to-new-version update separately; a valid signature alone is not an installation test.

GitHub's public download URL may cache an earlier manifest briefly. Compare the exact release asset/API response before diagnosing an upload failure, then verify the public URL after it refreshes. Do not overwrite public installer bytes to fix a release: publish the next version.
