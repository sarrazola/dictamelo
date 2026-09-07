# Personal cloud providers

Dictámelo keeps the transcription and cleanup adapters separate. Personal API keys stay in the existing device credential store and requests go directly to the selected provider. Dictámelo Cloud subscriptions, included usage and existing licenses use their existing hosted route.

## Supported catalog

| Provider | Transcription models | Text cleanup models |
| --- | --- | --- |
| Groq | Whisper Large v3 (default), Whisper Large v3 Turbo | GPT-OSS 120B (default), GPT-OSS 20B |
| OpenAI | GPT-4o mini Transcribe (default), GPT-4o Transcribe, Whisper | GPT-4.1 mini (default), GPT-4o mini |
| Mistral | Voxtral Mini Transcribe 2 (`voxtral-mini-latest`) | Mistral Small (`mistral-small-latest`) |
| Deepgram | Nova-3 (default), Nova-2 | Select a separate cleanup provider |

Each cleanup provider uses its own saved API key. Transcription and cleanup can use different providers. Provider charges, language support, availability and account limits are controlled by the provider. Mistral's `latest` aliases may change the served model without a Dictámelo release.

Local transcription does not require any cloud key. Sending its resulting text to a cloud cleaner requires the separate local-cleanup permission; otherwise the text remains local. This is independent of the general cleanup preference.

## Request contracts

- **Groq and OpenAI:** multipart audio uploads to `/audio/transcriptions`. Whisper uses `verbose_json`; GPT transcription models use `json`. The selected language and vocabulary prompt are included when provided.
- **Mistral:** native multipart uploads to `/audio/transcriptions`, with repeated `context_bias` fields for up to 100 unique vocabulary terms. The adapter does not send OpenAI-only `prompt` or `response_format` fields. Context bias is optimized for English; other languages are experimental according to Mistral.
- **Deepgram:** raw audio bytes to `/listen`, using `Authorization: Token`. An explicit language disables automatic language detection. Nova-3 uses repeated `keyterm` parameters; Nova-2 uses `keywords`. The adapter sets `mip_opt_out=true` to exclude requests from Deepgram's Model Improvement Program. Deepgram's March 5, 2026 update says opting out does not change the listed Pay as You Go or Growth rates.
- **Cleanup:** `/chat/completions` with the cleanup instructions in the system message and the dictated text in a separate wrapped user message. Text chunks are supported for Mistral. Refusals, missing results and truncated completions are errors so the caller can retain the original transcript. A valid empty text remains valid for filler-only input.

Never log API keys, authorization headers or raw provider response bodies. Successful transcription response parsing excludes the response body from diagnostic errors.

## Adding a model or provider

1. Verify the official API documentation, account availability, accepted audio formats, supported languages and privacy settings.
2. For a new OpenAI, Mistral or Deepgram model using the same request contract, add its ID, display name and description key to `src-tauri/src/transcription/cloud_catalog.rs`. Groq metadata currently lives in `src-tauri/src/transcription/groq.rs`.
3. For a compatible cleanup model, update `src-tauri/src/cleanup/cloud.rs`. Groq's reasoning option and models remain in `src-tauri/src/cleanup/groq.rs`.
4. Add descriptions in all six interface languages. The UI reads models from the backend registry; do not add a second hard-coded model list.
5. A provider with a different request contract needs its own `TranscriptionProvider` implementation and registration in `ProviderRegistry::with_defaults`. Add its local logo asset and key-management link. A new cleaner implements `TextCleaner` and registers in `CleanerRegistry::with_defaults`.
6. Add loopback contract tests for authentication, audio body, optional parameters, vocabulary and response/error decoding. Keep unsupported parameters out of the request. Test catalog default IDs.
7. Run the relevant Rust and UI checks. With an authorized provider key, separately test the licensed audio fixture against the real service. Record that result honestly; mocked contract tests are not live provider verification.

## Verification boundary

The OpenAI, Mistral and Deepgram adapters have loopback HTTP tests using the bundled licensed English WAV. Those tests inspect actual request bodies and return controlled provider responses. They do not contact paid endpoints or prove that a user's account has model access. Live verification remains a separate step and must not be inferred from the catalog or the presence of a provider logo.

This development pass targets macOS. Windows integration and hardware verification must be completed before publishing matching Windows installers.

## Official references

- [OpenAI speech-to-text](https://developers.openai.com/api/docs/guides/speech-to-text)
- [OpenAI GPT-4.1 mini](https://developers.openai.com/api/docs/models/gpt-4.1-mini)
- [Mistral file transcription and context bias](https://docs.mistral.ai/studio/audio/speech_to_text/offline_transcription)
- [Mistral chat completions](https://docs.mistral.ai/studio/conversations/chat-completion)
- [Deepgram prerecorded API](https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded)
- [Deepgram language detection](https://developers.deepgram.com/docs/language-detection)
- [Deepgram Model Improvement Program](https://developers.deepgram.com/docs/the-deepgram-model-improvement-partnership-program)
- [Deepgram opt-out pricing update](https://developers.deepgram.com/changelog/2026/3/5)

API contracts checked September 7, 2026. Recheck provider documentation when extending the catalog.
