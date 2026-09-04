//! Proveedor OpenAI. Comparte el cliente genérico con Groq; solo cambian URL y modelos.
//! Nota: incluido para demostrar la extensibilidad; NO se probó de extremo a extremo.

use super::openai_compatible::{OpenAiCompatibleClient, ResponseFormat};
use super::{ModelInfo, ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest, TranscriptionResult};
use async_trait::async_trait;

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    client: OpenAiCompatibleClient,
}

impl OpenAiProvider {
    pub const ID: &'static str = "openai";

    pub fn new(http: reqwest::Client) -> Self {
        Self { client: OpenAiCompatibleClient::new(http, OPENAI_BASE_URL) }
    }
}

#[async_trait]
impl TranscriptionProvider for OpenAiProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: Self::ID.into(),
            name: "OpenAI".into(),
            requires_api_key: true,
            key_url: "https://platform.openai.com/api-keys".into(),
            default_model: "gpt-4o-mini-transcribe".into(),
            verified: false,
            models: vec![
                ModelInfo {
                    id: "gpt-4o-mini-transcribe".into(),
                    name: "GPT-4o mini Transcribe".into(),
                    description: "Rápido y económico".into(),
                },
                ModelInfo {
                    id: "gpt-4o-transcribe".into(),
                    name: "GPT-4o Transcribe".into(),
                    description: "Mayor precisión".into(),
                },
                ModelInfo {
                    id: "whisper-1".into(),
                    name: "Whisper".into(),
                    description: "Modelo clásico".into(),
                },
            ],
        }
    }

    async fn transcribe(
        &self,
        api_key: Option<&str>,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let key = api_key.map(str::trim).filter(|k| !k.is_empty()).ok_or(TranscriptionError::MissingApiKey)?;
        // Los modelos gpt-4o-* solo aceptan `json`/`text`; Whisper admite `verbose_json`.
        let format = if request.model.starts_with("whisper") { ResponseFormat::VerboseJson } else { ResponseFormat::Json };
        self.client.transcribe(key, request, format).await
    }
}
