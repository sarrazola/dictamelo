//! Mistral's native file transcription API, including vocabulary context bias.

use super::cloud_catalog::{vocabulary_terms, MISTRAL};
use super::openai_compatible::{audio_part, map_reqwest, map_status, parse_transcription_json};
use super::{
    ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest,
    TranscriptionResult,
};
use async_trait::async_trait;

pub struct MistralProvider {
    http: reqwest::Client,
    endpoint: String,
}

impl MistralProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            endpoint: format!("{}/audio/transcriptions", MISTRAL.base_url),
        }
    }
}

#[async_trait]
impl TranscriptionProvider for MistralProvider {
    fn info(&self) -> ProviderInfo {
        MISTRAL.info()
    }

    async fn transcribe(
        &self,
        api_key: Option<&str>,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let key = api_key
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(TranscriptionError::MissingApiKey)?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", audio_part(&request.audio_path).await?)
            .text("model", request.model.clone());
        if let Some(language) = &request.language {
            form = form.text("language", language.clone());
        }
        // Mistral accepts up to 100 context terms; it does not accept OpenAI's prompt/response_format fields.
        for term in vocabulary_terms(request.prompt.as_deref(), 100) {
            form = form.text("context_bias", term);
        }
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(key)
            .multipart(form)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            return Err(map_status(status, &body));
        }
        parse_transcription_json(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::test_support::{audio_request, http_fixture};

    #[tokio::test]
    async fn native_multipart_sends_vocabulary_and_parses_usage() {
        let (url, captured) = http_fixture(
            200,
            r#"{"text":" Hola, Málaga. ","language":"es","usage":{"prompt_audio_seconds":4}}"#,
        )
        .await;
        let provider = MistralProvider {
            http: reqwest::Client::new(),
            endpoint: format!("{url}/audio/transcriptions"),
        };
        let result = provider
            .transcribe(Some(" test-token "), &audio_request("voxtral-mini-latest"))
            .await
            .unwrap();
        assert_eq!(result.text, "Hola, Málaga.");
        assert_eq!(result.language.as_deref(), Some("es"));
        assert_eq!(result.duration_secs, Some(4.0));
        let request = captured.await.unwrap();
        assert_eq!(request.path, "/audio/transcriptions");
        assert_eq!(request.headers["authorization"], "Bearer test-token");
        let body = String::from_utf8_lossy(&request.body);
        assert!(body.contains("name=\"model\"\r\n\r\nvoxtral-mini-latest\r\n"));
        assert!(body.contains("name=\"language\"\r\n\r\nes\r\n"));
        assert!(body.contains("name=\"context_bias\"\r\n\r\nAcme Corp\r\n"));
        assert_eq!(body.matches("name=\"context_bias\"").count(), 2);
        assert!(!body.contains("name=\"prompt\"") && !body.contains("name=\"response_format\""));
    }

    #[tokio::test]
    async fn provider_rejection_and_invalid_response_are_not_silence() {
        for (status, body) in [
            (422, r#"{"message":"Unknown model"}"#),
            (200, r#"{"error":"bad payload"}"#),
        ] {
            let (url, captured) = http_fixture(status, body).await;
            let provider = MistralProvider {
                http: reqwest::Client::new(),
                endpoint: url,
            };
            let error = provider
                .transcribe(Some("test-token"), &audio_request("voxtral-mini-latest"))
                .await
                .unwrap_err();
            captured.await.unwrap();
            if status == 422 {
                assert!(
                    matches!(error, TranscriptionError::Rejected(message) if message == "Unknown model")
                );
            } else {
                assert!(matches!(error, TranscriptionError::InvalidResponse(_)));
            }
        }
    }
}
