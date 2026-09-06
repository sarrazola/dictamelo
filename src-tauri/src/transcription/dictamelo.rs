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

pub struct DictameloProvider {
    http: reqwest::Client,
    endpoint: Option<String>,
}

#[derive(Deserialize)]
struct BackendResponse {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default, rename = "cleanupReceipt")]
    cleanup_receipt: Option<String>,
    /// Mensaje ya listo para mostrar cuando el servidor rechaza la petición.
    #[serde(default)]
    error: Option<String>,
}

impl DictameloProvider {
    pub const ID: &'static str = "dictamelo";

    pub fn new(http: reqwest::Client) -> Self {
        Self { http, endpoint: crate::cloud_config::backend_url().ok().map(|base| format!("{base}/transcribe")) }
    }
}

#[async_trait]
impl TranscriptionProvider for DictameloProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: Self::ID.into(),
            name: "Dictámelo Cloud".into(),
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
        let endpoint = self.endpoint.as_deref().ok_or_else(|| TranscriptionError::Rejected("Cloud services are not configured in this build. Use your own API key.".into()))?;
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

        let builder = self.http.post(endpoint);
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
        Ok(transcription_result(parsed))
    }
}

fn transcription_result(parsed: BackendResponse) -> TranscriptionResult {
    let raw = parsed.text.unwrap_or_default();
    // Free Cloud returns the canonical text covered by the receipt. Preserve its exact
    // bytes: Rust and JavaScript disagree on a few Unicode whitespace characters.
    let text = if parsed.cleanup_receipt.is_some() { raw } else { raw.trim().to_string() };
    TranscriptionResult {
        text,
        language: parsed.language,
        duration_secs: parsed.duration,
        cleanup_receipt: parsed.cleanup_receipt,
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;

    #[test]
    fn free_receipt_preserves_canonical_text_and_remains_optional_for_pro() {
        let receipt = "55740015-ff96-4af8-9323-2d72ce02bc62";
        let raw = "\u{0085}Please send it Friday.\u{0085}";
        let parsed: BackendResponse = serde_json::from_value(serde_json::json!({
            "text": raw, "cleanupReceipt": receipt, "duration": 5.0
        })).unwrap();
        let result = transcription_result(parsed);
        assert_eq!(result.text, raw);
        assert_eq!(result.cleanup_receipt.as_deref(), Some(receipt));
        let pro: BackendResponse = serde_json::from_str(r#"{"text":"  hello  "}"#).unwrap();
        let result = transcription_result(pro);
        assert_eq!(result.text, "hello");
        assert!(result.cleanup_receipt.is_none());
    }
}
