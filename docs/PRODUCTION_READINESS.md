# Production readiness

## Current Mac development work — September 7, 2026

Version 0.6.0 adds local speech models and more personal-key providers. Its signed/notarized Mac build has passed the recorded native model, file, shutdown/restart and first-settings checks before Windows work begins. It does not replace the published 0.5.1 installers or updater manifest. See the current [verification record](TESTING.md) and [local model guide](LOCAL_MODELS.md).

The new OpenAI, Mistral and Deepgram adapters have offline request/response contract coverage. Live paid-provider calls still require suitable test keys; protocol tests must not be reported as live provider verification. Local-model speed and accuracy measurements come from an Apple M4 Max, not every supported Mac. Canary's tested Spanish phrase loses two `ñ` characters with both Q8 and F16 weights; Parakeet and Whisper transcribe those words correctly. Parakeet is the recommended local default. Model recognition is not corrected by secretly sending local text to a cloud service.

The historical release and service review below remains scoped to 0.5.1. Cloud plans, billing entitlements, public policy pages and hosted deployment are unchanged by this Mac-first work. Before distributing 0.6.0 publicly, synchronize policy/provider descriptions, complete installed Windows verification, and publish a complete new set of immutable artifacts.

## Published release review

Last reviewed: September 6, 2026, for the 0.5.1 distribution and website. Version 0.5.1 changes the Mac installer layout and version; application and backend behavior remain the same as 0.5.0. The release review checked builds, installed artifacts, public downloads and updating. Resend SMTP and Google branding/audience were separately verified earlier on September 6. CAPTCHA and checkout dashboard settings below were last checked on September 5; they have not been revalidated in those dashboards today. A working local application, a public installer and a production cloud service are separate delivery milestones. Keep exact commit, artifact and native execution evidence in [Testing](TESTING.md).

## Remaining service and verification work

The [0.5.1 official release](https://github.com/sarrazola/dictamelo/releases/tag/v0.5.1) is GitHub Latest, with one set of architecture-specific README links. All nine current assets were downloaded anonymously and matched the staged bytes and GitHub digests; all eight checksum entries, three updater signatures and the public Mac DMG/contained app's signing, stapling and Gatekeeper checks passed. The extracted public Mac updater archive passed the same app checks. The exact-version and Latest URLs return the identical 0.5.1 manifest with all three platforms. Real installed 0.5.0 → 0.5.1 updates passed on Mac, Windows ARM64 and x64 under ARM emulation, preserving settings and credentials. Installed Windows ARM64 and x64-under-emulation file/paste checks also passed; see the [Windows report](WINDOWS_BUILD_REPORT.md) for exact boundaries.

The website, six-language policies, official logo and 0.5.1 download links are public. All six homepages link to the three verified installers. Google domain ownership and branding are complete: Search Console confirms the Megacubos account is a verified owner, Google says the verified branding is shown to users, and OAuth remains External / In production. The installed 0.5.0 owned-account Google login, restart persistence and Free Cloud audio flow passed on September 6 before the final branding confirmation. The existing account persisted through the 0.5.1 update; a fresh Google login was not repeated afterward.

1. Add tested signup abuse controls and provider spend alerts before broad signup. Production Resend SMTP, real Inbox delivery and public-API confirmation/password recovery passed on September 6.
2. Test purchase, activation, cancellation/expiry and existing-license compatibility. Bind new hosted Pro requests to their activated device instance through a compatible client/server transition.
3. Review the price/180-hour subsidy and assign an operating budget. Test the published support/deletion process, document internal retention procedures and verify database recovery ownership.
4. Complete physical microphone/hardware checks. Installer upgrades and ARM64/x64-emulated runtime tests passed; actual installed 0.5.0 → 0.5.1 updating passed on Mac and both Windows architectures (x64 under ARM emulation). The Windows report distinguishes the clean x64 native-UI repetition from an earlier assisted attempt that paused on a file-write lock.

The seven-day trial can stay disabled for the initial launch. Additional model providers, a private wrapper repository and a repository-history reset are not prerequisites. Windows Authenticode and physical Intel/AMD microphone tests remain separate trust and hardware-validation improvements; updater signatures and VM emulation do not replace them.

Windows follow-up: launching a second copy currently permits two processes, with a reported shortcut conflict and fallback. Add a single-instance guard that focuses the existing Settings window. The normal one-instance startup/restart checks passed.

## Release requirements

| Area | Production evidence | Current boundary |
| --- | --- | --- |
| Desktop | Same reviewed source; installed Mac, Windows x64 and ARM64; first-run/Skip behavior; real file upload and cleanup; settings survive installation | Installed Mac 0.5.1 passed native file-picker import, Free Cloud cleanup, TXT export, fresh-settings Skip/restart and settings persistence. Windows ARM64 passed file/cleanup/WMA/paste, and x64 transcribed/pasted under ARM emulation. Original settings were preserved/restored as documented; the VM ends on ARM64 0.5.1. Physical dictation and Windows account/Pro flows remain separate. See Testing and the Windows report for exact boundaries. |
| macOS distribution | Developer ID signature, Apple Accepted notarization, stapled app/DMG, Gatekeeper and independently verified updater archive | The 0.5.1 app and DMG were accepted by Apple, stapled and checked by Gatekeeper. Public downloads and the extracted updater app passed the same checks. The real installed 0.5.0 → 0.5.1 native-menu update completed, restarted into the expected executable and preserved settings, the Google account and usage. |
| Windows distribution | Both installed payload architectures/version verified; native x64 CI; ARM64 and x64 VM functional checks; updater signatures | Both original 0.5.1 installers were installed and their payloads matched CI hashes, PE targets and versions. Installed 0.5.0 → 0.5.1 updating passed on ARM64 through the app's diagnostic hook and on emulated x64 through the native About UI, using actual NSIS installation. An earlier x64 diagnostic attempt required Retry after a file-write lock; its cause was not established, and the subsequent normal UI run passed without assistance. Existing installers have no Microsoft Authenticode certificate. Tauri update signing does not suppress SmartScreen. Build, emulated execution and physical hardware checks must remain distinct. |
| Free Cloud | 30 minutes per UTC week, included receipt-bound cleanup, exact time metering, user isolation, concurrency/replay protection and quota boundary | Audio-time migration and handlers are deployed. Real fixture used 5.855 seconds once with zero additional cleanup audio; last accepted recording reached 1804.855/1800 seconds and the next request returned 429. Temporary accounts and dependent records were removed. |
| Google account | Verified homepage/privacy/terms on the owned domain; configured audience; browser-to-installed-app sign-in and restart persistence | September 6 (September 7 UTC): Search Console confirms Megacubos is a verified owner of the dictamelo.com domain property. Google confirms branding is verified and shown to users; audience was rechecked as External / In production. Official name/logo, homepage, privacy, terms and domains are saved. Only three non-sensitive identity scopes are declared; live Supabase redirect requests email/profile. Installed 0.5.0 owned-account login, native callback and full-restart persistence passed before the final branding confirmation and were not repeated afterward. |
| Email account and signup abuse | Verified sender domain/SMTP; real confirmation and recovery delivered to an owned mailbox; login/refresh; tested signup abuse controls | September 6: Resend custom SMTP enabled using the verified domain and a domain-restricted sending key. Both confirmation and recovery arrived in Inbox from Dictámelo <no-reply@dictamelo.com>, with SPF/DKIM passing. Actual received codes passed confirmation, replay rejection, recovery and old/new password checks through the public Auth API; the temporary account was removed. Native email/password UI is a separate check. CAPTCHA was last observed off; signup abuse controls and spend alerts remain pending. |
| Pro | Correct store/product/variant, actual purchase or test checkout, immediate access, cancellation/expiry, compatible device enforcement and quota behavior | Ownership and 180-hour rolling quota service are deployed. Existing licenses remain supported. A fresh paid provider/lifecycle test remains outstanding. Normal activation limits five devices, but legacy key-only API requests do not enforce the instance/device cap against modified clients. |
| Product information | App, website and checkout display 30 minutes/week and 180 hours/rolling 30 days; policies match actual data flow | September 6: Vercel production deployment `dpl_B3an515qggXUbhq9hhZapoxyXRA4` from website commit `89666c3` is READY. All six homepages and 12 legal pages, ten assets, aliases, sitemap, robots, canonical/hreflang URLs and 404 behavior passed public HTTP checks. Downloads point to official 0.5.1 assets. Mac CTAs download the DMG immediately; Windows offers Intel/AMD and ARM64; unidentified systems retain all three options. Production browser clicks verified each route, no page errors, and the Mac download hash against the published checksum. Each installer URL returned anonymous HTTP 200. Policies retain the verified identity, audio/provider, local history, usage, billing and manual deletion/support flows. September 5: the saved/reloaded Lemon description confirmed 180 hours per rolling 30 days, $4.99/month and trial off. Checkout product images still need a consistency review. |
| Trial | Verified immediate entitlement and the complete seven-day trial lifecycle | Checkout and desktop trial flag remain false. No trial availability is claimed. |
| Public downloads | All architecture links, checksums and signatures re-downloaded and verified; actual update installation | September 6: 0.5.1 is the official Latest release. All nine current public assets were downloaded anonymously and passed byte-count/digest/checksum/signature verification. The exact-version and Latest manifests match and include all three platforms. README and website links target those installers. Installed 0.5.0 → 0.5.1 updating passed on Mac and both Windows architectures, with x64 under ARM emulation; per-attempt Windows results are recorded separately. |

## Cost of the 180-hour allowance

The selected allowance remains **180 hours per rolling 30 days at $4.99/month**. It has a negative margin at full usage even before hosting, email, support or Free Cloud users. Keep this explicit when assessing launch readiness; do not describe the earlier 60-hour planning scenario as current economics.

Official rates checked on September 5, 2026: Groq Whisper Large v3 Turbo costs $0.04/hour and Large v3 costs $0.111/hour, with a ten-second minimum billed per request. Hosted transcription remains Turbo; the personal-key wizard recommends Large v3 and does not change the hosted model. [Groq speech pricing](https://console.groq.com/docs/speech-to-text).

Lemon Squeezy's standard fee is 5% + $0.50 plus 0.5% for subscriptions. This simplified calculation excludes international, payout, PayPal, affiliate and other possible fees, and assumes no added sales tax in the fee base. [Lemon Squeezy fees](https://docs.lemonsqueezy.com/help/getting-started/fees).

| Monthly full-allowance scenario | USD |
| --- | ---: |
| Selling price | 4.99000 |
| Basic platform and subscription fee: $0.50 + 5.5% × $4.99 | -0.77445 |
| Available before other fees and operating costs | 4.21555 |
| 180 billable hours of hosted Turbo | -7.20000 |
| Margin before cleanup and other costs | **-2.98445** |
| Maximum 9M input + 6M output cleanup tokens | -2.47500 |
| Margin with the full cleanup allowance, before other costs | **-5.45945** |

Cleanup uses GPT-OSS 20B at $0.075/million input and $0.30/million output tokens; output accounting includes reasoning. The token calculation is an allowance scenario, not measured customer consumption. [Groq model pricing](https://console.groq.com/docs/models). At the same 180-hour allowance, Large v3 transcription alone would cost $19.98. No hosted model or selling-price change is implied by this calculation.

The public Free meter uses actual validated PCM duration; its 1,000-attempt weekly safeguard separately bounds request overhead. Provider billing still has its minimum, so thirty minutes of displayed audio is not a strict thirty-minute provider-cost ceiling. Pro retains ten-second minimum time accounting. Compatible compressed Pro uploads cannot be reliably timed before inference; an oversized legacy upload can exceed its reservation before being rejected and recorded. These limits and uncertain provider failures mean the table is not an absolute bound on every possible provider charge.

Operate the chosen price/allowance as a subsidy that depends on actual average usage and a funded spend budget. Review per-plan provider cost, retries, active-account growth and alerts before expanding public acquisition. Do not silently remove existing entitlements to repair the economics.

## Data and operational requirements

The MIT public repository remains the real application and reusable backend source. Public endpoints, client identifiers and the Supabase anon/publishable key are build configuration. Provider, Google-client, SMTP, service-role, Lemon management and signing secrets stay in server or OS secure storage. An optional private operations wrapper may pin this repository as a submodule; it is not required for credential security and must not become a second editable application. See [Auth and cloud configuration](AUTH_AND_CLOUD.md#public-source-and-private-credentials).

Free cleanup requires a server-issued receipt bound to the authenticated user's completed transcript. Cleanup adds no audio charge. Receipts expire after 24 hours, allow at most two reserved attempts and reject reuse after success. Separate weekly safeguards allow 250,000 input and 250,000 completion tokens. The final accepted recording is delivered whole, with at most one two-minute recording of overage; its receipt remains eligible for cleanup. Tables retain hashes and accounting metadata, not transcripts. A hash is not encrypted transcript storage, and receipt expiry is not a retention/deletion policy.

Live checks found all seven billing/usage tables protected by RLS with no anon/authenticated grants, and all twelve quota RPCs restricted to service access. The public-history scan found no detected privileged provider, server or signing credentials in the reviewed history; the historical anon JWT was expected public configuration. This targeted result is not a claim of zero vulnerabilities. A modified authorized client can use its server-enforced allowance; keeping client source private would not make bundled configuration secret.

Before public signup, verify an account-creation abuse control with its desktop flow and provider spend alerts. CAPTCHA is currently off; enabling it without client support can break authentication. Close the Pro instance-header gap through a compatible client/server transition and test released clients before requiring the header. See [Pro activation compatibility](AUTH_AND_CLOUD.md#pro-activation-compatibility-boundary).

Assign operators for provider failures, quota/cost alerts, deletion requests and billing support. Keep user audio/transcript content, provider error bodies and credentials out of routine logs. Maintain database recovery access and test restoration separately. The published [privacy policy](https://www.dictamelo.com/en/privacy) and [terms](https://www.dictamelo.com/en/terms) describe actual identity fields, account/usage records, local history, microphone processing, Supabase, Groq, personal-key providers, Lemon Squeezy, retention, deletion and contact procedures. Keep them synchronized with service changes and test the support/deletion process separately. Google identity scopes do not grant Gmail inbox access.

## Repeatable checks

Use the committed licensed speech fixture and offline regression gate required by both release scripts. Run the explicit live test using disposable identities and verify their removal, then test the actual installed app's file picker/queue and dictation. Offline tests, browser mocks, hosted API calls, native UI actions, VM emulation and physical hardware are separate evidence categories.

Do not mark unavailable credentials, a skipped mailbox check, an inaccessible VM or untested payment transitions as passing. Follow [Releasing](RELEASING.md), preserve existing updater platform entries, upload immutable artifacts before the complete manifest, and publish a new version whenever released installer bytes change.

## External documentation

- [Google OAuth branding requirements](https://support.google.com/cloud/answer/15549049?hl=en)
- [Google audience and identity-only Testing exception](https://support.google.com/cloud/answer/15549945)
- [Supabase production SMTP](https://supabase.com/docs/guides/auth/auth-smtp)
- [Supabase Google login configuration](https://supabase.com/docs/guides/auth/social-login/auth-google)
- [Supabase CAPTCHA integration](https://supabase.com/docs/guides/auth/auth-captcha)
- [Lemon Squeezy trial setup](https://docs.lemonsqueezy.com/help/products/free-trials)
