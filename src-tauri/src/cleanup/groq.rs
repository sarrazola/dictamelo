//! Limpieza con los modelos GPT-OSS de Groq (rápidos y baratos; comparten la API key de Groq).

use super::openai_compatible_chat::OpenAiCompatibleChatClient;
use super::{wrap_transcript, CleanerInfo, TextCleaner};
use crate::transcription::groq::GROQ_BASE_URL;
use crate::transcription::{ModelInfo, TranscriptionError};
use async_trait::async_trait;

pub struct GroqCleaner {
    client: OpenAiCompatibleChatClient,
}

impl GroqCleaner {
    pub const ID: &'static str = "groq";

    pub fn new(http: reqwest::Client) -> Self {
        Self { client: OpenAiCompatibleChatClient::new(http, GROQ_BASE_URL) }
    }
}

#[async_trait]
impl TextCleaner for GroqCleaner {
    fn info(&self) -> CleanerInfo {
        CleanerInfo {
            id: Self::ID.into(),
            name: "Groq".into(),
            key_provider: "groq".into(),
            default_model: "openai/gpt-oss-120b".into(),
            models: vec![
                ModelInfo {
                    id: "openai/gpt-oss-120b".into(),
                    name: "GPT-OSS 120B".into(),
                    description: "model.desc.oss120".into(),
                },
                ModelInfo {
                    id: "openai/gpt-oss-20b".into(),
                    name: "GPT-OSS 20B".into(),
                    description: "model.desc.oss20".into(),
                },
            ],
        }
    }

    async fn clean(
        &self,
        api_key: Option<&str>,
        model: &str,
        system_prompt: &str,
        text: &str,
        _cleanup_receipt: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let key = api_key.map(str::trim).filter(|k| !k.is_empty()).ok_or(TranscriptionError::MissingApiKey)?;
        // Razonamiento al mínimo: el trabajo es mecánico y la latencia importa más.
        self.client.complete(key, model, system_prompt, &wrap_transcript(text), Some("low")).await
    }
}

/// Live Groq cleanup test; the API key must be present in Keychain.
/// Run: `DICTAMELO_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml cleans_spanish_dictation -- --ignored`.
#[cfg(all(test, target_os = "macos"))]
mod live_tests {
    use super::*;
    use crate::cleanup::build_system_prompt;
    use crate::transcription::shared_http_client;
    use std::process::Command;

    fn keychain_key() -> Option<String> {
        let out = Command::new("security")
            .args(["find-generic-password", "-s", crate::state::KEYCHAIN_SERVICE, "-a", "groq", "-w"])
            .output()
            .ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    #[tokio::test]
    #[ignore = "requires explicit live/OS opt-in"]
    async fn cleans_spanish_dictation() {
        assert_eq!(std::env::var("DICTAMELO_LIVE_TESTS").as_deref(), Ok("1"), "explicit opt-in requires DICTAMELO_LIVE_TESTS=1");
        let key = keychain_key().expect("API key de Groq en el Llavero");
        let cleaner = GroqCleaner::new(shared_http_client());
        let prompt = build_system_prompt("", "Sarrazola");
        let raw = "eh bueno entonces o sea mándale el correo a sarrasola el jueves no espera el viernes punto y dile que que la reunión es a las tres";
        let out = cleaner.clean(Some(&key), "openai/gpt-oss-120b", &prompt, raw, None).await.expect("limpieza");
        eprintln!("limpio: {out:?}");
        let lower = out.to_lowercase();
        assert!(!lower.contains("o sea") && !lower.starts_with("eh"), "quedaron muletillas: {out}");
        assert!(lower.contains("viernes") && !lower.contains("jueves"), "no aplicó la autocorrección: {out}");
        assert!(out.contains("Sarrazola"), "no respetó el vocabulario: {out}");
        assert!(!out.contains("<transcript>") && !out.starts_with('"'));

        // Una pregunta se limpia, no se responde.
        let out = cleaner.clean(Some(&key), "openai/gpt-oss-20b", &prompt, "cuál es la capital de francia", None).await.unwrap();
        eprintln!("pregunta: {out:?}");
        assert!(!out.to_lowercase().contains("parís"), "respondió en vez de limpiar: {out}");
    }
}
