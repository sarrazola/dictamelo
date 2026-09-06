<p align="center"><img src="assets/logo-original.png" width="128" alt="Dictámelo"></p>

# Dictámelo

Voice dictation for any app on **macOS and Windows**. Hold a shortcut, speak, and release to put the text at your cursor. Built with Rust and Tauri 2.

[Downloads](https://github.com/sarrazola/dictamelo/releases) · [Setup](#get-started) · [Development](#development) · [Release guide](docs/RELEASING.md)

## Download

**0.4.0 release candidate:** the three installers are being verified. Downloads below will be enabled when the checked artifacts are public. The [current stable release](https://github.com/sarrazola/dictamelo/releases/latest) remains available.

| Your computer | Installer |
| --- | --- |
| Apple Silicon Mac — M1, M2, M3, M4 and later | `Dictamelo_0.4.0_aarch64.dmg` — pending verification |
| Windows — Intel or AMD 64-bit | `Dictamelo_0.4.0_x86_64-setup.exe` — pending verification |
| Windows 11 — ARM64 | `Dictamelo_0.4.0_aarch64-setup.exe` — pending verification |

On macOS, open the DMG and drag Dictámelo into Applications. Allow Microphone and Accessibility when requested. On Windows, run the installer and allow desktop microphone access in Windows Settings. Intel Macs, 32-bit Windows and Linux installers are not provided.

Mac releases require Developer ID signing, Apple notarization and stapling. Windows installers are signed for the Tauri updater, but do not currently have a Microsoft Authenticode certificate; SmartScreen may display a warning. See [the actual verification record](docs/TESTING.md).

**Cloud preview:** Google sign-in currently accepts permitted test users. Production email confirmation/recovery delivery and the proposed seven-day Pro trial are pending. Personal-key mode works without a Dictámelo account. A GitHub prerelease does not change the stable automatic-update channel.

## Get started

1. Open **Onboarding** and choose your own keys, Free Cloud or Pro. The button remains visible so you can repeat the setup.
2. With your own keys, choose Groq or OpenAI in **Models** and save the provider key. No Dictámelo account is required.
3. In an official cloud build, use **Create free account** with email/password, **Sign in**, or **Continue with Google**. Cloud preview restrictions above still apply. Existing Pro licenses can be activated in **Plan**.
4. Review language, shortcut and permissions. Hold **Alt/Option + Shift + Space**, speak, and release.
5. Open **Files** to transcribe a recording. Optional AI cleanup applies to dictation and uploaded files. If cleanup fails, the original transcript stays available.

## Three plans

| | Free — your own keys | Free Cloud | Pro |
| --- | --- | --- | --- |
| Dictámelo price | Free; your provider may charge | Free | $4.99/month |
| Account | Not required | Email/password or Google | Existing Pro license supported |
| Transcription | Your provider's limits | 2,000 words/week across devices | 60 hours per rolling 30 days |
| AI cleanup | Uses your provider key | Included with accepted transcription | Included within token allowance |
| Hosted recording limit | Not applicable | Two minutes | Ten minutes per request |
| Audio files | Supported formats and local splitting | Mono 16 kHz PCM WAV, up to two minutes | Local conversion and ten-minute chunks |
| Devices | Your provider's limits | Account-wide word counter | Up to five per license |

Free Cloud renews Monday at 00:00 UTC, displayed in your local time. The last recording is delivered in full even if it crosses 2,000 words. Cleaning that recording does not charge words again. Failed transcription uses no words; signing out or reinstalling does not reset usage. The service permits 200 transcription attempts per week and one active transcription per account. Cleanup is tied to the original transcript, with short-lived receipts and bounded retries. See [cloud configuration](docs/AUTH_AND_CLOUD.md) for safeguards.

Pro counts the preceding 30 days, with a minimum of ten seconds per transcription. The hosted service uses Whisper Large v3 Turbo and GPT-OSS 20B, with 12,000 provider requests, 3 million cleanup input tokens and 2 million completion tokens per rolling 30 days. Completion tokens include reasoning. Reaching a cleanup allowance does not remove remaining transcription time. Personal keys remain available for other workloads.

The seven-day trial stays disabled until checkout, immediate access, cancellation and expiry are verified. [Production readiness](docs/PRODUCTION_READINESS.md) lists the remaining cloud launch work. [Provider inventory](docs/INITIAL_RELEASE_REVIEW.md#small-provider-inventory) distinguishes implemented adapters from future options.

## Features

- Menu bar/system tray app with a configurable push-to-talk shortcut.
- Paste at the cursor and restore the previous clipboard, including images and files; preserve anything copied during the operation.
- Floating recording indicator; Escape cancels without transcription.
- English, Spanish, Portuguese, French, German and Italian interfaces.
- Launch at login, system sounds, custom vocabulary and optional AI cleanup.
- Audio-file transcription with local conversion and splitting for longer recordings.
- Local history with copy/delete controls and retry for failed dictation.
- Check for Updates in the tray and Mac application menu; verified automatic updates in explicitly enabled official builds.

Windows uses Win32 for keyboard/clipboard and Media Foundation for conversion. A regular process cannot paste into an administrator app; text remains on the clipboard. That decoder does not support AIFF/CAF. x64 execution in an ARM VM is emulation, not a physical Intel/AMD test.

## Public source and credentials

This is the real application used to compile the official edition. **A clean source build uses personal keys and leaves hosted services and automatic updates disabled.** Official and self-hosted builds inject public service metadata into the same source. There is no second editable copy of the app.

Free/Pro audio goes through the configured backend to Groq; personal-key mode sends it to the selected provider. Temporary audio is removed after use and history stays local. The backend records account/license usage metadata and cleanup transcript hashes, not saved audio or transcript contents. Provider retention policies still apply. Supabase Auth manages identity.

API keys, account sessions and Pro licenses use macOS Keychain or Windows Credential Manager. The Mac implementation caches runtime credentials, performs noninteractive access and separates development credentials. An inaccessible old credential asks for re-entry in the app. Keys are not stored in unencrypted SQLite or JSON. See [local credentials](docs/LOCAL_CREDENTIALS.md).

Supabase URLs, anon/publishable keys, client IDs and checkout identifiers are public configuration. Provider keys, Google client secrets, SMTP passwords, service-role keys and signing secrets belong outside the source and app bundle. A private operations wrapper can pin this repository as a submodule; it is optional. See [auth and cloud configuration](docs/AUTH_AND_CLOUD.md).

## Development

Install Rust stable, Node.js 20+, Python 3 and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode Command Line Tools. Windows requires MSVC, Windows SDK, Clang and NASM as documented in the [release guide](docs/RELEASING.md). Backend checks require Deno.

```sh
npm ci
npm run dev                         # personal keys; cloud disabled by default
npm test
npm run test:backend
npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The committed [English speech fixture](tests/fixtures/README.md) has an open redistribution license and an official reference transcript. Release build scripts run offline regression checks before compiling/signing. Live provider checks are explicit and require test credentials; never place credentials in the repository.

For a cloud build, prepare the ignored `.env.cloud-build` file from `.env.cloud-build.example`, then run:

```sh
python3 scripts/with-cloud-config.py --config .env.cloud-build -- npm run dev
```

The wrapper accepts only public build metadata and injects it at compile time. Changing configuration requires rebuilding. Do not source a general production `.env` into the desktop build. Enable updates only for the official distribution or after configuring your own endpoint and public verification key.

For a browser-only UI preview, run `python3 -m http.server 4179 --directory ui`. Mock login does not create an account or prove native authentication or audio behavior.

## Project structure

```text
ui/                          Interface, onboarding, translations and browser mocks
src-tauri/src/account.rs      Password/Google authentication and sessions
src-tauri/src/cloud_config.rs Optional public service configuration
src-tauri/src/pipeline.rs     Record → transcribe → clean → paste
src-tauri/src/platform/       macOS and Windows integration
src-tauri/src/transcription/  Provider and hosted transcription adapters
src-tauri/src/cleanup/        Text cleanup adapters
src-tauri/src/secrets.rs      OS credential storage, tracked source code
supabase/functions/          Hosted transcription, cleanup and usage
supabase/migrations/         Server-owned schemas and quotas
tests/fixtures/              Licensed speech and expected transcript
scripts/                     Checks, configuration, building and release tools
docs/                        Setup, operations and verification records
```

Groq is the established provider path. OpenAI adapters are implemented but require real-key verification before claiming equivalent coverage. Add a provider through `TranscriptionProvider` or `TextCleaner`, register it, and verify it with the speech fixture.

## Releasing a version

Follow [docs/RELEASING.md](docs/RELEASING.md): synchronize versions and lockfiles, update README and English release notes, deploy compatible backend changes, run checks, push the reviewed source to `main`, build the same commit on each machine, sign/notarize, verify the actual installers, and publish only the complete checked set.

`AGENTS.md` records maintenance rules. `.gitignore` excludes private and generated files; release instructions remain tracked. Installers belong in **GitHub Releases**, not source commits. Public installer bytes are immutable.

## License

Application: [MIT](LICENSE). The speech fixture has its own [attribution and license](tests/fixtures/README.md).
