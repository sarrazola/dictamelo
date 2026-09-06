# Release runbook

Release artifacts live in GitHub Releases. Source, release notes, and these instructions live in Git. `.gitignore` only excludes generated files and credentials; `AGENTS.md` tells coding assistants to follow this runbook.

## Current release candidate: 0.5.0

This iteration restores macOS and both Windows targets. Push the reviewed, tested source to `main` before asking the Windows machine to pull and build. Keep the same source commit, version and public cloud metadata across all three artifacts. Record the actual results in [Testing](TESTING.md).

The first distribution may be a GitHub **prerelease** while the external launch requirements in [Production readiness](PRODUCTION_READINESS.md) remain open. A prerelease provides public installer downloads without changing the stable automatic-update channel. Do not advertise universal cloud signup, verified email delivery or the seven-day trial before those flows actually work. Keep `DICTAMELO_PRO_TRIAL_AVAILABLE=false` until trial entitlement has been verified.

## 1. Prepare the version

Start from current `main` and preserve any unrelated work. Choose a new version; never replace an already-public installer's bytes.

```sh
git pull --ff-only
python3 scripts/set-version.py 0.5.0
```

This updates `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Add `docs/releases/0.5.0.md` in English, add a CHANGELOG entry, and review the README's platform support, features, plan limits, installation steps, and download filenames. Update the UI preview version if needed. Do not claim a platform or signing status based only on configuration.

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
supabase db query --linked --file supabase/tests/pro_quota.sql
supabase functions deploy transcribe cleanup usage --project-ref iburiyhhfodndqgmsaot
```

For a repeatable hosted test on macOS with Python `requests` installed, run `python3 scripts/verify-free-backend.py --live`. It creates its own temporary account, redeems an admin-generated email code without sending a message, transcribes the committed licensed English speech fixture, verifies quota/auth/refresh behavior, and removes that account. This incurs one small provider request.

The manually triggered **Windows builds and regression checks** GitHub Actions workflow runs the tests on native x64 Windows and builds both explicit targets. ARM64 is cross-compiled, and its payload execution still needs an ARM machine. Unsigned CI artifacts require the existing Tauri signing key and independent verification before distribution. The official-cloud option uses validated public repository variables; ordinary PR builds stay unconfigured. Trigger it with `gh workflow run windows-check.yml --ref main -f official_cloud=true`.

Deploy compatible schema before dependent functions, and functions before distributing clients. Both quota SQL tests roll back their temporary data. Verify invalid authentication, real free transcription, audio-time accounting, quota exhaustion, session refresh, existing Pro licenses, product ownership, concurrency and immediate transcription-to-cleanup. Use temporary test data, never a customer's account. A clean local PostgreSQL database can test the migration/RPC behavior without changing production; record that distinction.

Free Cloud allows 30 minutes per UTC week, using measured PCM audio time and preserving legacy usage. AI cleanup adds no audio time. The paid service now uses 180 hours, 36,000 provider requests, 9M cleanup input tokens and 6M completion tokens per rolling 30 days. It fixes hosted models to Turbo and GPT-OSS 20B. Current PCM duration is validated before reserving quota. Legacy compressed uploads remain compatible with a conservative reservation and a post-provider duration check; record actual usage even if that result is rejected. Do not describe this legacy route as a strict preflight cost ceiling.

Read [Auth and cloud configuration](AUTH_AND_CLOUD.md) before deploying account changes. The current client creates email/password accounts, signs in with a password or Google, and uses confirmation/recovery codes only for those actions. Keep email confirmations enabled and configure production SMTP delivery. Confirmation/recovery templates must include `{{ .Token }}`. Verify delivery, confirmation, password login and password recovery with an owned mailbox; admin-generated test tokens are not an inbox test.

For Google, configure Supabase's provider using the selected Google Cloud project's Web client, keep the client secret server-side, register the Supabase callback with Google, and allow the native loopback callback documented in the auth guide. Test the complete browser-to-installed-app flow. Keep the seven-day trial action off until checkout and trial access/expiry have been verified.

`supabase config push` applies more than templates: it can change Auth/API/Storage defaults. Inspect the diff and explicitly preserve production settings. Prefer a targeted Auth Management API patch for SMTP/provider changes. Never put SMTP or Google secrets in tracked configuration.

Required hosted provider secret: `GROQ_API_KEY`. Required Pro ownership configuration: verified `LEMON_STORE_ID`, `LEMON_PRODUCT_ID` and comma-separated `LEMON_VARIANT_IDS`, including variants that existing customers use. Configure these before deploying handlers that fail closed without them. The SQL quota migration is authoritative; the former `MONTHLY_SECONDS` variable no longer sets the allowance. Quota tables and mutation RPCs must remain service-only.

Review and commit the tested application source, then push `main` before starting builds on other machines. Keep all build machines on that same application source and version. Later documentation-only verification records can be committed separately.

## 3. Configure and build macOS

A clean source build leaves hosted services and automatic updates disabled and uses personal API keys. The official build uses the same public source with an ignored `.env.cloud-build` file containing only public endpoint, key, checkout and product metadata. See [the exact fields](AUTH_AND_CLOUD.md#desktop-build-metadata). Never put provider, service-role, SMTP, Google-client or signing secrets in this file. Do not use a second modified copy of the app.

Set `DICTAMELO_UPDATES_ENABLED=true` only for an official build, or after configuring your own updater URL and verification key; its default is false. The build wrapper injects these values into the command process. They are compile-time configuration, so changing them requires rebuilding. A private operations repository can pin the public source as a submodule and supply the same file without adding private UI overlays.

The Mac needs a valid **Developer ID Application** certificate and a `notarytool` keychain profile for its Apple Developer team. The existing build machine has profile `SnipHaloNotary` for team `2V4GNZ89F6`; the script's portable default profile name is `DictameloNotary`. Use the existing profile or create one with `xcrun notarytool store-credentials DictameloNotary` interactively. Do not put Apple passwords or private keys in the repository.

The existing Tauri updater private key is stored in Keychain, service `com.dictamelo.desktop`, account `updater_private_key`. Back it up in a secure password manager. **All build machines must use this same key.** Do not generate a replacement for an ordinary release.

Runtime credentials use different namespaces: release `.runtime.v1` and debug `.runtime.debug.v1`. Debug builds do not migrate release credentials; use dedicated test keys when developing. The runtime store rejects `updater_*` entries. Do not move signing keys into runtime storage or change their ACLs as a workaround for an application prompt. See [Local credentials](LOCAL_CREDENTIALS.md).

```sh
NOTARY_KEYCHAIN_PROFILE=SnipHaloNotary python3 scripts/with-cloud-config.py --config .env.cloud-build -- ./scripts/build-release.sh
```

The script explicitly targets `aarch64-apple-darwin`, signs with Developer ID, submits the app to Apple, staples and verifies it, rebuilds the updater archive from the stapled app, signs that archive with Tauri, then creates/signs/notarizes/staples/verifies the DMG. It refuses to proceed without the required identity, key, or notarization profile.

Outputs are under `src-tauri/target/aarch64-apple-darwin/release/bundle/`. Keep the `.app.tar.gz.sig` beside the archive. A successful build without an `Accepted` notarization response is not a finished release.

### Verify a Mac-only preview without publishing

Stage the final 0.3.1 local files in `dist/v0.3.1-macos-preview/`. Verify the single updater archive with:

```sh
cargo run --quiet --manifest-path src-tauri/Cargo.toml --example verify_artifact -- \
  "dist/v0.3.1-macos-preview/Dictámelo.app.tar.gz" \
  "dist/v0.3.1-macos-preview/Dictámelo.app.tar.gz.sig"
```

Check `SHA256SUMS.txt` from that directory, validate the DMG with `xcrun stapler` and Gatekeeper, mount it read-only, and inspect the actual contained app before copying it to Applications. Verify the installed app too. This preview path does not require Windows artifacts and must not create or upload `latest.json`; the complete-release verifier and publisher below serve a different, later release step.

## 4. Build both Windows targets

Use the same committed source, version and appropriate explicit public build metadata on Windows. Ensure MSVC x64/ARM64 toolchains, Windows SDK, Clang, and NASM are installed. Set `TAURI_SIGNING_PRIVATE_KEY` securely in the build process, using the same key as macOS. An empty updater-key password needs an actual empty environment entry on Windows; the build script handles this with `ProcessStartInfo`.

Run the Windows build script once per explicit Rust target:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target aarch64-pc-windows-msvc
```

Verify the installed application executable's PE machine type and product version for each target. The NSIS bootstrap itself may be x86, so inspecting only the installer header does not prove payload architecture. Install and run each build, test first-launch setup and Skip, settings-window persistence, clipboard restoration, microphone capture, file conversion, credentials, and updating from the previous installed version. Record whether x64 was tested on physical Intel/AMD hardware or under ARM emulation.

CI uses the Windows runner's existing 7-Zip to extract its unsigned NSIS installer, checks both application executables' PE architecture and file/product versions, and runs `scripts/windows_payload.py` to compare their bytes. Tauri CLI 2.11.4 [patches the bundle-type marker](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs#L36-L92) from `UNK` to `NSS` before NSIS packaging and [restores the compiler output afterward](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs#L128-L203). The helper requires exactly one compiler `UNK` marker and permits only its same-offset change to `NSS`; pre-existing `NSS` strings must stay unchanged, and any other byte difference fails the build. It never rewrites an executable. `build-verification.json` records `compiledPayloadSha256`, `packagedPayloadSha256` and `installerSha256` separately; the legacy `payloadSha256` remains an explicitly labeled compiler-output alias. Compare an installed executable with **`packagedPayloadSha256`**. This unsigned-CI check does not normalize Authenticode or replace signature/runtime verification. An upstream packaging change must be investigated before adapting the check.

Copy the signed outputs to the Mac's ignored `dist/windows/` folder, named exactly:

```text
Dictamelo_0.5.0_x86_64-setup.exe
Dictamelo_0.5.0_x86_64-setup.exe.sig
Dictamelo_0.5.0_aarch64-setup.exe
Dictamelo_0.5.0_aarch64-setup.exe.sig
```

Create a draft to transfer artifacts between machines:

```sh
gh release create v0.5.0 --draft --target main --title "Dictámelo 0.5.0" --notes-file docs/releases/0.5.0.md
```

Use `gh release upload` / `gh release download` to transfer files. Keep it draft until all platforms are verified. Do not allow two machines to rewrite `latest.json` simultaneously. The Windows publishing helper refuses to overwrite public releases and can append a platform to a draft release; the full-release procedure below regenerates the final manifest from all three verified artifacts.

When the x64 release artifact comes from native CI, upload only the ARM64 pair from the VM. The helper otherwise defaults to both targets and could replace the draft's CI artifact with a different cross-built file:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.5.0 -Targets aarch64-pc-windows-msvc -SkipBuild -AssetsOnly
```

## 5. Commit, stage, and publish a complete release

Update the testing record with actual results and limitations. Review `git diff` and stage only intended paths. Commit and push the source. Ensure source files did not change after the final build; rebuild affected artifacts if they did.

```sh
python3 scripts/stage-release.py 0.5.0
cargo run --quiet --manifest-path src-tauri/Cargo.toml --example verify_release -- "$PWD/dist/v0.5.0"
./scripts/release.sh 0.5.0
```

The release script requires a clean `main`, the correct macOS bundle version, signed artifacts for all three platforms, valid updater signatures, and stapled Apple artifacts. It creates/pushes only this release's tag, uploads a draft's artifacts, and then publishes it as latest. It does not stage arbitrary source changes or push every local tag.

The release contains the macOS DMG, macOS updater archive and `.sig`, both Windows `.exe` installers and `.sig`, `latest.json`, and `SHA256SUMS.txt`. The manifest must contain `darwin-aarch64`, `windows-x86_64`, and `windows-aarch64`, with exact case-sensitive asset URLs. The Windows updater consumes the signed `.exe` directly, not a renamed `.zip`.

## 6. Verify the public release

Download the assets again into a fresh directory using `gh release download v0.5.0`. Compare SHA-256 checksums and verify every updater signature against the app's public key. Run:

```sh
DICTAMELO_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml published_release_signature_is_valid -- --ignored --nocapture
xcrun stapler validate /path/to/downloaded.dmg
spctl --assess --type open --context context:primary-signature --verbose=2 /path/to/downloaded.dmg
```

Mount the downloaded DMG and check the contained app with `codesign --verify --deep --strict`, `xcrun stapler validate`, and `spctl --assess --type execute`. Confirm `source=Notarized Developer ID`. Verify a real old-version-to-new-version update separately; a valid signature alone is not an installation test.

GitHub's public download URL may cache an earlier manifest briefly. Compare the exact release asset/API response before diagnosing an upload failure, then verify the public URL after it refreshes. Do not overwrite public installer bytes to fix a release: publish the next version.

## Publishing a release candidate

Use this only after staging and verifying the complete set of artifacts above. The installers are immutable once public, including prereleases. A later correction requires a new version.

```sh
./scripts/release.sh 0.5.0 --prerelease
```

The script checks that the release is still a draft before uploading, then publishes it as a prerelease without changing the stable channel. Re-download and verify every checksum and updater signature after publication. Keep the README download table linked to the exact version and architecture that was checked. When production requirements are satisfied, promoting this identical candidate to stable is a metadata change; do not replace its bytes. Verify the stable updater endpoint and a real old-version update after promotion.
