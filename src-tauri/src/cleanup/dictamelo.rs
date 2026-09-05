//! Limpieza a través de nuestro servidor (plan Pro). Mismo trato que la transcripción:
//! la credencial es la licencia y la clave del proveedor vive solo en el servidor.

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
}

#[async_trait]
impl TextCleaner for DictameloCleaner {
    fn info(&self) -> CleanerInfo {
        CleanerInfo {
            id: Self::ID.into(),
            name: "Dictámelo Pro".into(),
            key_provider: Self::ID.into(),
            default_model: "openai/gpt-oss-20b".into(),
            models: vec![ModelInfo {
                id: "openai/gpt-oss-20b".into(),
                name: "GPT-OSS 20B".into(),
                description: "model.desc.oss20".into(),
            }],
        }
    }

    /// `api_key` es aquí la clave de licencia del usuario.
    async fn clean(
        &self,
        api_key: Option<&str>,
        model: &str,
        system_prompt: &str,
        text: &str,
    ) -> Result<String, TranscriptionError> {
        let endpoint = self.endpoint.as_deref().ok_or_else(|| TranscriptionError::Rejected("Cloud services are not configured in this build. Use your own API key.".into()))?;
        let license = api_key.map(str::trim).filter(|k| !k.is_empty()).ok_or(TranscriptionError::MissingApiKey)?;
        let response = self
            .http
            .post(endpoint)
            .header("x-license-key", license)
            .timeout(Duration::from_secs(45))
            .json(&CleanupRequest { system: system_prompt, text: wrap_transcript(text), model })
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
