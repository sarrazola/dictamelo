//! Hosted cleanup. Pro uses a license; Free Cloud uses an account JWT and a receipt
//! bound to the original transcription. Provider credentials remain on the server.

use super::{wrap_transcript, CleanerInfo, TextCleaner};
use crate::transcription::openai_compatible::{map_reqwest, map_status};
use crate::transcription::{ModelInfo, TranscriptionError};
use crate::util::truncate;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct DictameloCleaner {
    http: reqwest::Client,
    endpoint: Option<String>,
}

#[derive(Serialize)]
struct CleanupRequest<'a> {
    system: &'a str,
    text: String,
    model: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FreeCleanupRequest<'a> {
    text: &'a str,
    cleanup_receipt: &'a str,
}

#[derive(Deserialize)]
struct CleanupResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

impl DictameloCleaner {
    pub const ID: &'static str = "dictamelo";

    pub fn new(http: reqwest::Client) -> Self {
        Self { http, endpoint: crate::cloud_config::backend_url().ok().map(|base| format!("{base}/cleanup")) }
    }

    fn request(
        &self,
        api_key: Option<&str>,
        model: &str,
        system_prompt: &str,
        text: &str,
        cleanup_receipt: Option<&str>,
    ) -> Result<reqwest::RequestBuilder, TranscriptionError> {
        let endpoint = self.endpoint.as_deref().ok_or_else(|| TranscriptionError::Rejected("Cloud services are not configured in this build. Use your own API key.".into()))?;
        let credential = api_key.map(str::trim).filter(|key| !key.is_empty()).ok_or(TranscriptionError::MissingApiKey)?;
        let builder = self.http.post(endpoint).timeout(Duration::from_secs(65));
        if let Some(token) = credential.strip_prefix("Bearer ") {
            if token.is_empty() { return Err(TranscriptionError::MissingApiKey); }
            let receipt = cleanup_receipt.filter(|value| uuid::Uuid::parse_str(value).is_ok())
                .ok_or_else(|| TranscriptionError::Rejected("Included cleanup is unavailable for this transcription. Your original text was preserved.".into()))?;
            // Do not trim, wrap or rewrite receipt-bound text, and do not send custom
            // model/system instructions. The server owns the Free Cloud cleanup policy.
            Ok(builder.bearer_auth(token).json(&FreeCleanupRequest { text, cleanup_receipt: receipt }))
        } else {
            Ok(builder.header("x-license-key", credential)
                .json(&CleanupRequest { system: system_prompt, text: wrap_transcript(text), model }))
        }
    }
}

#[async_trait]
impl TextCleaner for DictameloCleaner {
    fn info(&self) -> CleanerInfo {
        CleanerInfo {
            id: Self::ID.into(),
            name: "Dictámelo Cloud".into(),
            key_provider: Self::ID.into(),
            default_model: "openai/gpt-oss-20b".into(),
            models: vec![ModelInfo {
                id: "openai/gpt-oss-20b".into(),
                name: "GPT-OSS 20B".into(),
                description: "model.desc.oss20".into(),
            }],
        }
    }

    /// `api_key` contains the captured Pro license or Free Cloud bearer credential.
    async fn clean(
        &self,
        api_key: Option<&str>,
        model: &str,
        system_prompt: &str,
        text: &str,
        cleanup_receipt: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let response = self.request(api_key, model, system_prompt, text, cleanup_receipt)?
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            if let Ok(parsed) = serde_json::from_str::<CleanupResponse>(&body) {
                if let Some(message) = parsed.error {
                    return Err(TranscriptionError::Rejected(message));
                }
            }
            return Err(map_status(status, &body));
        }
        let parsed: CleanupResponse = serde_json::from_str(&body)
            .map_err(|e| TranscriptionError::InvalidResponse(format!("{e}: {}", truncate(&body, 200))))?;
        let content = parsed.choices.into_iter().next().and_then(|c| c.message.content).unwrap_or_default();
        Ok(super::openai_compatible_chat::tidy(&content))
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;

    fn cleaner() -> DictameloCleaner {
        DictameloCleaner { http: reqwest::Client::new(), endpoint: Some("https://example.invalid/cleanup".into()) }
    }

    #[test]
    fn free_request_uses_bearer_and_exact_text_without_pro_instructions() {
        let text = "\u{0085}eh send it Thursday no Friday\u{0085}";
        let receipt = "55740015-ff96-4af8-9323-2d72ce02bc62";
        let request = cleaner().request(Some("Bearer example-token"), "custom-model", "custom instructions", text, Some(receipt)).unwrap().build().unwrap();
        assert_eq!(request.headers()["authorization"], "Bearer example-token");
        assert!(request.headers().get("x-license-key").is_none());
        let body: serde_json::Value = serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({ "text": text, "cleanupReceipt": receipt }));
    }

    #[test]
    fn pro_contract_is_unchanged_and_free_receipts_are_required() {
        let request = cleaner().request(Some("example-license"), "model", "system", " hello ", None).unwrap().build().unwrap();
        assert_eq!(request.headers()["x-license-key"], "example-license");
        assert!(request.headers().get("authorization").is_none());
        let body: serde_json::Value = serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({ "model": "model", "system": "system", "text": "<transcript>\nhello\n</transcript>" }));
        assert!(cleaner().request(Some("Bearer example-token"), "", "", "hello", None).is_err());
        assert!(cleaner().request(Some("Bearer example-token"), "", "", "hello", Some("not-a-receipt")).is_err());
    }
}
