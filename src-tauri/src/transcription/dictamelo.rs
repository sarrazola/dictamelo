//! Transcripción a través de nuestro servidor (plan Pro).
//!
//! El usuario no pone ninguna API key: la credencial es su clave de licencia, que viaja en una
//! cabecera. El servidor la valida y llama al proveedor real con nuestra clave, que nunca sale
//! de allí. Por eso este proveedor no aparece en la lista que elige el usuario.

use super::openai_compatible::{map_reqwest, map_status};
use super::{
    ModelInfo, ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest,
    TranscriptionResult,
};
use crate::util::truncate;
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

/// Funciones de borde del proyecto de Supabase.
pub const BACKEND_URL: &str = "https://iburiyhhfodndqgmsaot.supabase.co/functions/v1";

pub struct DictameloProvider {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Deserialize)]
struct BackendResponse {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    /// Mensaje ya listo para mostrar cuando el servidor rechaza la petición.
    #[serde(default)]
    error: Option<String>,
}

impl DictameloProvider {
    pub const ID: &'static str = "dictamelo";

    pub fn new(http: reqwest::Client) -> Self {
        Self { http, endpoint: format!("{BACKEND_URL}/transcribe") }
    }
}

#[async_trait]
impl TranscriptionProvider for DictameloProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: Self::ID.into(),
            name: "Dictámelo Pro".into(),
            requires_api_key: false,
            key_url: "https://dictamelo.com".into(),
            default_model: "whisper-large-v3-turbo".into(),
            verified: true,
            models: vec![ModelInfo {
                id: "whisper-large-v3-turbo".into(),
                name: "Whisper Large v3 Turbo".into(),
                description: "model.desc.whisper_turbo".into(),
            }],
        }
    }

    /// `api_key` es aquí la clave de licencia del usuario.
    async fn transcribe(
        &self,
        api_key: Option<&str>,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let license = api_key.map(str::trim).filter(|k| !k.is_empty()).ok_or(TranscriptionError::MissingApiKey)?;
        let bytes = tokio::fs::read(&request.audio_path).await?;
        let file_name = request
            .audio_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio.wav".into());
        let audio = Part::bytes(bytes)
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|e| TranscriptionError::InvalidResponse(e.to_string()))?;

        let mut form = Form::new().part("file", audio).text("model", request.model.clone());
        if let Some(lang) = &request.language {
            form = form.text("language", lang.clone());
        }
        if let Some(prompt) = &request.prompt {
            form = form.text("prompt", prompt.clone());
        }

        let builder = self.http.post(&self.endpoint);
        let builder = if let Some(token) = license.strip_prefix("Bearer ") { builder.bearer_auth(token) } else { builder.header("x-license-key", license) };
        let response = builder
            .multipart(form)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            // El servidor ya devuelve un mensaje pensado para el usuario; se respeta.
            if let Ok(parsed) = serde_json::from_str::<BackendResponse>(&body) {
                if let Some(message) = parsed.error {
                    return Err(TranscriptionError::Rejected(message));
                }
            }
            return Err(map_status(status, &body));
        }

        let parsed: BackendResponse = serde_json::from_str(&body)
            .map_err(|e| TranscriptionError::InvalidResponse(format!("{e}: {}", truncate(&body, 200))))?;
        Ok(TranscriptionResult {
            text: parsed.text.unwrap_or_default().trim().to_string(),
            language: parsed.language,
            duration_secs: parsed.duration,
        })
    }
}
