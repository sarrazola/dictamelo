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

Current deployed state: migration `20260905020000_pro_quota` and the revised `transcribe`/`cleanup` handlers are live, with the verified Lemon ownership IDs above. A live rolled-back quota test, ACL/RLS checks and invalid-auth HTTP checks passed. See [Testing](TESTING.md). A fresh valid-license transcription/cleanup test is still separate.

| Server setting | Purpose |
| --- | --- |
| `GROQ_API_KEY` | Hosted transcription and cleanup; Edge Function secret |
| `LEMON_STORE_ID` | Verified store allowed to grant hosted Pro |
| `LEMON_PRODUCT_ID` | Verified product allowed to grant hosted Pro |
| `LEMON_VARIANT_IDS` | Comma-separated allowed variants, including supported existing licenses |
| Supabase service-role/secret credential | Server-only database access; never a desktop setting |
| Google provider client ID and secret | Supabase Auth's Google configuration |
| SMTP host, sender and credentials | Delivery of confirmation and password-reset messages |

The server's Lemon IDs and desktop IDs must agree. Server checks remain authoritative even if a client is modified. The quota migration sets 60 hosted audio hours, 12,000 requests, 3M cleanup input tokens and 2M completion tokens over a rolling 30-day window. The former `MONTHLY_SECONDS` setting is no longer the source of truth. Free Cloud remains 2,000 words and 200 attempts per UTC week.

Apply compatible migrations before deploying dependent Edge Functions. Quota tables and reservation/finish RPCs are service-only. Abandoned or ambiguous paid requests retain their conservative reservation because a timeout does not prove that the provider billed nothing. Current PCM uploads are validated before inference; compressed legacy uploads retain the documented oversize-request limitation in [the initial review](INITIAL_RELEASE_REVIEW.md).

## Email/password accounts

The preview supports account creation with email and password, ordinary password login, account confirmation and password recovery. Tokens are used for confirmation/recovery, not as the normal sign-in method. Passwords are sent to Supabase Auth; session tokens use the operating system credential store.

Keep email confirmations enabled. Live readback confirmed that setting and both confirmation/recovery token templates; SMTP currently has no host configured. Configure a production transactional sender in Supabase Auth's SMTP settings. Supabase manages identity and authentication; the SMTP provider delivers the messages. Changing the login screen to use a password does not remove confirmation and recovery email requirements.

Confirmation and recovery templates must show `{{ .Token }}` so the user can enter the code in the app. Do not replace normal password login with magic-link-only wording. Preserve any required legacy template behavior for existing draft clients while they remain in use.

Avoid an unreviewed `supabase config push`: it can change unrelated Auth, API and Storage settings. Prefer a targeted Auth Management API patch or the dashboard, preserving existing settings and keeping credentials out of tracked files and logs.

The live Auth API contract passed confirmation, replay rejection, password login, refresh, recovery and logout using a temporary synthetic account; that account was removed afterward. Before announcing the complete flow, verify actual delivery and confirmation/recovery with an owned mailbox. Admin-generated test tokens verify backend behavior but do not verify SMTP delivery.

## Google sign-in

Use the user-selected Google Cloud project and configure an OAuth **Web application** client for Supabase. The Google callback is:

```text
https://iburiyhhfodndqgmsaot.supabase.co/auth/v1/callback
```

For a self-hosted deployment, substitute its actual Supabase project callback. Store the Google client secret in Supabase Auth's Google provider configuration. The desktop app starts OAuth through Supabase, so it needs neither a Google client secret nor a direct Google client ID in its executable.

The native client opens the system browser and uses PKCE. Its temporary loopback listener uses an ephemeral port and a nonce path. The matching Supabase redirect allowlist entry is:

```text
http://127.0.0.1:*/auth/callback/**
```

Do not expand this to arbitrary remote hosts. Request only identity scopes (`openid`, email and profile). Check Google consent-screen branding, audience/test-user restrictions and publishing status before claiming general availability. A configured button is not proof that browser sign-in returns to the installed app.

Verify sign-in with an owned Google account, correct account display, cancellation, timeout, an invalid callback, session persistence and sign-out. The redirect allowlist above is present in the live Supabase configuration. Google sign-in is still disabled pending completion of Google Cloud setup/terms and a complete live return-to-app test.

## Seven-day Pro trial

The verified checkout currently has `has_free_trial=false`. `DICTAMELO_PRO_TRIAL_AVAILABLE` stays `false` until a tested billing flow grants immediate access. A Lemon Squeezy subscription trial collects payment details and charges after the trial unless cancelled; it is separate from the always-free cloud allowance.

Do not infer trial entitlement from a success screen or an unverified client flag. Either prove that the existing license-key flow issues and validates a usable key during the zero-charge trial, or use signed subscription webhooks to grant account entitlement from `on_trial` and the actual trial end. Preserve existing active licenses. Test checkout, immediate access, cancellation, expiry, successful first payment and failed first payment before enabling the public trial action.

## Deployment, preview and publication are separate

Deploying the backend changes the hosted service. Building/installing a Mac preview changes the local application. Publishing a GitHub release makes installers and an updater manifest public. This iteration built, notarized and locally installed the 0.3.0 Mac preview; it does not publish a release, modify the public 0.1.2 assets or replace the 0.2.0 draft installers.
