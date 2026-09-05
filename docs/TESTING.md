# Verification record

## Version 0.2.0 — September 5, 2026

- Rust: 35 macOS tests passed; `cargo clippy --all-targets -- -D warnings` passed after correcting existing lint warnings. A separate real Keychain read/write/delete round trip passed.
- Free backend helpers: 3 Deno tests passed, including multilingual word segmentation, punctuation, actual PCM duration, malformed/truncated WAV, forged byte rate, and the two-minute boundary.
- Database quota test (`supabase/tests/free_quota.sql`): passed in a rolled-back transaction. Covers new-account usage, concurrent request blocking, full final recording accounting, exhausted allowance, duplicate completion, UTC weekly renewal, failed requests, attempt cap, and server-only SQL permissions.
- UI: browser preview exercised email entry, code entry, signed-in account, 742/2,000 used words, 1,258 remaining, and local renewal time. This uses mock data; it is not proof of email delivery or native credential persistence.
- macOS packaging: Apple accepted the app and DMG; both were stapled and Gatekeeper reported `source=Notarized Developer ID`. The downloaded draft DMG matched its local checksum, and its contained app passed signature, notarization-ticket, version, and Gatekeeper checks.
- Native Windows x64: [Actions run 33990373020](https://github.com/sarrazola/dictamelo/actions/runs/33990373020) passed 36 Rust tests and built the NSIS installer. The installed payload is PE `0x8664`. This CI artifact was signed locally with the existing updater key, then installed and launched in the ARM VM.
- Windows: see [WINDOWS_BUILD_REPORT.md](WINDOWS_BUILD_REPORT.md) for installation, ARM64 upgrade, functional checks, and release-script guard results. ARM emulation does not prove behavior on physical Intel/AMD hardware.
- Final draft assets: all nine files were downloaded into a fresh directory. Every checksum in `SHA256SUMS.txt` and all three updater signatures passed. The complete manifest contains exactly the three required platforms. Public download and in-app update checks remain for publication.

## Historical macOS verification — versions 0.1.0–0.1.2

Tests ran on macOS 26.5 / Apple Silicon. Earlier Rust suites covered resampling (voice-band preservation and anti-aliasing), mono mixing, WAV round trips, settings persistence/fallback, history limits, shortcut validation, provider errors, cleanup prompts, and silence-aware file splitting.

Live Groq checks transcribed synthesized Spanish speech with Whisper Large v3 Turbo/v3 in roughly 0.5–1.5 seconds. Cleanup with GPT-OSS corrected fillers and spoken self-corrections, including “Thursday, no, Friday”, without answering dictated questions. Invalid/missing credentials were rejected. These timings are historical observations, not service guarantees.

Clipboard integration tests preserved formats and restored the previous content. Signed-app self-tests exercised startup, keychain reads, transcription, temporary-file deletion, history, accessibility-denied fallback, sounds, launch at login, and Escape cancellation. Escape uses an AppKit monitor because registering it as an additional global shortcut interrupted the main held shortcut.

File tests included small M4A, AIFF converted through `afconvert`, and a 26-minute/50 MB WAV split into three chunks. No temporary audio remained after completion. A held-shortcut self-test captured 7.96 seconds from BlackHole, transcribed it, reported successful pasting, and restored the clipboard. Visual insertion into a real target app was not conclusively observed on that Mac because focus was contested by VMware and system dialogs. Physical spoken-microphone dictation was not tested in that historical session.

The real macOS updater was tested from 0.1.1 to 0.1.2: the installed bundle changed versions, updater signatures matched, Developer ID/Team ID stayed consistent, and existing microphone/accessibility access and the stored provider key remained available. Those historical installers were signed but **not notarized**.

## Historical Windows ARM64 verification — versions 0.1.0–0.1.2

Environment: Windows 11 ARM64 in VMware on Apple Silicon, 4 GB RAM, Rust 1.98, Node 24, VS Build Tools 2022 with ARM64 C++, Windows SDK 26100, Clang, and WebView2 152.

The 29-test suite passed, including clipboard format names, real snapshot/restore, virtual-key/scan-code mapping, tray icon alpha, and a real Windows Credential Manager round trip. A synthesized WAV self-test pasted 104 characters into the PowerShell target window, restored the prior clipboard, added history, and removed temporary audio.

A six-second held shortcut captured 5.7 seconds from the VM microphone. The overlay did not steal target-window focus. WASAPI startup discontinuities were made nonfatal. Escape cancellation worked using `GetAsyncKeyState` polling; a low-level keyboard hook had not worked reliably in the VM.

Media Foundation converted a 27.9 MB/14.5-minute WAV into two chunks and retained all 150 numbered phrases in order. WMA conversion, direct M4A upload, and AI cleanup also passed. Physical speech was not tested; the VM input provided silence/noise. Pasting into administrator-elevated apps is blocked by Windows UIPI by design.

Windows ARM64 0.1.2 was published alongside macOS, with both updater signatures verified. The release helper was corrected to pipe Python code through stdin, preserve an empty signing password in the process environment, select artifacts by version, and publish the actual signed NSIS `.exe` rather than misnaming it `.zip`. Physical x64 hardware, NSIS installation, and a version-to-version Windows update were not tested in that historical session.

## Historical Pro backend verification

Live calls to `transcribe` and `cleanup` in project `iburiyhhfodndqgmsaot` succeeded with an active Pro license. Missing/invalid licenses were rejected before provider use; invalid keys did not create database rows. Audio duration/cleanup usage was recorded without storing audio or transcript contents. Early request rejection was fixed to drain the incoming body, avoiding proxy timeouts while the client was still uploading.

## Reusable checks

```sh
npm test
npm run test:backend
npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
supabase db query --linked --file supabase/tests/free_quota.sql
```

Optional tests that touch real resources must be run deliberately:

```sh
DICTAMELO_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml live_tests -- --nocapture
DICTAMELO_CLIPBOARD_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml snapshot_and_restore
DICTAMELO_KEYRING_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml roundtrip_in_system_store
```

Record exact versions, architectures, and results for future releases. Do not treat a configured workflow, successful compile, UI mock, or valid signature as proof of a real installation or end-to-end user flow.

### Live free-account backend verification

A temporary account redeemed an email OTP generated through the Auth admin test API (no email sent), fetched zero initial usage, transcribed synthesized speech through the real hosted free endpoint, and recorded 11 words. After exhausting only that test account's allowance, another request returned HTTP 429. Missing/invalid authentication and direct client quota access were rejected. Refresh-token rotation succeeded. The temporary account and its usage were removed. Actual SMTP inbox delivery is a separate check.

### Clean checkout and final macOS updater archive

A fresh archive of tracked source at `28160ef` passed `npm ci` and `cargo check --locked`. The final macOS updater archive was extracted into a fresh temporary directory; the extracted app passed strict code-signature, stapler, and Gatekeeper verification. All new account labels are present in all six interface languages.
