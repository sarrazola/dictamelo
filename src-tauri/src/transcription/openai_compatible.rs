//! Cliente genérico para la API `POST /audio/transcriptions` compatible con OpenAI
//! (la usan Groq, OpenAI y otros). Los proveedores concretos solo fijan la URL base,
//! los modelos y el formato de respuesta.

use super::{TranscriptionError, TranscriptionRequest, TranscriptionResult};
use crate::util::truncate;
use reqwest::multipart::{Form, Part};
use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    /// `{"text": ...}`
    Json,
    /// `{"text": ..., "language": ..., "duration": ...}` (solo modelos Whisper).
    VerboseJson,
}

impl ResponseFormat {
    fn as_str(self) -> &'static str {
        match self {
            ResponseFormat::Json => "json",
            ResponseFormat::VerboseJson => "verbose_json",
        }
    }
}

pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Deserialize)]
struct TranscriptionJson {
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    usage: Option<TranscriptionUsage>,
}

#[derive(Deserialize)]
struct TranscriptionUsage {
    #[serde(default)]
    prompt_audio_seconds: Option<f64>,
}

impl OpenAiCompatibleClient {
    pub fn new(http: reqwest::Client, base_url: &str) -> Self {
        Self {
            http,
            endpoint: format!("{}/audio/transcriptions", base_url.trim_end_matches('/')),
        }
    }

    pub async fn transcribe(
        &self,
        api_key: &str,
        request: &TranscriptionRequest,
        format: ResponseFormat,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let audio = audio_part(&request.audio_path).await?;

        let mut form = Form::new()
            .part("file", audio)
            .text("model", request.model.clone())
            .text("response_format", format.as_str())
            .text("temperature", "0");
        if let Some(lang) = &request.language {
            form = form.text("language", lang.clone());
        }
        if let Some(prompt) = &request.prompt {
            form = form.text("prompt", prompt.clone());
        }

        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key)
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

pub(crate) async fn audio_part(path: &std::path::Path) -> Result<Part, TranscriptionError> {
    let bytes = tokio::fs::read(path).await?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio.wav".into());
    Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime_for(path))
        .map_err(|e| TranscriptionError::InvalidResponse(e.to_string()))
}

pub(crate) fn parse_transcription_json(
    body: &str,
) -> Result<TranscriptionResult, TranscriptionError> {
    let parsed: TranscriptionJson = serde_json::from_str(body)
        .map_err(|e| TranscriptionError::InvalidResponse(e.to_string()))?;
    Ok(TranscriptionResult {
        text: parsed.text.trim().to_string(),
        language: parsed.language,
        duration_secs: parsed
            .duration
            .or_else(|| parsed.usage.and_then(|usage| usage.prompt_audio_seconds)),
        cleanup_receipt: None,
    })
}

/// Tipo MIME según la extensión (el proveedor lo usa como pista para decodificar).
pub(crate) fn mime_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp3") | Some("mpga") | Some("mpeg") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("mp4") => "video/mp4",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("webm") => "audio/webm",
        _ => "audio/wav",
    }
}

pub(crate) fn map_reqwest(e: reqwest::Error) -> TranscriptionError {
    if e.is_timeout() {
        TranscriptionError::Timeout
    } else if e.is_connect() {
        TranscriptionError::Network("no se pudo conectar; revisa tu conexión a internet".into())
    } else {
        // `without_url` evita filtrar parámetros de la URL en los mensajes.
        TranscriptionError::Network(e.without_url().to_string())
    }
}

pub(crate) fn map_status(status: StatusCode, body: &str) -> TranscriptionError {
    let message = extract_error_message(body);
    match status.as_u16() {
        401 | 403 => TranscriptionError::Unauthorized,
        429 => TranscriptionError::RateLimited,
        400 | 404 | 413 | 415 | 422 => TranscriptionError::Rejected(message),
        code => TranscriptionError::Server {
            status: code,
            message,
        },
    }
}

/// Supports the documented OpenAI, Mistral and Deepgram error envelopes.
fn extract_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message").or(Some(e)))
                .or_else(|| v.get("message"))
                .or_else(|| v.get("err_msg"))
                .or_else(|| v.get("detail"))
                .and_then(|m| m.as_str().map(String::from))
        })
        .unwrap_or_else(|| truncate(body.trim(), 200))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes() {
        assert!(matches!(
            map_status(StatusCode::UNAUTHORIZED, ""),
            TranscriptionError::Unauthorized
        ));
        assert!(matches!(
            map_status(StatusCode::TOO_MANY_REQUESTS, ""),
            TranscriptionError::RateLimited
        ));
        assert!(matches!(
            map_status(StatusCode::BAD_GATEWAY, "x"),
            TranscriptionError::Server { status: 502, .. }
        ));
        match map_status(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"modelo no existe","type":"invalid"}}"#,
        ) {
            TranscriptionError::Rejected(m) => assert_eq!(m, "modelo no existe"),
            other => panic!("inesperado: {other:?}"),
        }
    }

    #[test]
    fn error_message_falls_back_to_body() {
        assert_eq!(extract_error_message("Bad Gateway"), "Bad Gateway");
        assert_eq!(
            extract_error_message(r#"{"error":"texto plano"}"#),
            "texto plano"
        );
        assert_eq!(
            extract_error_message(r#"{"message":"Invalid model"}"#),
            "Invalid model"
        );
        assert_eq!(
            extract_error_message(r#"{"err_code":"BAD_REQUEST","err_msg":"Unsupported language"}"#),
            "Unsupported language"
        );
    }

    #[test]
    fn transcription_requires_text_without_echoing_raw_provider_body() {
        let error = parse_transcription_json(r#"{"content":"private transcript"}"#).unwrap_err();
        assert!(matches!(error, TranscriptionError::InvalidResponse(_)));
        assert!(!error.to_string().contains("private transcript"));
        assert_eq!(parse_transcription_json(r#"{"text":""}"#).unwrap().text, "");
    }
}
