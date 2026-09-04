//! Capa de transcripción desacoplada: la app solo conoce el trait `TranscriptionProvider`.
//!
//! Para añadir un proveedor (OpenAI, Gemini, Grok, Deepgram, un modelo local…):
//! 1. Crear un módulo que implemente `TranscriptionProvider`.
//! 2. Registrarlo en `ProviderRegistry::with_defaults`.
//! Nada más cambia: la UI lista los proveedores/modelos a partir de `ProviderInfo`.

pub mod groq;
pub mod openai;
pub mod openai_compatible;

use async_trait::async_trait;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    /// Archivo de audio (WAV mono 16 kHz) a transcribir.
    pub audio_path: PathBuf,
    pub model: String,
    /// Código ISO-639-1; `None` = detección automática.
    pub language: Option<String>,
    /// Texto de contexto opcional (vocabulario, estilo).
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // `duration_secs` lo informan solo algunos proveedores; se conserva para diagnóstico.
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("Falta la API key del proveedor")]
    MissingApiKey,
    #[error("API key inválida o sin autorización")]
    Unauthorized,
    #[error("Límite de uso del proveedor alcanzado; espera unos segundos")]
    RateLimited,
    #[error("Sin conexión con el servicio de transcripción ({0})")]
    Network(String),
    #[error("El servicio tardó demasiado en responder")]
    Timeout,
    #[error("Error del servidor ({status}): {message}")]
    Server { status: u16, message: String },
    #[error("El proveedor rechazó la petición: {0}")]
    Rejected(String),
    #[error("Respuesta inesperada del proveedor: {0}")]
    InvalidResponse(String),
    #[error("No se pudo leer el audio: {0}")]
    Io(#[from] std::io::Error),
}

impl TranscriptionError {
    /// Mensaje para el usuario en el idioma indicado.
    pub fn localized(&self, lang: &str) -> String {
        use crate::i18n::{t, tf};
        match self {
            TranscriptionError::MissingApiKey => t(lang, "tr.missing_key").into(),
            TranscriptionError::Unauthorized => t(lang, "tr.unauthorized").into(),
            TranscriptionError::RateLimited => t(lang, "tr.rate").into(),
            TranscriptionError::Network(_) => t(lang, "tr.network").into(),
            TranscriptionError::Timeout => t(lang, "tr.timeout").into(),
            TranscriptionError::Server { status, .. } => tf(lang, "tr.server", &[("s", &status.to_string())]),
            TranscriptionError::Rejected(e) => tf(lang, "tr.rejected", &[("e", e)]),
            TranscriptionError::InvalidResponse(_) => t(lang, "tr.invalid").into(),
            TranscriptionError::Io(e) => tf(lang, "tr.io", &[("e", &e.to_string())]),
        }
    }

    /// Errores transitorios que merecen un reintento automático.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TranscriptionError::Network(_)
                | TranscriptionError::Timeout
                | TranscriptionError::Server { .. }
                | TranscriptionError::RateLimited
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub requires_api_key: bool,
    /// Página donde el usuario obtiene su API key.
    pub key_url: String,
    pub models: Vec<ModelInfo>,
    pub default_model: String,
    /// `true` si el proveedor se probó de extremo a extremo en esta versión.
    pub verified: bool,
}

#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;

    /// Transcribe el audio. `api_key` es `None` cuando el usuario no la configuró.
    async fn transcribe(
        &self,
        api_key: Option<&str>,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError>;
}

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn TranscriptionProvider>>,
}

impl ProviderRegistry {
    pub fn with_defaults() -> Self {
        let http = shared_http_client();
        let mut registry = ProviderRegistry { providers: Vec::new() };
        registry.register(Arc::new(groq::GroqProvider::new(http.clone())));
        registry.register(Arc::new(openai::OpenAiProvider::new(http)));
        registry
    }

    pub fn register(&mut self, provider: Arc<dyn TranscriptionProvider>) {
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn TranscriptionProvider>> {
        self.providers.iter().find(|p| p.info().id == id).cloned()
    }

    pub fn list(&self) -> Vec<ProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }
}

/// Cliente HTTP compartido (rustls, sin OpenSSL) con tiempos de espera razonables.
pub fn shared_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Dictamelo/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()
        .expect("configuración válida del cliente HTTP")
}
