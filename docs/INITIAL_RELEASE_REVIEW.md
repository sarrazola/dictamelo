# Initial release review

Updated on September 5, 2026 for application source `097551f9582fce8c17d6f4a539192d89b80236d8` (0.5.0) and the deployed audio-time backend. This review records the current product decision and remaining production work. Historical test counts and release evidence remain in [Testing](TESTING.md); they are not new verification of a 0.5.0 native installation or public release.

## Three understandable choices

| Choice | What the customer receives | Account and payment |
| --- | --- | --- |
| Free — your own API keys | Dictation and supported cleanup billed directly to the customer's provider account, with no Dictámelo-hosted allowance. | No Dictámelo account required. The provider may charge the customer. |
| Free Cloud | 30 minutes per UTC week, shared across devices, with included cleanup of those transcriptions and a visible time/renewal counter. | Confirmed email/password account or Google account; no subscription required. |
| Pro | 180 hours of hosted transcription per rolling 30 days with cleanup, preserving existing licenses and normal five-device activation. | $4.99/month. Seven-day trial remains disabled. |

Describe provider costs alongside personal-key setup. A signed-in customer can still choose personal keys; account presence and the active transcription route are separate. The public cloud allowance is time, not the previous 2,000-word limit, and Pro is not unlimited.

New users receive the short first-run wizard with a visible **Skip** action. Existing settings skip automatic onboarding and preserve provider choices. The new personal-key selector shows Groq, recommends Whisper Large v3 and offers Turbo; existing provider adapters/settings remain compatible. Hosted transcription continues to use Turbo. Keep the six interface languages and the small dictation-focused scope.

The Mac source keeps the main window available while switching apps with Cmd-Tab; closing it returns the app to tray operation. Browser and source regressions cannot establish that native behavior. Verify the installed signed artifact and permissions separately, including real dictation and file transcription.

## Deployed allowance and cleanup contract

Migration `20260906000000_audio_time_plans` and the compatible `transcribe`, `cleanup` and `usage` handlers are deployed to official Supabase project `iburiyhhfodndqgmsaot`.

| Safeguard | Free Cloud | Pro |
| --- | --- | --- |
| Audio | 1,800 measured seconds per UTC week; Monday 00:00 renewal | 648,000 seconds across the rolling last 30 days |
| Request budget | 1,000 transcription attempts/week | 36,000 transcription/cleanup requests per rolling 30 days |
| Cleanup input/output | 250,000 / 250,000 tokens per original transcription week | 9M / 6M tokens per rolling 30 days |
| Recording boundary | At most two minutes; last accepted recording delivered whole, then subsequent audio rejected until renewal | Ten-minute request bound; existing ten-second minimum charge per transcription retained |

Free time is measured from validated PCM bytes. Historical word usage is preserved once at 0.45 seconds/word; old 2,000-word usage becomes 15 minutes, while all new work uses measured audio. Successful silence also consumes audio time. One active transcription reservation prevents concurrent callers from multiplying Free's last-recording overage.

Both plans include cleanup without a second audio charge. Free cleanup uses GPT-OSS 20B and a server receipt bound to the account and exact trimmed transcript hash. Receipts expire after 24 hours, permit at most two reserved attempts and reject reuse after success. Cleanup remains available for the final accepted transcript even after audio exceeds 30 minutes. Token/output, body-size and single-active-request limits prevent the endpoint becoming an unrestricted text-generation service. The receipt tables retain hashes and accounting metadata, not raw or cleaned transcripts; provider processing and retention still apply.

Database reservations are atomic and fail closed. Explicit provider rejection can release reserved usage; uncertain failures retain conservative reservations because a timeout does not establish zero cost. Current PCM is timed before inference. Legacy compressed Pro uploads reserve ten minutes because the server cannot reliably determine their duration in advance; an oversized result is rejected and its actual usage recorded after inference. This compatibility path leaves a possible provider-cost overrun from one oversized upload.

The new live fixture returned the expected 17 words with WER 0 and exactly 5.855 seconds charged once. Cleanup added no audio. A concurrent cleanup race plus replay created one provider attempt. The final accepted recording reached 1804.855/1800 seconds; its cleanup succeeded and the next transcription returned 429. Both synthetic accounts and dependent records were removed, with no email sent. Local and production rollback SQL checks, real database races and service-only access checks passed. See [Testing](TESTING.md) for exact commands/counts and [the API contract](AUTH_AND_CLOUD.md#free-cloud-transcription-and-cleanup-contract).

## The selected Pro price requires a subsidy at full usage

The selected 180-hour allowance remains in place at $4.99/month. It is not profitable at full use under the verified provider prices. Groq lists Turbo at $0.04/hour and Large v3 at $0.111/hour, making 180 hours cost **$7.20** or **$19.98** for transcription alone. Hosted transcription stays on Turbo; recommending Large v3 for personal keys does not change that cost. [Groq speech pricing](https://console.groq.com/docs/speech-to-text).

After Lemon Squeezy's standard 5% + $0.50 and 0.5% subscription fee, a simplified $4.99 subscription yields **$4.21555 before other fees and operating costs**. Full Turbo usage therefore loses $2.98445 before cleanup. International transactions, payouts, taxes in the fee base and other applicable charges can reduce receipts further. [Lemon Squeezy fees](https://docs.lemonsqueezy.com/help/getting-started/fees).

At GPT-OSS 20B's $0.075/million input and $0.30/million output, the full 9M/6M cleanup allowance adds **$2.475**, taking that scenario to **-$5.45945** before hosting, email, support, Free users and other costs. Completion accounting includes reasoning. These are allowance scenarios, not observed average customer usage or a guarantee against legacy-upload overruns. [Groq model pricing](https://console.groq.com/docs/models).

The operational recommendation is to measure actual usage and fund the subsidy, with provider spend alerts and tested account-creation abuse controls before broad acquisition. Do not label the plan profitable by reusing the earlier 60-hour estimate. The calculation does not authorize changing the selected price, model or existing paid entitlement. See [Production readiness](PRODUCTION_READINESS.md#cost-of-the-180-hour-allowance) for the complete calculation and Free billing-minimum caveat.

## Public application, private credentials

Keep one real MIT-licensed public application and its reusable backend source. The license permits reuse, modification, distribution and sale with the required copyright/license notice. Preserve third-party notices. Public source includes login, migrations, permission rules and `src-tauri/src/secrets.rs`; that filename describes credential-storage code, not embedded credential values.

| Safe public configuration/source | Keep outside source and app bundles |
| --- | --- |
| Supabase URL and publishable/legacy anon key | Supabase service-role/secret keys and Management API tokens |
| Google client ID, callback and login UI | Google client secret, passwords and session/refresh tokens |
| Checkout/store/product identifiers and updater public verification key | Lemon management/webhook secrets and updater private key |
| Backend source and database access rules | Provider keys, SMTP passwords and Apple credentials |

A Supabase public key identifies the project; verified identity, license ownership and database rules authorize operations. Privileged keys bypass client restrictions and stay server-side. [Supabase key types](https://supabase.com/docs/guides/getting-started/api-keys). Live readback found seven billing/usage tables with RLS and zero anon/authenticated grants, plus twelve quota RPCs restricted to service access. A targeted public-history scan at `5e61508` found no detected privileged provider/server/signing credentials; the historical anon JWT was public configuration. This is a bounded audit result, not a promise of zero vulnerabilities.

Installed binaries and modified clients can reveal public service configuration. A private cloud wrapper is not required for security and would not protect embedded secrets. An optional private operations repository may pin this public source as a submodule for builds/deployment; retain one editable app and keep actual secrets in server, CI or OS secure storage. Default public builds use personal keys with hosted services/updates disabled; forks should configure their own service and updater identity.

Local provider keys, sessions and runtime license credentials remain in the OS secure store with the existing release/debug namespace isolation, noninteractive macOS lookups and cache. Do not move them into ordinary SQLite or settings JSON. See [Local credentials](LOCAL_CREDENTIALS.md) for migration, tombstones and verification boundaries.

## Pro device compatibility and billing verification

Normal desktop activation obtains a Lemon instance and applies the existing five-device activation limit. The hosted API also accepts legacy `x-license-key` requests without `x-license-instance`, which current released clients omit. Key-only validation does not enforce the instance/device cap against every modified client; total audio/request/token allowances remain shared by the license.

Resolve this with a compatible client/server transition that sends and validates the instance, preserves existing licenses and tests old-client behavior. Do not suddenly make the header mandatory for every released client. A fresh valid-license provider call and complete paid lifecycle were not part of the new Free fixture checks. Test activation, deactivation, multiple devices, cancellation and expiry before claiming the complete commercial flow verified. See [the compatibility boundary](AUTH_AND_CLOUD.md#pro-activation-compatibility-boundary).

The verified checkout has `has_free_trial=false` and the desktop flag remains false. A seven-day trial must prove immediate access, any license issuance, cancellation, expiry, successful first payment and failed first payment before being advertised. Preserve existing key-based licenses during any account/webhook entitlement addition. A configured trial flag or success screen is not entitlement evidence. [Lemon Squeezy trial setup](https://docs.lemonsqueezy.com/help/products/free-trials).

## Remaining production work

Google uses the configured Web application client in Megacubos and Supabase's callback, with the desktop external browser/PKCE flow and restricted loopback redirect. An owned test-user sign-in and restart passed on 0.3.1. Google's audience remains Testing. The last verified homepage, privacy and terms URLs on `dictamelo.com` redirect to parking content; publish real product/legal pages and complete production branding before claiming general Google availability.

Email/password uses Supabase Auth. Confirmations are enabled, but production SMTP has no host configured. Admin-generated confirmation/recovery tokens verify API behavior, not inbox delivery. Configure a transactional sender and verify confirmation and password recovery with an owned mailbox. CAPTCHA is off, and per-account quotas do not prevent many-account abuse; any signup control must be integrated and tested with the native flow.

Align app, website and checkout copy with 30 minutes/week and 180 hours/rolling 30 days. Complete privacy/retention, support, deletion and incident procedures. A backend deployment, a signed local candidate, a public GitHub prerelease, Google production status, native Windows execution and a successful updater installation are separate milestones. Keep their status and remaining limitations explicit in [Production readiness](PRODUCTION_READINESS.md) and [Testing](TESTING.md).
