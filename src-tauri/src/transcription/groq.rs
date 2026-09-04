//! Proveedor Groq (API compatible con OpenAI, modelos Whisper).

use super::openai_compatible::{OpenAiCompatibleClient, ResponseFormat};
use super::{ModelInfo, ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest, TranscriptionResult};
use async_trait::async_trait;

pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";

pub struct GroqProvider {
    client: OpenAiCompatibleClient,
}

impl GroqProvider {
    pub const ID: &'static str = "groq";

    pub fn new(http: reqwest::Client) -> Self {
        Self { client: OpenAiCompatibleClient::new(http, GROQ_BASE_URL) }
    }
}

#[async_trait]
impl TranscriptionProvider for GroqProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: Self::ID.into(),
            name: "Groq".into(),
            requires_api_key: true,
            key_url: "https://console.groq.com/keys".into(),
            default_model: "whisper-large-v3-turbo".into(),
            verified: true,
            models: vec![
                ModelInfo {
                    id: "whisper-large-v3-turbo".into(),
                    name: "Whisper Large v3 Turbo".into(),
                    description: "Rápido y multilingüe (recomendado)".into(),
                },
                ModelInfo {
                    id: "whisper-large-v3".into(),
                    name: "Whisper Large v3".into(),
                    description: "Máxima precisión, algo más lento".into(),
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
        self.client.transcribe(key, request, ResponseFormat::VerboseJson).await
    }
}

/// Pruebas reales contra la API de Groq. Se ejecutan solo con `DICTADO_LIVE_TESTS=1`;
/// leen la API key del Llavero y sintetizan una frase en español con `say` (macOS).
#[cfg(all(test, target_os = "macos"))]
mod live_tests {
    use super::*;
    use crate::transcription::shared_http_client;
    use std::process::Command;

    fn keychain_key() -> Option<String> {
        let out = Command::new("security")
            .args(["find-generic-password", "-s", crate::state::KEYCHAIN_SERVICE, "-a", GroqProvider::ID, "-w"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Sintetiza `text` en español y lo deja como WAV mono 16 kHz.
    fn synthesize_spanish(text: &str, wav: &std::path::Path) -> Result<(), String> {
        let aiff = wav.with_extension("aiff");
        let voices = Command::new("say").args(["-v", "?"]).output().map_err(|e| e.to_string())?;
        let voices = String::from_utf8_lossy(&voices.stdout);
        let candidates: Vec<String> = voices
            .lines()
            .filter(|l| l.contains(" es_"))
            .map(|l| l.split("  ").next().unwrap_or("").trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        let mut tried = Vec::new();
        for voice in candidates.iter().chain(std::iter::once(&String::new())) {
            let mut cmd = Command::new("say");
            if !voice.is_empty() {
                cmd.args(["-v", voice]);
            }
            let ok = cmd.args(["-o", aiff.to_str().unwrap(), text]).status().map(|s| s.success()).unwrap_or(false);
            if ok && aiff.exists() {
                let conv = Command::new("afconvert")
                    .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff.to_str().unwrap(), wav.to_str().unwrap()])
                    .status()
                    .map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&aiff);
                if conv.success() {
                    eprintln!("voz usada: {}", if voice.is_empty() { "(predeterminada)" } else { voice });
                    return Ok(());
                }
            }
            tried.push(voice.clone());
        }
        Err(format!("no se pudo sintetizar audio (voces probadas: {tried:?})"))
    }

    fn normalize(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .map(|c| match c {
                'á' => 'a', 'é' => 'e', 'í' => 'i', 'ó' => 'o', 'ú' => 'u', 'ü' => 'u',
                _ => c,
            })
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect()
    }

    #[tokio::test]
    async fn transcribes_spanish_tts_audio() {
        if std::env::var("DICTADO_LIVE_TESTS").is_err() {
            eprintln!("omitido: define DICTADO_LIVE_TESTS=1");
            return;
        }
        let key = keychain_key().expect("API key de Groq en el Llavero");
        let dir = std::env::temp_dir().join(format!("dictado-live-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("frase.wav");
        synthesize_spanish("Hola, esto es una prueba de dictado por voz con Groq.", &wav).unwrap();

        let provider = GroqProvider::new(shared_http_client());
        let request = TranscriptionRequest {
            audio_path: wav.clone(),
            model: "whisper-large-v3-turbo".into(),
            language: Some("es".into()),
            prompt: None,
        };
        let result = provider.transcribe(Some(&key), &request).await.expect("transcripción");
        eprintln!("texto: {:?} idioma: {:?} duración: {:?}", result.text, result.language, result.duration_secs);
        let text = normalize(&result.text);
        assert!(text.contains("prueba"), "texto inesperado: {}", result.text);
        assert!(text.contains("dictado"), "texto inesperado: {}", result.text);

        // Detección automática de idioma con el otro modelo.
        let request = TranscriptionRequest { model: "whisper-large-v3".into(), language: None, ..request };
        let result = provider.transcribe(Some(&key), &request).await.expect("transcripción v3");
        eprintln!("v3 texto: {:?} idioma: {:?}", result.text, result.language);
        assert!(normalize(&result.text).contains("prueba"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn invalid_key_is_reported_as_unauthorized() {
        if std::env::var("DICTADO_LIVE_TESTS").is_err() {
            return;
        }
        let dir = std::env::temp_dir();
        let wav = crate::audio::wav::new_temp_path(&dir);
        crate::audio::wav::write_wav_mono_i16(&wav, &vec![0i16; 16_000], 16_000).unwrap();
        let provider = GroqProvider::new(shared_http_client());
        let request = TranscriptionRequest { audio_path: wav.clone(), model: "whisper-large-v3-turbo".into(), language: None, prompt: None };
        let err = provider.transcribe(Some("gsk_clave_invalida"), &request).await.unwrap_err();
        assert!(matches!(err, TranscriptionError::Unauthorized), "{err:?}");
        assert!(matches!(provider.transcribe(None, &request).await.unwrap_err(), TranscriptionError::MissingApiKey));
        let err = provider.transcribe(Some("x"), &TranscriptionRequest { model: "modelo-inexistente".into(), ..request.clone() }).await.unwrap_err();
        assert!(matches!(err, TranscriptionError::Unauthorized | TranscriptionError::Rejected(_)), "{err:?}");
        std::fs::remove_file(wav).ok();
    }
}
