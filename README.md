<p align="center"><img src="assets/logo-original.png" width="128" alt="Dictámelo"></p>

# Dictámelo

Voice dictation for any app on **macOS and Windows**. Hold a shortcut, speak, and release: the text appears at your cursor. Built with Rust and Tauri 2, with a small HTML/CSS/JavaScript interface.

[Website](https://dictamelo.com) · [Latest release](https://github.com/sarrazola/dictamelo/releases/latest) · [Release guide](docs/RELEASING.md) · [Testing](docs/TESTING.md)

**0.2.0 release status:** the new free account and installers are being prepared in a draft release. Public email-code sign-in still requires production SMTP configuration. The current public release remains 0.1.2; the features below describe the 0.2.0 source. See [release status](docs/releases/0.2.0-status.md).

## Download

Choose the installer for your computer from the [latest release](https://github.com/sarrazola/dictamelo/releases/latest):

| Computer | Installer |
| --- | --- |
| Mac with Apple Silicon, macOS 12 or later | `Dictamelo_<version>_aarch64.dmg` |
| Windows 10/11 with Intel or AMD, 64-bit | `Dictamelo_<version>_x86_64-setup.exe` |
| Windows 11 on ARM, including Snapdragon | `Dictamelo_<version>_aarch64-setup.exe` |

The macOS release process requires **Developer ID signing, Apple notarization, and stapling** for both the app and the DMG. Windows installers are signed for the Tauri updater; this is separate from Microsoft Authenticode. Windows may show a SmartScreen warning while the installer has no Authenticode certificate/reputation. Windows 32-bit, older Windows versions, Intel Macs, and Linux are not included in the published installers.

On macOS, open the DMG and drag Dictámelo into Applications. Allow Microphone and Accessibility when requested. On Windows, run the installer; it installs for your user and can install WebView2 if needed. Allow desktop microphone access in Windows Settings.

## Start dictating

1. Open **Plan**, enter your email, and enter the verification code you receive.
2. Your free account includes **2,000 words each week**, shared across all your computers.
3. Hold **Alt/Option + Shift + Space**, speak, and release to paste.
4. Check **Plan → Account & weekly usage** for words used, words remaining, and the next renewal date.

Already using a personal Groq or OpenAI key? Keep it in **Models**, and choose **Use my own API key** in Plan. Your provider bills that usage; it does not consume the included free allowance.

## Plans

| | Free account | Pro license | Your own API key |
| --- | --- | --- | --- |
| Included transcription | 2,000 words/week | Up to 20 audio hours per rolling 30 days | Billed by your provider |
| Sign-in | Email verification code | Existing Lemon Squeezy license | Not required |
| Dictation | Up to two minutes per recording | Configurable recording duration | Configurable recording duration |
| Audio files | Mono 16 kHz PCM WAV, up to two minutes | Supported formats and long files | Supported formats and long files |
| AI cleanup | — | Included | Uses your provider key |
| Usage storage | Account-level weekly word count | License-level audio duration | Local history |

Free words renew **Monday at 00:00 UTC**. The app displays that moment in your local time. The last recording is delivered in full even if it crosses 2,000 words; further recordings are blocked until renewal. A 200-request weekly safeguard and one request at a time per account limit abuse. Failed provider requests consume no words. Reinstalling or signing out does not reset usage.

Pro activation is retained for existing licenses. The purchase product must be published and `CHECKOUT_URL` updated to its actual checkout before advertising Pro sales; the repository does not publish or alter the Lemon Squeezy product.

## Features

- Menu bar/system tray app with a configurable push-to-talk shortcut.
- Paste at the cursor and restore the previous clipboard, including images and files; preserve anything the user copies during the operation.
- Floating recording indicator that does not take focus; Escape cancels without transcription.
- Six interface languages: English, Spanish, Portuguese, French, German, and Italian.
- Optional launch at login, system sounds, custom vocabulary, and AI cleanup.
- Audio-file transcription with local conversion and silence-aware splitting for long recordings (Pro/personal key).
- Small local history with copy/delete controls and retry for failed dictation.
- Signed automatic updates from GitHub Releases, checked at startup and every six hours.

Windows uses Win32 for keyboard/clipboard and Media Foundation for audio conversion. Windows cannot paste into an elevated administrator app from a regular process; the text remains on the clipboard. AIFF/CAF decoding is not available through the Windows decoder. ARM VM testing of an x64 executable is emulation, not testing on physical Intel/AMD hardware.

## Privacy and credentials

Free/Pro audio passes through Supabase Edge Functions to Groq. Personal-key mode sends it directly to the selected provider. The app removes temporary audio after use and stores history locally. The backend stores usage and license metadata, not audio or transcript contents. Supabase Auth stores the account email. Provider retention policies still apply.

Session tokens, license keys, and personal provider keys use macOS Keychain or Windows Credential Manager. The provider's server key is an Edge Function secret and is never bundled in the app. `src-tauri/src/supabase-public-key.txt` is intentionally public: it contains only the client `anon` key; all quota tables and mutation functions are restricted to the server's service role.

## Development

Install current stable Rust, Node.js 20+, and the platform prerequisites in [Tauri's setup guide](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode Command Line Tools. Windows needs Visual Studio 2022 C++ Build Tools, a Windows SDK, WebView2, NASM for x64, and Clang for ARM64.

```sh
npm ci
npm run dev
npm test
npm run test:backend                 # requires Deno
npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

For a UI-only preview (mock data, no real account or audio), run `python3 -m http.server 4179 --directory ui` and open `http://localhost:4179`.

## Project structure

```text
ui/                         Interface, translations, and browser preview mocks
src-tauri/src/account.rs     Email-code login, session refresh, secure session storage
src-tauri/src/pipeline.rs    Record → transcribe → clean → paste
src-tauri/src/platform/      macOS and Windows system integration
src-tauri/src/transcription/ Provider interface, Groq/OpenAI and hosted transcription
src-tauri/src/cleanup/       Optional text cleanup providers
src-tauri/src/license.rs     Existing Pro license activation
src-tauri/src/updates.rs     Signed updater and release signature verification
src-tauri/src/secrets.rs     OS credential-store implementation (tracked source code)
supabase/functions/         Hosted transcription, cleanup, account usage
supabase/migrations/        Database schema and server-only quota functions
supabase/templates/         Sign-in email template
scripts/                    Build, notarization, publishing, and diagnostics
docs/                       Release instructions and verification records
```

To add a transcription provider, implement `TranscriptionProvider` and register it in `ProviderRegistry::with_defaults()`. To add a cleaner, implement `TextCleaner` and register it in `CleanerRegistry::with_defaults()`. Keep platform-specific changes under `platform/`.

## Releasing a new version

Follow **[docs/RELEASING.md](docs/RELEASING.md)**. It covers version bumps, README/changelog updates, backend deployment, Windows x64/ARM64 builds, macOS notarization, updater signatures, GitHub Releases, and verification of the publicly downloaded installers.

`AGENTS.md` carries these maintenance rules for coding assistants. `.gitignore` only controls which files Git tracks; it is not the place for release instructions. Installers belong in **GitHub Releases**, not in source commits.

## License

[MIT](LICENSE).
