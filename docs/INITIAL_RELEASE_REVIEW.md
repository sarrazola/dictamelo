# Initial release review

Reviewed on September 5, 2026. This is a product and architecture recommendation, not a claim that every proposed provider, trial, or safeguard has been deployed. The application under review started at `d4a613abee9c79855648e8328fab9f933f61663b`. No Windows changes or tests form part of this review.

## Three understandable choices

| Choice | What the customer receives | Account and payment |
| --- | --- | --- |
| Free — your own API keys | Dictation and supported cleanup using the customer's provider account. Dictámelo does not impose a cloud allowance on this mode. | No Dictámelo account required. The provider may charge the customer. |
| Free Cloud | 2,000 words per week, shared across devices, with a visible usage counter and renewal date. | Create a free account using email/password or Google. No subscription required. |
| Pro | Recommended: 60 hours of hosted transcription per rolling 30 days, with text cleanup and the existing five-device license. | $4.99/month. Preserve existing licenses. |

“Free with your keys” describes the application's price, not a promise that Groq, OpenAI, or another provider is free. Explain that directly under its price. Display the active route separately from whether an account exists: a signed-in user can still choose their own keys.

Use 60 hours as an explicit allowance, not “unlimited.” It is three times the previous allowance and averages two hours per day. Keep provider details out of the cloud signup flow; expose them in the personal-key setup where the choice changes the customer's bill.

## Why 60 hours can work at $4.99

Current official prices are $0.04/hour for Groq Whisper Large v3 Turbo and $0.111/hour for Large v3. Groq bills at least ten seconds per request. Its listed GPT-OSS 20B rates are $0.075/million input tokens and $0.30/million output tokens; GPT-OSS 120B costs twice those rates. These are provider prices checked on the review date, not permanent guarantees. [Groq speech pricing and billing minimum](https://console.groq.com/docs/speech-to-text), [Groq model pricing](https://console.groq.com/docs/models).

Lemon Squeezy's standard fee is 5% + $0.50, with another 0.5% for subscriptions and 1.5% for international transactions. Some payment methods, marketing features, taxes and payouts change the result. The example below includes a 1% non-US bank payout fee and excludes sales tax, PayPal and affiliate fees. [Lemon Squeezy fees](https://docs.lemonsqueezy.com/help/getting-started/fees).

| Example per fully used monthly subscription | USD |
| --- | ---: |
| Selling price | 4.9900 |
| Platform/subscription/international fee: $0.50 + 7% × $4.99 | -0.8493 |
| Example non-US bank payout: 1% of remaining amount | -0.0414 |
| Available before service costs | 4.0993 |
| 60 billable hours of Turbo | -2.4000 |
| Example bounded cleanup budget: 3M input + 2M output tokens with GPT-OSS 20B | -0.8250 |
| Remaining before hosting, email, support, free users, refunds and tax | 0.8743 |

The cleanup budget above is a proposed enforceable ceiling, not measured customer usage. Completion accounting must include billable reasoning tokens. A representative, less demanding estimate is 150 words/minute, two tokens/word, one cleanup pass, and 3,600 calls with 150 extra prompt tokens each: 1.62M input + 1.08M output tokens cost approximately $0.4455 with 20B. Languages, prompts, reasoning and retry behavior can change this materially. Backward-compatible compressed uploads cannot be sized reliably before inference by the current service; one oversized legacy upload can exceed the reservation before being rejected and recorded. Therefore the table is a planning scenario, not a strict total-spend guarantee for legacy clients.

The maximum-use margin is modest. Typical usage below the allowance provides the room to operate; monitor real costs before promising larger limits. Sixty hours of Large v3 would cost $6.66 before cleanup or payment fees, so that model cannot share this flat allowance at this price. Recommended hosted defaults are Turbo and GPT-OSS 20B. Keep expensive alternatives available with personal keys; do not silently substitute an expensive provider during an outage.

For a defensible allowance:

- Count at least ten billable seconds per provider request; otherwise many tiny recordings defeat the cost estimate. Explain the minimum in detailed plan terms.
- Reserve quota atomically before calling the provider and settle with provider-reported duration. Reject new work if quota cannot be read. Serialize requests per license/account and bound stale reservations.
- Recommend a ten-minute recording/request limit and 12,000 requests per rolling 30 days. A request limit also bounds repeated failures and overhead; it is not extra audio entitlement.
- Limit input length, instructions, output tokens and cleanup requests. A hard monthly token budget gives a real cost ceiling; per-request limits alone do not establish the $0.825 monthly bound.
- Check the purchased Lemon Squeezy store/product/variant on the server, not only `valid: true`. An unrelated valid license must not unlock hosted inference. [License API validation guidance](https://docs.lemonsqueezy.com/guides/tutorials/license-keys).

At the starting revision, Pro transcription used Groq's returned duration, which is better than trusting an incoming duration field. Its quota read and later usage write were separate, quota failures could allow work, cleanup had no completion-token limit, and server license validation did not check product identity. These were review findings to address before relying on the larger allowance; they are not statements about the final implementation of this iteration.

The backend changes prepared in this iteration add atomic reservations, fail-closed quota checks, a single active call per license, the rolling audio/request/token allowances above, product ownership checks, fixed hosted Turbo/20B models and bounded cleanup requests. Completed transcription can immediately proceed to cleanup. Uncertain failures retain their reservation instead of assuming the provider billed nothing. Current PCM uploads are sized before inference; compressed legacy uploads reserve ten minutes, then their actual duration is recorded even when an oversized result is rejected. This preserves compatibility but leaves the oversized-legacy-request limitation described above.

Local validation passed six Deno tests, type checks for all three Edge Functions, all migrations and free/Pro quota assertions on an isolated PostgreSQL 14 database, and a real two-connection reservation race where exactly one caller obtained a reservation. Subsequently, the Pro migration and updated handlers were deployed to the official project. Independent live rollback quota assertions, RPC/table access checks, client-role RLS tests and invalid-auth HTTP checks passed. No new valid-license provider call was made during those checks. Verified ownership IDs are store `447162`, product `1340872` and variant `2094776`; the public checkout reports $4.99/month with no trial. See [the current configuration](AUTH_AND_CLOUD.md) and [verification record](TESTING.md).

## Onboarding scope

For Dictámelo, keep a shorter, repeatable wizard behind the requested visible **Onboarding** button: choose one of the three modes; configure an account or a provider key as appropriate; select language/shortcut and review microphone/accessibility permissions; try a short dictation. Preserve settings when reopening or cancelling. Do not add meeting recording, notes, assistants, calendars or a large local-model download system to imitate a larger product.

## Small provider inventory

| Priority | Provider/models | Recommendation for this initial version |
| --- | --- | --- |
| Keep | Groq: Whisper Large v3 Turbo; Large v3 with personal keys | Already the validated transcription path. Use Turbo for predictable hosted costs. |
| First alternative | OpenAI: `gpt-4o-mini-transcribe`, `gpt-4o-transcribe`; retain `whisper-1` compatibility | These already exist in `src-tauri/src/transcription/openai.rs`, initially marked unverified. Make the option discoverable and verify real transcription before marking it tested. No new provider architecture is needed. |
| Next small addition | Mistral: `voxtral-mini-latest` / Voxtral Mini Transcribe 2 | A useful independent multilingual engine. Add only a small dedicated adapter and real provider tests when its key is available. |
| Later | Local Whisper or Parakeet | Valuable for privacy/offline operation, but entails model downloads, storage, hardware behavior and runtime packaging. Defer to a separate scoped iteration. |

OpenAI estimates $0.003/minute for Mini Transcribe and $0.006/minute for full Transcribe, equivalent to about $0.18/$0.36 per hour. Keep these as personal-key options under the proposed hosted pricing. [OpenAI transcription pricing](https://developers.openai.com/api/docs/pricing).

Mistral's transcription endpoint accepts multipart audio and supports Spanish among its thirteen listed languages. It has its own parameters, including `context_bias`; it is not a guaranteed drop-in replacement for every OpenAI request field. [Mistral speech overview](https://docs.mistral.ai/studio/audio/speech_to_text), [transcription endpoint usage](https://docs.mistral.ai/studio/audio/speech_to_text/offline_transcription).

## Public application, private credentials

Supabase Auth can support email/password and Google in an open-source desktop app. Public client code is expected. The Supabase URL and publishable/legacy anon key identify the service; a user's verified token and database permissions authorize access. Service-role/secret keys bypass normal client restrictions and must remain on the server. Supabase recommends publishable keys for new client integrations; legacy-key migration should preserve released clients. [Supabase key types](https://supabase.com/docs/guides/getting-started/api-keys).

The Google client ID is public configuration. The Google client secret belongs in Supabase's provider configuration, never in the shipped app. Use an external browser and PKCE for desktop OAuth, restrict callback URLs and request only identity scopes. [Supabase Google setup](https://supabase.com/docs/guides/auth/social-login/auth-google).

The native implementation in this iteration starts OAuth through Supabase, so it does not need a Google client ID or secret in the executable. The Google Web client's authorized callback is the Supabase `/auth/v1/callback` URL. Supabase's desktop redirect allowlist must match `http://127.0.0.1:*/auth/callback/**` for the app's ephemeral loopback port and nonce path. Email tokens remain for account confirmation and password recovery; ordinary email login uses the password.

| Safe to make public | Must remain outside source and app bundles |
| --- | --- |
| Login UI, auth client, backend source, migrations, permission rules | Provider account secrets and SMTP passwords |
| Supabase URL and publishable/anon key | Supabase service-role/secret keys and Management API tokens |
| Google client ID and registered public callback URL | Google client secret |
| Checkout URL/product ID and updater public verification key | Lemon Squeezy management/webhook secrets and updater private signing key |
| Configurable endpoint names and example configuration | Apple credentials and individual users' passwords/session tokens |

Private repositories do not make bundled secrets private: binaries can be inspected too. Protection comes from keeping privileged credentials on the server and enforcing identity, quotas and permissions there.

The recommended initial arrangement is one real public Dictámelo application with optional, configurable cloud endpoints. The reusable server functions can also remain public while Supabase stores their deployment secrets. Forks should configure their own service or use personal keys; their default build should not accidentally inherit the official cloud, checkout, signing identity or update channel.

The user-requested OpenLivery comparison does demonstrate a valid optional wrapper: its private cloud repository consumes the public application through a pinned `core/` Git submodule. Read-only inspection confirmed that the submodule referenced the same public commit as the public repository's head on the review date, and its container build used that core. The pattern preserves one source of truth; it does not require a second editable copy of the app. No private operational implementation is reproduced here.

If a private `dictamelo_cloud` operations repository becomes useful, keep it this small:

```text
dictamelo_cloud/          private operations repository
  core/                  pinned submodule → public sarrazola/dictamelo
  deploy/                environment wiring and deployment commands
  docs/                  operator runbooks; no secret values
```

The official build runs against the pinned public `core/` commit with official public configuration injected at build time. Core fixes go to the public repository first; advance the submodule after tests. Secrets stay in Supabase/Keychain/CI secret storage, not in the private Git repository. Avoid copying, patching or overlaying a separate login UI. A private wrapper is an organization choice, not an OAuth requirement.

## Seven-day Pro trial

Lemon Squeezy supports a seven-day trial on subscription products. Its native trial collects payment details and automatically charges when the trial ends unless cancelled. A no-card trial is different: the application's server must track the trial itself and send the customer to checkout later. [Free trial configuration](https://docs.lemonsqueezy.com/help/products/free-trials), [trial integration guide](https://docs.lemonsqueezy.com/guides/tutorials/saas-free-trials).

For account-based entitlement, verify signed subscription webhooks, bind checkout to the authenticated user on the server, and store the subscription's status and trial end. `on_trial` must grant trial access; cancellation can retain access until `ends_at`; expiry must stop it. Keep the existing license route compatible. Show the exact first-charge date and price next to the trial action. [Subscription states](https://docs.lemonsqueezy.com/api/subscriptions/the-subscription-object).

License objects use `inactive`, `active`, `expired` and `disabled`; there is no `on_trial` license status. Published documentation explains that subscription licenses follow subscription expiry, but does not establish exactly when a key is issued for the initial zero-charge trial checkout. Do not assume a key-only integration grants access immediately, and do not claim it cannot without testing. [License object](https://docs.lemonsqueezy.com/api/license-keys/the-license-key-object), [subscription-linked licenses](https://docs.lemonsqueezy.com/help/licensing/license-keys-subscriptions).

Before advertising the trial, run a test-mode checkout and verify immediate entitlement, any license issuance/activation, cancellation during the trial, expiry, successful first payment and failed first payment. If relying only on existing license keys, prove those exact transitions first. Otherwise use the account webhook entitlement route. This research did not change the product or create a paid transaction.

## What “email setup” and “publishing” mean

Supabase remains the identity system. A transactional mail sender is a delivery service used by Supabase for confirmation and password-reset messages. Switching from email codes to email/password does not remove those email needs. Google sign-in avoids an app-sent login code, but it requires the Google provider and callback configuration to work end to end.

Publishing a GitHub release makes its installers and updater manifest available to everyone. It is separate from deploying an Auth configuration or opening a locally installed test build. A locally working login screen is not proof that a new customer can confirm their email, recover a password or return from Google to the desktop app. Verify those paths before declaring the cloud onboarding ready for public release.
