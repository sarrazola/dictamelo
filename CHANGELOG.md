# Changelog

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
