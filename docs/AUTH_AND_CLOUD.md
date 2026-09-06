# Auth and cloud configuration

This guide describes the 0.5.0 source contract. Deployment and installed-artifact evidence belongs in [Testing](TESTING.md); documentation alone does not establish that a server migration or release is live.

The desktop application, login UI and reusable backend code stay in the public repository. A clean source build uses personal provider keys and leaves hosted services and automatic updates disabled. The official edition compiles the same source with explicit public service metadata.

## Public source and private credentials

Keep one public application implementation. A private cloud wrapper is **not required** for the current architecture. The [MIT license](../LICENSE) permits others to use, modify, distribute and sell the code while retaining its copyright and license notice. Open source does not mean that users receive the operator's private credentials or access to another customer's account.

| Public configuration or source | Private operational or user data |
| --- | --- |
| Supabase project URL and publishable/legacy anon key | Supabase service-role/secret keys and Management API tokens |
| Login UI, backend functions, migrations and permission rules | Provider API secrets, SMTP credentials and Google client secret |
| Google client ID and registered callback URL | Account passwords, access/refresh tokens and individual license credentials |
| Checkout/store/product identifiers and updater public verification key | Lemon management/webhook secrets, updater private key and Apple credentials |

The public Supabase key identifies the project. It does not replace the user's verified token, a valid license, database permissions or server-owned quota checks. The desktop configuration parser rejects privileged Supabase keys. Server credentials bypass client permissions and must never be embedded in source or a shipped binary. [Supabase key types](https://supabase.com/docs/guides/getting-started/api-keys).

A person can inspect an installed binary, obtain its public endpoint configuration and modify a client. A private repository would not prevent that. Hosted endpoints therefore validate identity or license ownership before provider access and enforce allowances in the database. An authorized user can call the API within their allowance; the system does not claim that only an unmodified official binary can call it. Keep administrative credentials on the server and user credentials in the [OS secure store](LOCAL_CREDENTIALS.md).

## Desktop build metadata

Create an ignored `.env.cloud-build` file. These values are public configuration that will be readable in the compiled app; none is a server secret.

```dotenv
DICTAMELO_SUPABASE_URL=https://your-project.supabase.co
DICTAMELO_SUPABASE_ANON_KEY=your-public-anon-or-publishable-key
DICTAMELO_BACKEND_URL=https://your-project.supabase.co/functions/v1
DICTAMELO_LEMON_STORE_ID=your-store-id
DICTAMELO_LEMON_PRODUCT_ID=your-product-id
DICTAMELO_LEMON_VARIANT_IDS=your-allowed-variant-ids
DICTAMELO_CHECKOUT_URL=https://your-store.lemonsqueezy.com/buy/your-checkout-id
DICTAMELO_PRO_TRIAL_AVAILABLE=false
DICTAMELO_UPDATES_ENABLED=false
```

The example contains placeholders; obtain verified IDs from the product/store/variant records. A product-editor URL alone does not establish which ID it contains. Use comma-separated variant IDs when more than one existing variant should remain valid. `DICTAMELO_BACKEND_URL` can be omitted when the backend uses the Supabase project's standard `/functions/v1` path.

Official deployment metadata verified on September 5, 2026 (reference only; keep default source builds unconfigured):

| Field | Verified value |
| --- | --- |
| Supabase project | `iburiyhhfodndqgmsaot` |
| Lemon Squeezy store | `447162` |
| Product | `1340872` |
| Allowed variant | `2094776` |
| Price | $4.99/month |
| Free trial | `false` |

The [official checkout](https://megacubos.lemonsqueezy.com/checkout/buy/10dfddfe-2870-45b9-818b-43197ae8b370) provided the public product/variant metadata. These identifiers are safe to disclose; they do not authorize server or provider access.

Run the desired command through the wrapper:

```sh
python3 scripts/with-cloud-config.py --config .env.cloud-build -- npm run dev
python3 scripts/with-cloud-config.py --config .env.cloud-build -- cargo check --manifest-path src-tauri/Cargo.toml
```

Configuration is compiled into the executable. Rebuild after changing it. `DICTAMELO_UPDATES_ENABLED` defaults to `false`; set it to `true` only for official builds, or after configuring your own updater URL and public verification key. A fork must not replace itself with the official application. The wrapper is not a secret manager; do not put `GROQ_API_KEY`, a Supabase service-role/secret key, a Google client secret, SMTP credentials or signing credentials in this file. Do not source an unrelated production environment into a desktop build.

Fork maintainers should use their own cloud, checkout, app identity and update channel. Keeping official metadata outside the default source build prevents forks from accidentally using the official service. If a private operations repository becomes useful for deployment orchestration, it may pin this public repository as a Git submodule and run its build commands. It must not contain a second editable app or bundle privileged credentials. That wrapper is optional operational organization, not a security requirement.

## Server configuration

The official hosted project is `iburiyhhfodndqgmsaot`. Verify the target before changing any configuration; a connected tool may point at another project.

The 0.5.0 allowance migration `20260906000000_audio_time_plans` is deployed after the existing license, Free, Pro and cleanup migrations, followed by compatible `transcribe`, `cleanup` and `usage` handlers. Live rollback SQL checks and the real Free transcription/cleanup regression passed against the official project. The fixture consumed exactly 5.855 seconds once; its cleanup consumed no additional audio, and the final accepted recording reached 1804.855/1800 seconds before the next transcription returned 429. Both disposable accounts and dependent records were removed. See [Testing](TESTING.md) for the commands and evidence; these checks do not constitute a new valid-license Pro provider or paid lifecycle test.

| Server setting | Purpose |
| --- | --- |
| `GROQ_API_KEY` | Hosted transcription and cleanup; Edge Function secret |
| `LEMON_STORE_ID` | Verified store allowed to grant hosted Pro |
| `LEMON_PRODUCT_ID` | Verified product allowed to grant hosted Pro |
| `LEMON_VARIANT_IDS` | Comma-separated allowed variants, including supported existing licenses |
| Supabase service-role/secret credential | Server-only database access; never a desktop setting |
| Google provider client ID and secret | Supabase Auth's Google configuration |
| SMTP host, sender and credentials | Delivery of confirmation and password-reset messages |

The server's Lemon IDs and desktop IDs must agree. Server checks remain authoritative even if a client is modified. The 0.5.0 contract is:

| Hosted plan | Public audio allowance | Separate request/cleanup safeguards |
| --- | --- | --- |
| Free Cloud | 30 minutes per UTC week, shared across devices; renews Monday at 00:00 UTC | 1,000 transcription attempts/week; 250,000 cleanup input and 250,000 completion tokens per original transcription week |
| Pro | 180 hours across a rolling 30-day window; prior usage expires continuously, rather than resetting on a calendar date | 36,000 transcription/cleanup requests; 9M cleanup input and 6M completion tokens in the same rolling window |

Free time comes from validated PCM bytes, not client duration metadata or a word estimate. Successful silence consumes processing time too. Existing word usage is converted once at 0.45 seconds per word: an old 2,000-word balance becomes 15 minutes of historical usage. New requests use measured time. The API retains word fields for older clients, but words are no longer the current plan's public limit. This migration increases the allowance without discarding previous usage.

Pro retains the existing **ten-second minimum per transcription request**, matching the provider's billing minimum. Both plans include cleanup; cleanup does not add another audio charge. Personal-key mode has no Dictámelo-hosted allowance and is billed directly by the chosen provider. Hosted transcription uses Whisper Large v3 Turbo; the recommended Whisper Large v3 selection in personal-key setup does not change the hosted model. Hosted cleanup uses GPT-OSS 20B.

The database functions are the quota source of truth; the former `MONTHLY_SECONDS` setting is not. Keep the app, website and checkout consistent with these limits. See [Production readiness](PRODUCTION_READINESS.md#cost-of-the-180-hour-allowance) for the explicit subsidy at maximum usage.

Quota tables and reservation/finish RPCs are service-only. Live verification found RLS enabled on all seven billing/usage tables, no anon/authenticated table grants, and twelve quota RPCs restricted to service access. The time-plan migration also removes legacy client grants on `licenses` and `usage_events`; those tables already had default-deny RLS with no client policies. Both grants and RLS matter. [Supabase access controls](https://supabase.com/docs/guides/database/secure-data).

Abandoned or ambiguous requests retain their conservative reservation because a timeout does not prove that the provider billed nothing. An explicit provider rejection may release the reserved time while still counting the attempt. Current PCM uploads are validated before inference; compressed legacy Pro uploads retain the documented oversize-request limitation in [the initial review](INITIAL_RELEASE_REVIEW.md).

## Pro activation compatibility boundary

The normal desktop activation flow obtains a Lemon license instance and applies the existing five-device activation limit. The hosted API currently accepts the legacy `x-license-key` header without requiring `x-license-instance`; current released hosted clients omit the instance header. Lemon validation without an instance checks the license key alone. Therefore the five-device restriction is not yet enforced against every modified hosted client. The total server allowance remains shared by the license. [Lemon validation contract](https://docs.lemonsqueezy.com/api/license-api/validate-license-key).

Closing this gap requires a compatible desktop/server transition that sends and verifies the instance without invalidating existing paid licenses. Do not suddenly reject every released client by making the header mandatory before that transition. Include activation, deactivation, multiple devices, invalid instances and old-client behavior in the release evidence.

## Free Cloud transcription and cleanup contract

Free cleanup requires a confirmed Supabase account and a receipt for that account's completed transcription. It does not require a Pro license or personal provider key. The server fixes the model to `openai/gpt-oss-20b` and supplies the cleanup instructions; client-supplied models or prompts do not customize this route.

1. Send `POST /functions/v1/transcribe` with `Authorization: Bearer <user access token>` and a multipart `file`: mono PCM16 WAV at 16 kHz, at most two minutes. Do not send `x-license-key` for Free Cloud.
2. The server reserves the validated audio duration before contacting the provider. A successful response includes the trimmed raw `text`, measured `duration` and `cleanupReceipt` UUID. Empty speech has a null receipt but still settles its audio time. Duration settlement, compatibility word accounting and receipt creation occur in one database transaction.
3. Send `POST /functions/v1/cleanup` using the same account token and JSON `{"text": "<exact returned raw text>", "cleanupReceipt": "<receipt UUID>"}`. Preserve the raw text: the receipt is bound to the account and SHA-256 of the UTF-8 transcript after trimming surrounding whitespace. Edited text or another account cannot redeem it.
4. Read the cleaned text from `choices[0].message.content`. Keep the original transcription available if cleanup fails. The Free response omits provider reasoning and internal usage details.

Each receipt is valid for **24 hours**, permits **at most two reserved attempts**, and becomes unusable after one successful cleanup. Replays are rejected. An account may have only one active cleanup reservation at a time; the lease lasts three minutes. Expiring a lease does not refund an uncertain provider outcome. Definite provider rejection can settle zero tokens, while timeouts/ambiguous results retain their conservative token reservation.

The independent cost safeguards are **250,000 input tokens and 250,000 completion tokens per account per original transcription UTC week**. Completion usage includes reasoning. A receipt created before a weekly rollover remains charged to its original week. Requests are bounded to 20,000 text characters and at most 8,192 completion tokens, with conservative input/output reservations before provider access and actual usage settlement afterward. The HTTP JSON body also has a 100,000-byte limit.

Cleanup never adds audio time or transcription attempts to the public Free allowance. The last accepted recording is delivered whole and may take usage beyond 30 minutes by at most one two-minute recording. A single active transcription reservation per account prevents concurrent requests from multiplying that final overage. Its valid receipt can still be cleaned; subsequent transcription waits for renewal. Exhausted cleanup safeguards leave the raw transcript intact. They do not convert Free Cloud into an unrestricted text-generation API.

The service-only `free_cleanup_receipts` and `free_cleanup_attempts` tables store the transcript digest, account/receipt identifiers, original week, word/token counts, timestamps and attempt state. They store **neither the transcript nor the cleaned output**. The digest is verification metadata, not encrypted transcript storage or an anonymity guarantee. Plain text is processed transiently and sent to the hosted model; provider retention policies still apply. Receipt expiry ends permission to clean the text, not automatic deletion of the accounting metadata. Client roles have neither direct table access nor permission to execute the settlement/reservation RPCs.

The explicit live regression is `python3 scripts/test-free-cleanup-live.py --live --project-ref iburiyhhfodndqgmsaot`. It uses disposable identities and the licensed speech fixture, not a customer's session. Test exact duration settlement, cleanup without a second audio charge, successful silence, provider rejection, uncertain failures, replay/cross-account/changed-text rejection, concurrent reservations and the 30-minute boundary. Preserve the expected 17-word transcript as a quality check; word accuracy and time metering are separate assertions. Verify removal of test identities and their dependent records. Record actual results in [Testing](TESTING.md). These checks do not prove SMTP delivery or native dictation UI behavior.

## Email/password accounts

The preview supports account creation with email and password, ordinary password login, account confirmation and password recovery. Tokens are used for confirmation/recovery, not as the normal sign-in method. Passwords are sent to Supabase Auth; session tokens use the operating system credential store.

The credential store introduced in 0.3.1 remains the 0.5.0 design: separate release and debug runtime namespaces. Release builds can silently migrate accessible legacy entries; debug builds cannot read or migrate the installed app's credentials. See [Local credentials](LOCAL_CREDENTIALS.md) for caching, denied-access recovery and logout deletion markers.

Keep email confirmations enabled. Live readback on September 5, 2026 confirmed that setting; SMTP has no host, user or sender configured. Configure a production transactional sender in Supabase Auth's SMTP settings. Supabase manages identity and authentication; the SMTP provider delivers the messages. Changing the login screen to use a password does not remove confirmation and recovery email requirements. The default Supabase email service is restricted and is not a production sender. [Supabase SMTP requirements](https://supabase.com/docs/guides/auth/auth-smtp).

CAPTCHA was disabled in the same readback. Before opening public signup, implement and verify an account-creation abuse control and provider spend alerts. Per-account audio quotas do not stop someone creating many accounts. Enabling CAPTCHA requires a corresponding client flow; do not turn it on server-side without testing signup, login and recovery. [Supabase CAPTCHA integration](https://supabase.com/docs/guides/auth/auth-captcha).

Confirmation and recovery templates must show `{{ .Token }}` so the user can enter the code in the app. Do not replace normal password login with magic-link-only wording. Preserve any required legacy template behavior for existing draft clients while they remain in use.

Avoid an unreviewed `supabase config push`: it can change unrelated Auth, API and Storage settings. Prefer a targeted Auth Management API patch or the dashboard, preserving existing settings and keeping credentials out of tracked files and logs.

The live Auth API contract passed confirmation, replay rejection, password login, refresh, recovery and logout using a temporary synthetic account; that account was removed afterward. Before announcing the complete flow, verify actual delivery and confirmation/recovery with an owned mailbox. Admin-generated test tokens verify backend behavior but do not verify SMTP delivery.

## Google sign-in

The official deployment now uses an OAuth **Web application** client in the user-selected **Megacubos** Google Cloud project, `ardent-particle-507721-s2`. Required Google terms were accepted with the user's authorization. The provider is enabled in the correct Supabase project, `iburiyhhfodndqgmsaot`. The Google callback is:

```text
https://iburiyhhfodndqgmsaot.supabase.co/auth/v1/callback
```

For a self-hosted deployment, substitute its actual Supabase project callback. Store the Google client secret in Supabase Auth's Google provider configuration. The desktop app starts OAuth through Supabase, so it needs neither a Google client secret nor a direct Google client ID in its executable.

The configured client secret was checked through masked provider readback. Temporary credential downloads were cleaned after the server transfer; the secret was not added to desktop configuration or source files.

The native client opens the system browser and uses PKCE. Its temporary loopback listener uses an ephemeral port and a nonce path. The matching Supabase redirect allowlist entry is:

```text
http://127.0.0.1:*/auth/callback/**
```

Do not expand this to arbitrary remote hosts. Request only identity scopes (`openid`, email and profile). Check Google consent-screen branding, audience/test-user restrictions and publishing status before claiming general availability. A configured button is not proof that browser sign-in returns to the installed app.

The redirect allowlist above is present in the live Supabase configuration. An owned account added as a Google test user completed the installed 0.3.1 candidate's browser → Supabase → native PKCE callback flow. The app displayed the correct account and 0/2,000-word usage. Quitting and reopening preserved the session without a Keychain prompt. This is real native verification, not a mock or admin-generated token test; it does not establish cancellation/timeout behavior or general Google availability.

Google's audience remains **Testing** according to the last recorded dashboard verification on September 5. Production publishing was disabled while branding was incomplete. Google explicitly [exempts identity-only login](https://support.google.com/cloud/answer/15549945) from the Testing allowlist, warning and seven-day authorization expiry; this app requests only those identity scopes. Testing status alone therefore does not establish that other Google accounts cannot sign in. The installed 0.5.0 app completed owned-account Google login, native callback and full-restart persistence on September 6 without a Keychain prompt.

The latest homepage/privacy/terms URL checks are recorded in [Production readiness](PRODUCTION_READINESS.md): they still did not return usable product or policy pages on September 6. Publish the actual pages before completing production branding. Do not invent a policy or treat a successful HTTP status as sufficient. Branding completion, installed login and production email delivery are separate checks.

Google's [branding requirements](https://support.google.com/cloud/answer/15549049?hl=en) also require public terms of service for external production apps. Complete the homepage, privacy-policy and terms links, register the domains used in branding/client configuration, and verify the owned brand before public rollout. The current test login requests only name/profile and email identity information; it does not grant Gmail inbox access.

## Seven-day Pro trial

The verified checkout currently has `has_free_trial=false`. `DICTAMELO_PRO_TRIAL_AVAILABLE` stays `false` until a tested billing flow grants immediate access. A Lemon Squeezy subscription trial collects payment details and charges after the trial unless cancelled; it is separate from the always-free cloud allowance.

Do not infer trial entitlement from a success screen or an unverified client flag. Either prove that the existing license-key flow issues and validates a usable key during the zero-charge trial, or use signed subscription webhooks to grant account entitlement from `on_trial` and the actual trial end. Preserve existing active licenses. Test checkout, immediate access, cancellation, expiry, successful first payment and failed first payment before enabling the public trial action.

## Deployment, preview and publication are separate

Deploying the backend changes the hosted service. Building/installing a Mac candidate changes the local application. Publishing a GitHub release makes installers and an updater manifest public. The 0.4.0 prerelease has its own notarization, installation and public-download evidence; none automatically verifies a 0.5.0 artifact. Google production publication, real SMTP delivery, payment lifecycle and native Windows checks remain separate acceptance steps. Follow [Releasing](RELEASING.md), retain immutable published installers and record each result against its exact version and source commit. Historical release records remain evidence of their own checks and pending setup at that time.
