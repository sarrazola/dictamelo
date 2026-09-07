//! Deepgram prerecorded transcription: raw audio bytes, Token authentication and nested results.

use super::cloud_catalog::{vocabulary_terms, DEEPGRAM};
use super::openai_compatible::{map_reqwest, map_status, mime_for};
use super::{
    ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest,
    TranscriptionResult,
};
use async_trait::async_trait;
use serde::Deserialize;

pub struct DeepgramProvider {
    http: reqwest::Client,
    endpoint: String,
}

impl DeepgramProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            endpoint: format!("{}/listen", DEEPGRAM.base_url),
        }
    }
}

#[derive(Deserialize)]
struct DeepgramResponse {
    results: Results,
    metadata: Option<Metadata>,
}
#[derive(Deserialize)]
struct Metadata {
    duration: Option<f64>,
}
#[derive(Deserialize)]
struct Results {
    channels: Vec<Channel>,
}
#[derive(Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
    detected_language: Option<String>,
}
#[derive(Deserialize)]
struct Alternative {
    transcript: String,
}

#[async_trait]
impl TranscriptionProvider for DeepgramProvider {
    fn info(&self) -> ProviderInfo {
        DEEPGRAM.info()
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
        let mut query = vec![
            ("model", request.model.clone()),
            ("smart_format", "true".into()),
            // Keep dictated audio out of the provider's Model Improvement Program.
            ("mip_opt_out", "true".into()),
        ];
        if let Some(language) = &request.language {
            query.push(("language", language.clone()));
        } else {
            query.push(("detect_language", "true".into()));
        }
        // Nova-3 has key terms; Nova-2 uses the older keyword parameter instead.
        let vocabulary_parameter = if request.model.starts_with("nova-3") {
            "keyterm"
        } else {
            "keywords"
        };
        for term in vocabulary_terms(request.prompt.as_deref(), 100) {
            query.push((vocabulary_parameter, term));
        }
        let mut url = reqwest::Url::parse(&self.endpoint)
            .map_err(|error| TranscriptionError::InvalidResponse(error.to_string()))?;
        url.query_pairs_mut().extend_pairs(query);
        let audio = tokio::fs::read(&request.audio_path).await?;
        let response = self
            .http
            .post(url)
            .header(reqwest::header::AUTHORIZATION, format!("Token {key}"))
            .header(reqwest::header::CONTENT_TYPE, mime_for(&request.audio_path))
            .body(audio)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            return Err(map_status(status, &body));
        }
        let parsed: DeepgramResponse = serde_json::from_str(&body)
            .map_err(|error| TranscriptionError::InvalidResponse(error.to_string()))?;
        let channel =
            parsed.results.channels.into_iter().next().ok_or_else(|| {
                TranscriptionError::InvalidResponse("Missing audio channel".into())
            })?;
        let alternative = channel.alternatives.into_iter().next().ok_or_else(|| {
            TranscriptionError::InvalidResponse("Missing transcription alternative".into())
        })?;
        Ok(TranscriptionResult {
            text: alternative.transcript.trim().to_string(),
            language: channel
                .detected_language
                .or_else(|| request.language.clone()),
            duration_secs: parsed.metadata.and_then(|metadata| metadata.duration),
            cleanup_receipt: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::test_support::{audio_request, http_fixture};

    #[tokio::test]
    async fn sends_raw_audio_and_model_specific_vocabulary() {
        for model in ["nova-3", "nova-2"] {
            let (url, captured) = http_fixture(200, r#"{"metadata":{"duration":4.2},"results":{"channels":[{"detected_language":"es","alternatives":[{"transcript":" Hola, Málaga. "}]}]}}"#).await;
            let provider = DeepgramProvider {
                http: reqwest::Client::new(),
                endpoint: format!("{url}/listen"),
            };
            let audio = audio_request(model);
            let result = provider
                .transcribe(Some("test-token"), &audio)
                .await
                .unwrap();
            assert_eq!(result.text, "Hola, Málaga.");
            assert_eq!(result.language.as_deref(), Some("es"));
            assert_eq!(result.duration_secs, Some(4.2));
            let request = captured.await.unwrap();
            assert_eq!(request.headers["authorization"], "Token test-token");
            assert_eq!(request.headers["content-type"], "audio/wav");
            assert_eq!(request.body, std::fs::read(audio.audio_path).unwrap());
            let uri = reqwest::Url::parse(&format!("{url}{}", request.path)).unwrap();
            let query: Vec<_> = uri.query_pairs().collect();
            assert!(query
                .iter()
                .any(|(key, value)| key == "language" && value == "es"));
            assert!(query
                .iter()
                .any(|(key, value)| key == "mip_opt_out" && value == "true"));
            assert!(!query.iter().any(|(key, _)| key == "detect_language"));
            let field = if model == "nova-3" {
                "keyterm"
            } else {
                "keywords"
            };
            assert!(query
                .iter()
                .any(|(key, value)| key == field && value == "Acme Corp"));
        }
    }

    #[tokio::test]
    async fn detects_language_and_rejects_missing_results() {
        let (url, captured) = http_fixture(200, r#"{"results":{"channels":[]}}"#).await;
        let provider = DeepgramProvider {
            http: reqwest::Client::new(),
            endpoint: url,
        };
        let mut request = audio_request("nova-3");
        request.language = None;
        let error = provider
            .transcribe(Some("test-token"), &request)
            .await
            .unwrap_err();
        assert!(matches!(error, TranscriptionError::InvalidResponse(_)));
        assert!(captured
            .await
            .unwrap()
            .path
            .contains("detect_language=true"));
    }
}
