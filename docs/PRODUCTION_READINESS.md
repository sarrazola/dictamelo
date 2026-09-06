# Production readiness

Last reviewed: September 5, 2026. A working local application, a public installer and a production cloud service are separate delivery milestones. Keep release evidence in [TESTING.md](TESTING.md).

## Release requirements

| Area | Required evidence before stable cloud launch | Current boundary |
| --- | --- | --- |
| Desktop | Same reviewed source on `main`; Mac, Windows x64 and Windows ARM64 builds; real file upload and cleanup; settings survive installation | 0.4.0 candidate verification is in progress. Prior platform results do not establish this version. |
| macOS distribution | Developer ID signature, Apple Accepted notarization, stapled app/DMG, Gatekeeper, independently verified updater archive | 0.3.1 passed. Repeat against final 0.4.0 bytes. |
| Windows distribution | Both installed payload architectures/version verified; native x64 CI; ARM64 and x64 VM functional checks; updater signatures | Existing installers have no Microsoft Authenticode certificate. Tauri update signing does not suppress SmartScreen. Record emulation separately from physical hardware. |
| Free Cloud | 2,000 words/week; cleanup for the accepted transcript; user isolation, exact transcript binding, concurrent/replayed request protection, failures and quota boundary | New cleanup receipt flow must pass SQL, handler, native and live hosted tests before distribution. |
| Google account | Verified homepage/privacy/terms on the owned domain; configured audience; browser-to-installed-app sign-in and restart persistence | Megacubos Google project and Supabase provider are configured. Owned test-user login passed in 0.3.1; Google audience remains Testing. |
| Email account | Verified sender domain and SMTP; real confirmation and password recovery delivered to an owned mailbox; login and token refresh | Password/Auth API tests passed. Production SMTP is not configured; generated test tokens do not prove delivery. |
| Pro | Matching store/product/variant, actual purchase or test checkout, immediate license access, cancellation/expiry and quota behavior | Ownership and 60-hour quota service are deployed. A seven-day trial remains disabled until its entitlement lifecycle is proved. |
| Product information | Matching app, website and checkout limits; privacy and terms describe the actual data flow | `dictamelo.com`, `/privacy` and `/terms` still return a parking redirect. The separate website repository has a landing page but no published legal pages. Review checkout wording against the 60-hour allowance. |
| Public downloads | All architecture links, checksums and signatures re-downloaded and verified | Publish a candidate as a GitHub prerelease while cloud requirements remain open. Promote to stable only after closure and a real update installation check. |

## Data and operational requirements

The public repository is the source used for official builds. Public endpoint and client identifiers are build configuration. Provider, Google-client, SMTP, service-role, Lemon management and signing secrets stay in the service's secret storage or the OS credential store. A private wrapper may pin this repository as a submodule; it must not become a second application implementation.

Free cleanup is tied to a server-issued receipt for the authenticated user's actual transcription. Only transcription charges the weekly word counter. The receipt and transcript hash are usage metadata, not a saved transcript. Document receipt expiry and request/token safeguards in the cloud guide. Never accept an arbitrary unmetered cleanup prompt from a free account.

Before inviting public customers, assign an operator for provider errors, quota/cost alerts, account deletion requests and billing support. Record how to disable a failing hosted route and deploy a compatible fix without breaking personal-key mode. Keep provider response bodies, account tokens, user audio and transcript content out of routine logs. Maintain database recovery access and test a restore separately; a successful deployment is not a backup test.

The privacy policy must describe Google identity fields (name/email/profile image as supplied), account and usage records, local history, microphone/audio processing, Supabase, Groq, personal-key providers, Lemon Squeezy, retention and deletion/contact procedures. Google login requests identity scopes; it does not request Gmail mailbox access. Publish policies that match verified behavior and the business's actual practices.

## Repeatable checks

The committed speech fixture and offline regression gate are required by both platform release build scripts. Local checks must be possible without accounts, provider keys, network-dependent downloads or billable requests. Run the explicit live audio test before a release against a disposable test account, then verify the installed application's file picker/queue as well. A mocked response does not prove a working provider; a live backend request alone does not prove the desktop file interface.

Keep failures visible. Do not mark unavailable live credentials, an inaccessible VM, a skipped email inbox check or untested payment transitions as passing. Record the exact commit/artifact, command and result. Publish a new version if released installer bytes need to change.

## External documentation

- [Google OAuth branding requirements](https://support.google.com/cloud/answer/15549049?hl=en)
- [Supabase production SMTP](https://supabase.com/docs/guides/auth/auth-smtp)
- [Supabase Google login configuration](https://supabase.com/docs/guides/auth/social-login/auth-google)
- [Lemon Squeezy trial setup](https://docs.lemonsqueezy.com/help/products/free-trials)
