# Auth and cloud configuration

The desktop application, login UI and reusable backend code stay in the public repository. A clean source build uses personal provider keys and leaves hosted services and automatic updates disabled. The official edition compiles the same source with explicit public service metadata.

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

Fork maintainers should use their own cloud, checkout, app identity and update channel. Keeping official metadata outside the default source build prevents forks from accidentally using the official service. A private operations repository may pin this public repository as a Git submodule and run its build commands, without copying or modifying a separate app.

## Server configuration

The official hosted project is `iburiyhhfodndqgmsaot`. Verify the target before changing any configuration; a connected tool may point at another project.

Current deployed state: migrations `20260905020000_pro_quota` and `20260905030000_free_cleanup` and the revised `transcribe`/`cleanup` handlers are live, with the verified Lemon ownership IDs above. Free Cloud now includes cleanup of its completed transcriptions. Live rollback SQL, access-control and fixture transcription/cleanup checks passed; see [Testing](TESTING.md). A fresh valid-license Pro transcription/cleanup test is still separate.

| Server setting | Purpose |
| --- | --- |
| `GROQ_API_KEY` | Hosted transcription and cleanup; Edge Function secret |
| `LEMON_STORE_ID` | Verified store allowed to grant hosted Pro |
| `LEMON_PRODUCT_ID` | Verified product allowed to grant hosted Pro |
| `LEMON_VARIANT_IDS` | Comma-separated allowed variants, including supported existing licenses |
| Supabase service-role/secret credential | Server-only database access; never a desktop setting |
| Google provider client ID and secret | Supabase Auth's Google configuration |
| SMTP host, sender and credentials | Delivery of confirmation and password-reset messages |

The server's Lemon IDs and desktop IDs must agree. Server checks remain authoritative even if a client is modified. The Pro quota migration sets 60 hosted audio hours, 12,000 requests, 3M cleanup input tokens and 2M completion tokens over a rolling 30-day window. The former `MONTHLY_SECONDS` setting is no longer the source of truth. Free Cloud remains 2,000 words and 200 transcription attempts per UTC week, with included cleanup subject to the separate safeguards below.

Apply compatible migrations before deploying dependent Edge Functions. Quota tables and reservation/finish RPCs are service-only. Abandoned or ambiguous paid requests retain their conservative reservation because a timeout does not prove that the provider billed nothing. Current PCM uploads are validated before inference; compressed legacy uploads retain the documented oversize-request limitation in [the initial review](INITIAL_RELEASE_REVIEW.md).

## Free Cloud transcription and cleanup contract

Free cleanup requires a confirmed Supabase account and a receipt for that account's completed transcription. It does not require a Pro license or personal provider key. The server fixes the model to `openai/gpt-oss-20b` and supplies the cleanup instructions; client-supplied models or prompts do not customize this route.

1. Send `POST /functions/v1/transcribe` with `Authorization: Bearer <user access token>` and a multipart `file`: mono PCM16 WAV at 16 kHz, at most two minutes. Do not send `x-license-key` for Free Cloud.
2. A successful response includes the trimmed raw `text` and `cleanupReceipt` UUID. Empty speech has a null receipt. Word settlement and receipt creation occur in one database transaction.
3. Send `POST /functions/v1/cleanup` using the same account token and JSON `{"text": "<exact returned raw text>", "cleanupReceipt": "<receipt UUID>"}`. Preserve the raw text: the receipt is bound to the account and SHA-256 of the UTF-8 transcript after trimming surrounding whitespace. Edited text or another account cannot redeem it.
4. Read the cleaned text from `choices[0].message.content`. Keep the original transcription available if cleanup fails. The Free response omits provider reasoning and internal usage details.

Each receipt is valid for **24 hours**, permits **at most two reserved attempts**, and becomes unusable after one successful cleanup. Replays are rejected. An account may have only one active cleanup reservation at a time; the lease lasts three minutes. Expiring a lease does not refund an uncertain provider outcome. Definite provider rejection can settle zero tokens, while timeouts/ambiguous results retain their conservative token reservation.

The independent cost safeguards are **250,000 input tokens and 250,000 completion tokens per account per original transcription UTC week**. Completion usage includes reasoning. A receipt created before a weekly rollover remains charged to its original week. Requests are bounded to 20,000 text characters and at most 8,192 completion tokens, with conservative input/output reservations before provider access and actual usage settlement afterward. The HTTP JSON body also has a 100,000-byte limit.

Cleanup never adds words or transcription attempts to the public Free allowance. The last accepted transcription can take the counter above 2,000 words and still use its valid cleanup receipt; subsequent transcription waits for renewal. Exhausted cleanup safeguards leave the raw transcript intact. They do not convert Free Cloud into an unrestricted text-generation API.

The service-only `free_cleanup_receipts` and `free_cleanup_attempts` tables store the transcript digest, account/receipt identifiers, original week, word/token counts, timestamps and attempt state. They store **neither the transcript nor the cleaned output**. The digest is verification metadata, not encrypted transcript storage or an anonymity guarantee. Plain text is processed transiently and sent to the hosted model; provider retention policies still apply. Receipt expiry ends permission to clean the text, not automatic deletion of the accounting metadata. Client roles have neither direct table access nor permission to execute the settlement/reservation RPCs.

The explicit live regression is `python3 scripts/test-free-cleanup-live.py --live --project-ref iburiyhhfodndqgmsaot`. It uses disposable identities and the licensed speech fixture, not a customer's session. The verified run returned the expected 17-word transcript, completed real cleanup without increasing that word count, rejected replay/cross-account/changed-text requests, and demonstrated one winner in a concurrent claim. A separate test-only 1,999-word counter reached 2,016 after transcription and still permitted cleanup; the next transcription returned 429. Test identities and their dependent records were removed. These checks do not prove SMTP delivery or native dictation UI behavior.

## Email/password accounts

The preview supports account creation with email and password, ordinary password login, account confirmation and password recovery. Tokens are used for confirmation/recovery, not as the normal sign-in method. Passwords are sent to Supabase Auth; session tokens use the operating system credential store.

The 0.3.1 credential store uses a release runtime namespace and a separate debug namespace. Release builds can silently migrate accessible legacy entries; debug builds cannot read or migrate the installed app's credentials. See [Local credentials](LOCAL_CREDENTIALS.md) for caching, denied-access recovery and logout deletion markers.

Keep email confirmations enabled. Live readback confirmed that setting and both confirmation/recovery token templates; SMTP currently has no host configured. Configure a production transactional sender in Supabase Auth's SMTP settings. Supabase manages identity and authentication; the SMTP provider delivers the messages. Changing the login screen to use a password does not remove confirmation and recovery email requirements.

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

Google's audience remains **Testing**. Production publishing is disabled while branding is incomplete. The current `dictamelo.com` HTTP 200 response leads to a parked/lander page through JavaScript, so it is not a verified product homepage or privacy policy. The maintainer must supply the actual homepage and privacy-policy URLs before completing production branding. Do not invent a policy or treat a successful HTTP status as sufficient. Production Google publication is separate from configuring Supabase and allowing a test account.

Google's [branding requirements](https://support.google.com/cloud/answer/15549049?hl=en) also require public terms of service for external production apps. Complete the homepage, privacy-policy and terms links, register the domains used in branding/client configuration, and verify the owned brand before public rollout. The current test login requests only name/profile and email identity information; it does not grant Gmail inbox access.

## Seven-day Pro trial

The verified checkout currently has `has_free_trial=false`. `DICTAMELO_PRO_TRIAL_AVAILABLE` stays `false` until a tested billing flow grants immediate access. A Lemon Squeezy subscription trial collects payment details and charges after the trial unless cancelled; it is separate from the always-free cloud allowance.

Do not infer trial entitlement from a success screen or an unverified client flag. Either prove that the existing license-key flow issues and validates a usable key during the zero-charge trial, or use signed subscription webhooks to grant account entitlement from `on_trial` and the actual trial end. Preserve existing active licenses. Test checkout, immediate access, cancellation, expiry, successful first payment and failed first payment before enabling the public trial action.

## Deployment, preview and publication are separate

Deploying the backend changes the hosted service. Building/installing a Mac preview changes the local application. Publishing a GitHub release makes installers and an updater manifest public. The final 0.3.1 preview has been notarized, installed and verified locally, including the native application-menu update action and retained Google session. Google production publication and real SMTP delivery are additional external steps. This iteration does not publish a release, modify public 0.1.2 assets or replace the 0.2.0 draft installers. The [0.3.0 record](releases/0.3.0.md) remains historical evidence of its own checks and pending setup at that time.
