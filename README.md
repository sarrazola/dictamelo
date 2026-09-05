<p align="center"><img src="assets/logo-original.png" width="128" alt="Dictámelo"></p>

# Dictámelo

Voice dictation for any app on **macOS and Windows**. Hold a shortcut, speak, and release: the text appears at your cursor. Built with Rust and Tauri 2, with a small HTML/CSS/JavaScript interface.

[Latest public release](https://github.com/sarrazola/dictamelo/releases/latest) · [Release guide](docs/RELEASING.md) · [Testing](docs/TESTING.md)

**0.3.1 is a built and locally installed Mac preview.** The final app and DMG are signed, notarized and stapled. Google sign-in and restart persistence passed without a Keychain prompt, and the native application-menu update action was verified. Google remains restricted to permitted test users; production email delivery and the proposed Pro trial are pending. See [preview notes](docs/releases/0.3.1.md). The public release remains **0.1.2**; the verified **0.2.0** installers remain in draft. This iteration does not replace those assets or produce new Windows installers.

## Download and install

Use the [latest public release](https://github.com/sarrazola/dictamelo/releases/latest) for publicly available installers. It currently includes Apple Silicon and Windows ARM64. Windows Intel/AMD support was built and checked for the separate 0.2.0 draft; see its [release status](docs/releases/0.2.0-status.md).

| Computer | Installer naming | Current distribution |
| --- | --- | --- |
| Apple Silicon Mac, macOS 12 or later | `Dictamelo_<version>_aarch64.dmg` | 0.1.2 public; 0.2.0 draft; 0.3.1 verified local preview |
| Windows 10/11, Intel or AMD 64-bit | `Dictamelo_<version>_x86_64-setup.exe` | 0.2.0 draft; no new build in this iteration |
| Windows 11 on ARM | `Dictamelo_<version>_aarch64-setup.exe` | 0.1.2 public; 0.2.0 draft |

On macOS, open the DMG and drag Dictámelo into Applications. Allow Microphone and Accessibility when requested. On Windows, run the installer and allow desktop microphone access in Windows Settings. Intel Macs, 32-bit Windows and Linux installers are not provided.

The final 0.3.1 Mac preview passed Developer ID, Apple notarization, stapling and Gatekeeper checks. Its installed executable matches the app in the read-only DMG, and the signed updater archive was verified independently. Local previews are not public GitHub releases. Historical 0.1.x Mac installers were signed but not notarized. Windows updater signatures are separate from Microsoft Authenticode; the existing Windows installers have no Authenticode certificate. See the [verification record](docs/TESTING.md) for exact results.

## Start dictating in the preview

1. Open **Onboarding** to choose your own API keys, Free Cloud or Pro. The button is deliberately visible while the wizard is being tested; reopening it lets you review your choices.
2. For personal keys, select Groq or OpenAI in **Models** and enter your key. No Dictámelo account is needed.
3. In a cloud-configured build, **Create free account** uses email/password, with email confirmation; existing users can sign in with their password. **Continue with Google** opens the system browser. Installed 0.3.1 passed Google sign-in and restart persistence with an allowed test user; Google's audience is still in Testing. Password/confirmation/recovery API checks passed separately, but actual mailbox delivery remains pending.
4. Review language, shortcut and permissions. Hold **Alt/Option + Shift + Space**, speak, and release to paste.
5. Free Cloud usage appears in **Plan**, with words used, words remaining and the next renewal time. Existing Pro customers can still activate their Lemon Squeezy license.

## Three plans

| | Free — your own keys | Free Cloud | Pro |
| --- | --- | --- | --- |
| Dictámelo price | Free; your provider may charge | Free | $4.99/month |
| Account | Not required | Email/password or Google | Existing Pro license supported |
| Transcription allowance | Your provider's limits | 2,000 words/week across devices | 60 hours per rolling 30 days |
| Hosted recording limit | Not applicable | Two minutes | Ten minutes per request |
| Audio files | Supported formats and local splitting | Mono 16 kHz PCM WAV, up to two minutes | Local conversion and ten-minute chunks in the Mac preview |
| Text cleanup | Uses your provider key | Not included | Included within token allowance |

**Free Cloud:** words renew Monday at 00:00 UTC, shown in your local time. The last recording is delivered in full even if it crosses 2,000 words; further recordings wait for renewal. A 200-request weekly safeguard and one active request per account limit abuse. Failed provider requests use no words. Signing out or reinstalling does not reset usage.

**Pro:** the rolling window counts the last 30 days, not a calendar-month reset. Each transcription counts at least ten seconds. The hosted service uses Whisper Large v3 Turbo and GPT-OSS 20B, with 12,000 total provider requests, 3 million cleanup input tokens and 2 million completion tokens per rolling 30 days. Completion tokens include reasoning. Cleanup exhaustion does not remove the remaining transcription allowance. Larger or different-model workloads can use personal keys. These are explicit limits, not an unlimited plan.

The updated hosted Pro quota service is deployed, with live quota and access-control checks recorded in [Testing](docs/TESTING.md).

A seven-day trial is being evaluated. **It is not advertised as available:** the preview's trial flag remains off until checkout, immediate access, cancellation and expiry have been verified. See [the initial release review](docs/INITIAL_RELEASE_REVIEW.md) for provider comparisons and pricing assumptions.

## Features

- Menu bar/system tray app with a configurable push-to-talk shortcut.
- Paste at the cursor and restore the previous clipboard, including images and files; preserve anything the user copies during the operation.
- Floating recording indicator that does not take focus; Escape cancels without transcription.
- Six interface languages: English, Spanish, Portuguese, French, German and Italian.
- Optional launch at login, system sounds, custom vocabulary and AI cleanup.
- Audio-file transcription with local conversion and splitting for long recordings.
- Local history with copy/delete controls and retry for failed dictation.
- Signed automatic updates from GitHub Releases in explicitly enabled official builds. Default source builds leave updates disabled; a local preview does not publish an update.

Windows uses Win32 for keyboard/clipboard and Media Foundation for conversion. A regular process cannot paste into an elevated administrator app; text remains on the clipboard. AIFF/CAF decoding is not available through that decoder. ARM VM testing of x64 is emulation, not testing on physical Intel/AMD hardware. This preview has no new Windows verification.

## Public source and credentials

This repository contains the real application used to build the official edition. **A clean source build uses personal keys and leaves hosted services and automatic updates disabled.** Official or self-hosted builds inject public service metadata using the same source; there is no second editable copy of the app. See [Auth and cloud configuration](docs/AUTH_AND_CLOUD.md).

Free/Pro audio travels through the configured backend to Groq. Personal-key mode sends it to the selected provider. Temporary audio is removed after use and history stays local. The backend records account/license usage metadata, not audio or transcript contents. Supabase Auth manages the account. Provider retention policies still apply.

Session tokens, license keys and personal keys use macOS Keychain or Windows Credential Manager. The 0.3.1 Mac implementation caches runtime credentials and performs noninteractive Keychain access; debug builds use a separate namespace. An inaccessible old credential asks for re-entry in the app. Keys are not moved into unencrypted SQLite or JSON. See [Local credentials](docs/LOCAL_CREDENTIALS.md) for migration behavior.

Public Supabase URLs, anon/publishable keys, Google client IDs and checkout identifiers are not provider secrets. Google client secrets, SMTP credentials, service-role keys and hosted provider keys belong on the server, never in app bundles or source control. A private deployment repository can be useful for operations, but is not required to make public login code safe.

## Development

Install current stable Rust, Node.js 20+ and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode Command Line Tools. Windows build prerequisites and release commands remain in the release guide for a later cross-platform release.

```sh
npm ci
npm run dev                         # personal keys; cloud disabled by default
npm test
npm run test:backend                # requires Deno
npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

For an explicitly configured cloud build, prepare an ignored `.env.cloud-build` file containing only public build metadata, then run:

```sh
python3 scripts/with-cloud-config.py --config .env.cloud-build -- npm run dev
```

The wrapper injects compile-time configuration; changing the file requires a rebuild. `DICTAMELO_UPDATES_ENABLED` defaults to `false`; enable it only for an official build or after configuring your own updater endpoint and verification key. Do not source a general production `.env` into the desktop build. The [configuration guide](docs/AUTH_AND_CLOUD.md) lists the exact allowed fields and server-only secrets.

For a UI-only preview with mock data, run `python3 -m http.server 4179 --directory ui` and open `http://localhost:4179`. Browser mock login does not create a real account or prove email, Google or microphone behavior.

## Project structure

```text
ui/                          Interface, onboarding, translations and browser mocks
src-tauri/src/account.rs      Account creation, password/Google login, session storage
src-tauri/src/cloud_config.rs Optional public hosted-service build configuration
src-tauri/src/pipeline.rs     Record → transcribe → clean → paste
src-tauri/src/platform/       macOS and Windows integration
src-tauri/src/transcription/  Groq/OpenAI and hosted transcription
src-tauri/src/cleanup/        Optional text cleanup providers
src-tauri/src/license.rs      Existing Pro license activation
src-tauri/src/secrets.rs      OS credential store (tracked source code)
supabase/functions/          Hosted transcription, cleanup and usage
supabase/migrations/         Server-owned schemas and quotas
scripts/                     Configuration, building, signing and release tools
docs/                        Configuration, release and verification guides
```

To add a provider, implement `TranscriptionProvider` or `TextCleaner` and register it in the corresponding registry. Keep platform-specific changes under `platform/`. Groq is the established provider path; OpenAI already has an adapter but must be tested with real credentials before claiming equivalent verification. Additional providers are a separate scoped decision.

## Releasing a new version

Follow **[docs/RELEASING.md](docs/RELEASING.md)** for versions, English documentation, backend compatibility, signing, notarization and artifact checks. The current 0.3.1 task is a local Mac preview: do not run the full publication script or change the existing release assets for it.

`AGENTS.md` carries maintenance rules for coding assistants. `.gitignore` excludes private/generated files; it does not replace release instructions. Installers belong in **GitHub Releases**, not source commits.

## License

[MIT](LICENSE).
