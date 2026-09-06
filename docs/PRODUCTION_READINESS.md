# Production readiness

Last reviewed: September 6, 2026, for the 0.5.0 source and deployed audio-time backend. The current review checked source, GitHub releases/updater and public website URLs. Google, SMTP, CAPTCHA and checkout dashboard settings below were last checked on September 5; they have not been revalidated in those dashboards today. A working local application, a public installer and a production cloud service are separate delivery milestones. Keep exact commit, artifact and native execution evidence in [Testing](TESTING.md).

## Remaining launch inventory

1. Complete distribution verification and publish the 0.5.0 candidate with architecture-specific README links. Installed Windows ARM64 and x64-under-emulation checks now passed, including files, cleanup, paste and restored settings. See the [Windows report](WINDOWS_BUILD_REPORT.md) for exact boundaries.
2. Before stable promotion, publish the complete three-platform update manifest and verify an actual version-to-version update. The old stable 0.1.2 manifest lacks x64, causing the x64 update check to fail; it does not offer a downgrade.
3. Restore the public homepage, privacy and terms pages and finish Google branding. The installed 0.5.0 owned-account Google login, restart persistence and Free Cloud audio flow passed on September 6.
4. Configure production SMTP and verify real confirmation and password-recovery delivery. Add tested signup abuse controls and provider spend alerts before broad signup.
5. Test purchase, activation, cancellation/expiry and existing-license compatibility. Bind new hosted Pro requests to their activated device instance through a compatible client/server transition.
6. Review the price/180-hour subsidy and assign an operating budget. Publish real support, retention and deletion procedures and verify database recovery ownership.

The seven-day trial can stay disabled for the initial launch. Additional model providers, a private wrapper repository and a repository-history reset are not prerequisites. Windows Authenticode and physical Intel/AMD microphone tests remain separate trust and hardware-validation improvements; updater signatures and VM emulation do not replace them.

Small UI follow-up found during native testing: the Free usage progress control still has the accessible label `Weekly words used`, although its value now measures audio seconds. Visible minute totals are correct; update and translate the accessible label in the next UI build.

Windows follow-up: launching a second copy currently permits two processes, with a reported shortcut conflict and fallback. Add a single-instance guard that focuses the existing Settings window. The normal one-instance startup/restart checks passed.

## Release requirements

| Area | Required evidence before stable cloud launch | Current boundary |
| --- | --- | --- |
| Desktop | Same reviewed source; installed Mac, Windows x64 and ARM64; first-run/Skip behavior; real file upload and cleanup; settings survive installation | Installed Mac and Windows ARM64 passed file/cleanup/copy, fresh-settings setup/Skip and restart persistence. Windows x64 installed and transcribed/pasted under ARM emulation. Original settings were restored and the VM was left on ARM64 0.5.0. Physical dictation and Windows account/Pro flows remain separate. See Testing and the Windows report for exact boundaries. |
| macOS distribution | Developer ID signature, Apple Accepted notarization, stapled app/DMG, Gatekeeper and independently verified updater archive | The 0.5.0 app and DMG were accepted by Apple, stapled and checked by Gatekeeper. The installed executable matches the read-only DMG; complete public distribution and updater installation remain separate checks. |
| Windows distribution | Both installed payload architectures/version verified; native x64 CI; ARM64 and x64 VM functional checks; updater signatures | Existing installers have no Microsoft Authenticode certificate. Tauri update signing does not suppress SmartScreen. Build, emulated execution and physical hardware checks must remain distinct. |
| Free Cloud | 30 minutes per UTC week, included receipt-bound cleanup, exact time metering, user isolation, concurrency/replay protection and quota boundary | Audio-time migration and handlers are deployed. Real fixture used 5.855 seconds once with zero additional cleanup audio; last accepted recording reached 1804.855/1800 seconds and the next request returned 429. Temporary accounts and dependent records were removed. |
| Google account | Verified homepage/privacy/terms on the owned domain; configured audience; browser-to-installed-app sign-in and restart persistence | Last dashboard check: Megacubos Google project and Supabase provider configured, audience Testing. Installed 0.5.0 owned-account login, native callback and full-restart persistence passed on September 6. Identity-only Google login is exempt from the Testing allowlist/warning/seven-day restrictions; Testing alone does not prove public login is blocked. Public website and branding remain pending. |
| Email account and signup abuse | Verified sender domain/SMTP; real confirmation and recovery delivered to an owned mailbox; login/refresh; tested signup abuse controls | Password/Auth API tests passed. Last dashboard check: confirmations enabled, SMTP host absent and CAPTCHA off. Synthetic tokens do not prove email delivery; per-account quotas do not prevent multiple-account abuse. Supabase's default sender is restricted to project-team addresses and is not production SMTP. |
| Pro | Correct store/product/variant, actual purchase or test checkout, immediate access, cancellation/expiry, compatible device enforcement and quota behavior | Ownership and 180-hour rolling quota service are deployed. Existing licenses remain supported. A fresh paid provider/lifecycle test remains outstanding. Normal activation limits five devices, but legacy key-only API requests do not enforce the instance/device cap against modified clients. |
| Product information | App, website and checkout display 30 minutes/week and 180 hours/rolling 30 days; policies match actual data flow | September 6: apex homepage/privacy/terms return HTTP 308 to www. System resolution of www failed locally; DNS lookup returned its Vercel address, and requests through that observed address with TLS verification returned HTTP 404 for all three pages. September 5: the saved/reloaded Lemon description confirmed 180 hours per rolling 30 days, $4.99/month and trial off. Website and product images need a final consistency review. |
| Trial | Verified immediate entitlement and the complete seven-day trial lifecycle | Checkout and desktop trial flag remain false. No trial availability is claimed. |
| Public downloads | All architecture links, checksums and signatures re-downloaded and verified; actual update installation | September 6: 0.5.0 remains a draft with nine verified assets; public preview is 0.4.0. The configured stable updater returns 0.1.2 with Mac ARM64 and Windows ARM64 only. A prerelease does not advance this channel. Include all three platforms before stable promotion and verify a real installed update. |

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

Assign operators for provider failures, quota/cost alerts, deletion requests and billing support. Keep user audio/transcript content, provider error bodies and credentials out of routine logs. Maintain database recovery access and test restoration separately. Publish a privacy policy that describes actual identity fields, account/usage records, local history, microphone processing, Supabase, Groq, personal-key providers, Lemon Squeezy, retention, deletion and contact procedures. Google identity scopes do not grant Gmail inbox access.

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
