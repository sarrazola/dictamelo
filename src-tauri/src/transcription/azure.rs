//! Microsoft MAI-Transcribe-2 through Azure Speech's synchronous REST API.
//! Resource URLs are user settings; subscription keys remain in the secure store.

use super::cloud_catalog::{model_info, vocabulary_terms};
use super::openai_compatible::{audio_part, map_reqwest};
use super::{
    ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest,
    TranscriptionResult,
};
use async_trait::async_trait;
use reqwest::{header::HeaderValue, Url};
use serde::Deserialize;
use std::time::Duration;

pub const MODEL: &str = "MAI-Transcribe-2";
const API_PATH: &str = "/speechtotext/transcriptions:transcribe?api-version=2025-10-15";

/// Accept only a bare HTTPS Azure resource origin. Never send a key to an arbitrary
/// server, embedded URL, unexpected port, query string or redirect destination.
pub fn transcription_url(endpoint: &str) -> Result<Url, TranscriptionError> {
    let mut url = Url::parse(endpoint.trim()).map_err(|_| TranscriptionError::InvalidEndpoint)?;
    let host = url.host_str().ok_or(TranscriptionError::InvalidEndpoint)?;
    let resource = host
        .strip_suffix(".cognitiveservices.azure.com")
        .ok_or(TranscriptionError::InvalidEndpoint)?;
    if url.scheme() != "https"
        || resource.is_empty()
        || resource.len() > 63
        || !resource
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || resource.starts_with('-')
        || resource.ends_with('-')
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(TranscriptionError::InvalidEndpoint);
    }
    url.set_path(API_PATH.split('?').next().unwrap());
    url.set_query(Some("api-version=2025-10-15"));
    Ok(url)
}

pub struct AzureSpeechProvider {
    http: reqwest::Client,
}

impl Default for AzureSpeechProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AzureSpeechProvider {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("Dictamelo/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(180))
                // A subscription-key header must never follow an HTTP redirect.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("valid Azure HTTP client"),
        }
    }

    async fn send(
        &self,
        endpoint: Url,
        key: &str,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        if request.model != MODEL {
            return Err(TranscriptionError::Rejected(
                "Unsupported Azure Speech model".into(),
            ));
        }
        let mut credential =
            HeaderValue::from_str(key).map_err(|_| TranscriptionError::Unauthorized)?;
        credential.set_sensitive(true);
        let mut definition = serde_json::json!({"enhancedMode": {
            "enabled": true, "model": MODEL,
            "modelOptions": {"transcribeStyle": "verbatim", "timestamps": "word"}
        }});
        if let Some(language) = request.language.as_deref() {
            definition["locales"] = serde_json::json!([language]);
        }
        let vocabulary = vocabulary_terms(request.prompt.as_deref(), 100);
        if !vocabulary.is_empty() {
            definition["phraseList"] = serde_json::json!({"phrases": vocabulary});
        }
        let form = reqwest::multipart::Form::new()
            .part("audio", audio_part(&request.audio_path).await?)
            .text("definition", definition.to_string());
        let response = self
            .http
            .post(endpoint)
            .header("Ocp-Apim-Subscription-Key", credential)
            .multipart(form)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            // Azure error bodies can contain submitted text or resource metadata.
            // Keep them out of UI errors and logs, including malformed proxy errors.
            return Err(match status {
                401 | 403 => TranscriptionError::Unauthorized,
                429 => TranscriptionError::RateLimited,
                500..=599 => TranscriptionError::Server {
                    status,
                    message: "Azure Speech service error".into(),
                },
                _ => TranscriptionError::Rejected(format!("Azure Speech HTTP {status}")),
            });
        }
        parse_response(&response.text().await.map_err(map_reqwest)?)
    }
}

#[async_trait]
impl TranscriptionProvider for AzureSpeechProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "azure".into(),
            name: "Microsoft Azure".into(),
            requires_api_key: true,
            key_url: "https://portal.azure.com/".into(),
            default_model: MODEL.into(),
            models: model_info(&[(MODEL, MODEL, "model.desc.mai_transcribe_2")]),
            verified: true,
        }
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
        let endpoint = transcription_url(request.endpoint.as_deref().unwrap_or_default())?;
        self.send(endpoint, key, request).await
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzureResponse {
    combined_phrases: Vec<CombinedPhrase>,
    duration_milliseconds: f64,
}

#[derive(Deserialize)]
struct CombinedPhrase {
    text: String,
}

fn parse_response(body: &str) -> Result<TranscriptionResult, TranscriptionError> {
    let parsed: AzureResponse = serde_json::from_str(body)
        .map_err(|_| TranscriptionError::InvalidResponse("Invalid Azure Speech response".into()))?;
    if !parsed.duration_milliseconds.is_finite() || parsed.duration_milliseconds < 0.0 {
        return Err(TranscriptionError::InvalidResponse(
            "Invalid Azure audio duration".into(),
        ));
    }
    Ok(TranscriptionResult {
        text: parsed
            .combined_phrases
            .iter()
            .map(|p| p.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        language: None,
        duration_secs: Some(parsed.duration_milliseconds / 1000.0),
        cleanup_receipt: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::test_support::{audio_request, http_fixture};

    #[test]
    fn endpoint_cannot_leak_subscription_keys_to_other_origins() {
        assert_eq!(
            transcription_url(" https://example.cognitiveservices.azure.com/ ")
                .unwrap()
                .as_str(),
            format!("https://example.cognitiveservices.azure.com{API_PATH}")
        );
        for endpoint in [
            "",
            "http://example.cognitiveservices.azure.com",
            "https://evil.test",
            "https://example.cognitiveservices.azure.com.evil.test",
            "https://nested.example.cognitiveservices.azure.com",
            "https://example.cognitiveservices.azure.com:444",
            "https://key@example.cognitiveservices.azure.com",
            "https://example.cognitiveservices.azure.com/path",
            "https://example.cognitiveservices.azure.com/?key=secret",
            "https://example.cognitiveservices.azure.com/#fragment",
        ] {
            assert!(
                matches!(
                    transcription_url(endpoint),
                    Err(TranscriptionError::InvalidEndpoint)
                ),
                "{endpoint}"
            );
        }
    }

    #[tokio::test]
    async fn sends_native_mai_contract_and_preserves_combined_text_once() {
        let (url, captured) = http_fixture(200, r#"{"durationMilliseconds":5860,"combinedPhrases":[{"text":" Hola, Málaga. "}],"phrases":[{"text":"Do not duplicate this"}]}"#).await;
        let mut request = audio_request(MODEL);
        request.language = None;
        let result = AzureSpeechProvider::new()
            .send(Url::parse(&url).unwrap(), "test-token", &request)
            .await
            .unwrap();
        assert_eq!(result.text, "Hola, Málaga.");
        assert_eq!(result.duration_secs, Some(5.86));
        let sent = captured.await.unwrap();
        assert_eq!(sent.headers["ocp-apim-subscription-key"], "test-token");
        assert!(!sent.headers.contains_key("authorization"));
        let body = String::from_utf8_lossy(&sent.body);
        assert!(body.contains("name=\"audio\""));
        assert!(body.contains("name=\"definition\""));
        assert!(body.contains("\"model\":\"MAI-Transcribe-2\""));
        assert!(body.contains("\"transcribeStyle\":\"verbatim\""));
        assert!(body.contains("\"timestamps\":\"word\""));
        assert!(body.contains("\"phrases\":[\"Málaga\",\"Acme Corp\"]"));
        assert!(!body.contains("locales") && !body.contains("response_format"));
    }

    #[tokio::test]
    async fn honors_an_explicit_language_and_treats_http_failures_as_errors() {
        for status in [302, 400, 401, 403, 429, 500] {
            let (url, captured) =
                http_fixture(status, r#"{"error":{"message":"never echo secrets"}}"#).await;
            let error = AzureSpeechProvider::new()
                .send(
                    Url::parse(&url).unwrap(),
                    "test-token",
                    &audio_request(MODEL),
                )
                .await
                .unwrap_err();
            let sent = captured.await.unwrap();
            assert!(String::from_utf8_lossy(&sent.body).contains("\"locales\":[\"es\"]"));
            assert!(!error.to_string().contains("never echo"));
            assert_eq!(error.is_retryable(), status == 429 || status >= 500);
        }
    }

    #[test]
    fn silence_is_valid_but_missing_or_malformed_results_are_not() {
        assert!(
            parse_response(r#"{"durationMilliseconds":1000,"combinedPhrases":[]}"#)
                .unwrap()
                .text
                .is_empty()
        );
        for body in [
            "{}",
            "not-json",
            r#"{"durationMilliseconds":2,"combinedPhrases":[{}]}"#,
            r#"{"durationMilliseconds":-1,"combinedPhrases":[]}"#,
        ] {
            assert!(parse_response(body).is_err());
        }
    }

    #[tokio::test]
    async fn never_follows_redirects_with_a_subscription_key() {
        let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = format!("http://{}/capture", destination.local_addr().unwrap());
        let (url, captured) = super::super::test_support::http_fixture_with_headers(
            307,
            "",
            &[("Location", &target)],
        )
        .await;
        let error = AzureSpeechProvider::new()
            .send(
                Url::parse(&url).unwrap(),
                "test-token",
                &audio_request(MODEL),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, TranscriptionError::Rejected(_)));
        captured.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), destination.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn incomplete_configuration_never_calls_a_different_model_or_provider() {
        let provider = AzureSpeechProvider::new();
        let request = audio_request(MODEL);
        assert!(matches!(
            provider.transcribe(None, &request).await,
            Err(TranscriptionError::MissingApiKey)
        ));
        assert!(matches!(
            provider.transcribe(Some("test-token"), &request).await,
            Err(TranscriptionError::InvalidEndpoint)
        ));
        assert!(matches!(
            provider
                .send(
                    Url::parse("http://127.0.0.1:1").unwrap(),
                    "test-token",
                    &audio_request("other-model")
                )
                .await,
            Err(TranscriptionError::Rejected(_))
        ));
    }

    #[tokio::test]
    #[ignore = "requires explicit Azure live opt-in and credentials"]
    async fn azure_live_licensed_speech() {
        assert_eq!(
            std::env::var("DICTAMELO_TEST_AZURE_LIVE").as_deref(),
            Ok("1")
        );
        let key = std::env::var("AZURE_SPEECH_KEY").expect("Azure key in environment");
        let mut request = audio_request(MODEL);
        request.endpoint =
            Some(std::env::var("AZURE_SPEECH_ENDPOINT").expect("Azure endpoint in environment"));
        request.language = None;
        let result = AzureSpeechProvider::new()
            .transcribe(Some(&key), &request)
            .await
            .unwrap();
        assert!(result.text.to_lowercase().contains("quilter"));
        assert!(result.text.to_lowercase().contains("gospel"));
        assert!(result
            .duration_secs
            .is_some_and(|secs| (5.0..7.0).contains(&secs)));
    }
}
