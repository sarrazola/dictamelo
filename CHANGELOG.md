# Changelog

## 0.4.0 — Release candidate

- Include hosted AI cleanup in Free Cloud without charging transcription words twice.
- Apply optional cleanup to uploaded audio as well as dictation; preserve the original result when cleanup fails.
- Add an attributed open speech fixture, transcript comparison and repeatable offline/live regression checks.
- Validate all six interface languages and referenced controls before packaging.
- Recover from native file-dialog creation failures and offer local-path audio import.
- Prepare macOS, Windows Intel/AMD x64 and Windows ARM64 from the same source on `main`.
- Add a production-readiness checklist and an explicit prerelease publishing mode that preserves the stable updater channel.

Artifact and hosted test results are recorded in `docs/TESTING.md` as they complete. This entry is not a claim of production cloud availability.

## 0.3.1 — Mac preview, not published

See [preview notes](docs/releases/0.3.1.md) and the [verification record](docs/TESTING.md).

- Cache runtime credentials and prevent interactive macOS Keychain requests. Add a separate runtime namespace, silent legacy migration, deletion markers and debug-build isolation while retaining OS-backed storage.
- Configure Google OAuth through Supabase and verify sign-in, account usage and session persistence in an installed candidate. Google remains limited to permitted test users until production branding and website requirements are completed.
- Add **Check for Updates…** to the tray and native Mac application menu, opening the existing update screen without automatically installing anything. Verify the native application-menu action in the final installed, notarized preview.
- Pass 55 Rust tests, including synthetic native Keychain persistence/update/deletion and debug isolation, plus strict Clippy. Production SMTP and the proposed seven-day trial remain pending.
- Keep this iteration Mac-only; public 0.1.2 and draft 0.2.0 assets and updater manifests remain unchanged.

## 0.3.0 — Mac preview, not published

See [preview notes](docs/releases/0.3.0.md) and the [verification record](docs/TESTING.md).

- Separate personal-key, Free Cloud and Pro choices, with a visible repeatable onboarding wizard and a minimum application window size.
- Add email/password account creation and login, Google sign-in through the system browser, confirmation and password recovery. Live Google and email-delivery verification are still pending.
- Increase hosted Pro to 60 hours per rolling 30 days, with atomic reservations, product ownership checks, request limits and bounded text-cleanup usage. Preserve existing Pro licenses.
- Build the official and personal-key editions from the same public source. Clean builds leave cloud services and automatic updates disabled; an explicit wrapper injects only public hosted-service configuration.
- Keep the proposed seven-day trial disabled pending checkout and entitlement testing.
- Build, sign, notarize, staple and locally install the Apple Silicon Mac preview. Verify the installed window minimum, plans, wizard and real file transcription. No Windows builds or existing public/draft release assets change in this iteration.

## 0.2.0 — draft

See [release notes](docs/releases/0.2.0.md): free weekly allowance, email-code accounts, usage tracking, Windows Intel/AMD support, notarized macOS release process, and English documentation.

## 0.1.2

Check for updates every six hours, preserve platform entries in the updater manifest, and publish the first Windows ARM64 installer.

## 0.1.1

Add signed automatic updates and the server-side Pro transcription/cleanup backend.

## 0.1.0

Initial Apple Silicon macOS release with push-to-talk dictation, personal provider keys, optional AI cleanup, audio-file transcription, and six interface languages. This historical release was signed but not notarized.
