//! OpenAI file transcription. GPT transcription models require JSON, while Whisper supports verbose JSON.

use super::cloud_catalog::OPENAI;
use super::openai_compatible::{OpenAiCompatibleClient, ResponseFormat};
use super::{
    ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest,
    TranscriptionResult,
};
use async_trait::async_trait;

pub const OPENAI_BASE_URL: &str = OPENAI.base_url;

pub struct OpenAiProvider {
    client: OpenAiCompatibleClient,
}

impl OpenAiProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            client: OpenAiCompatibleClient::new(http, OPENAI_BASE_URL),
        }
    }
}

#[async_trait]
impl TranscriptionProvider for OpenAiProvider {
    fn info(&self) -> ProviderInfo {
        OPENAI.info()
    }

    async fn transcribe(
        &self,
        api_key: Option<&str>,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let key = api_key
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or(TranscriptionError::MissingApiKey)?;
        // Do not send verbose_json to the GPT transcription models.
        let format = if request.model.starts_with("whisper") {
            ResponseFormat::VerboseJson
        } else {
            ResponseFormat::Json
        };
        self.client.transcribe(key, request, format).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::test_support::{audio_request, http_fixture};

    #[tokio::test]
    async fn sends_each_model_with_its_supported_response_format() {
        for &(model, _, _) in OPENAI.models {
            let (url, captured) = http_fixture(
                200,
                r#"{"text":"  Hola, Málaga.  ","language":"es","duration":4.2}"#,
            )
            .await;
            let provider = OpenAiProvider {
                client: OpenAiCompatibleClient::new(reqwest::Client::new(), &url),
            };
            let result = provider
                .transcribe(Some(" test-token "), &audio_request(model))
                .await
                .unwrap();
            assert_eq!(result.text, "Hola, Málaga.");
            assert_eq!(result.duration_secs, Some(4.2));
            let request = captured.await.unwrap();
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, "/audio/transcriptions");
            assert_eq!(request.headers["authorization"], "Bearer test-token");
            let body = String::from_utf8_lossy(&request.body);
            assert!(body.contains(&format!("name=\"model\"\r\n\r\n{model}\r\n")));
            let expected = if model == "whisper-1" {
                "verbose_json"
            } else {
                "json"
            };
            assert!(body.contains(&format!("name=\"response_format\"\r\n\r\n{expected}\r\n")));
            assert!(body.contains("name=\"language\"\r\n\r\nes\r\n"));
            assert!(body.contains("name=\"prompt\"\r\n\r\nMálaga, Acme Corp; Málaga\r\n"));
        }
    }
}
