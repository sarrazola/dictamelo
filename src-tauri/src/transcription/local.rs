//! Offline transcription through the downloaded model catalog.

use std::sync::Arc;
use async_trait::async_trait;
use crate::local_models::LocalModelManager;
use super::{ModelInfo, ProviderInfo, TranscriptionError, TranscriptionProvider, TranscriptionRequest, TranscriptionResult};

pub struct LocalProvider { manager: Arc<LocalModelManager> }

impl LocalProvider {
    pub const ID: &'static str = "local";
    pub fn new(manager: Arc<LocalModelManager>) -> Self { Self { manager } }
}

#[async_trait]
impl TranscriptionProvider for LocalProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: Self::ID.into(), name: "Local models".into(), requires_api_key: false,
            key_url: String::new(), default_model: crate::local_models::DEFAULT_MODEL.into(), verified: false,
            models: self.manager.catalog().iter().map(|m| ModelInfo { id: m.id.clone(), name: m.name.clone(), description: m.description.clone() }).collect(),
        }
    }
    async fn transcribe(&self, _api_key: Option<&str>, request: &TranscriptionRequest) -> Result<TranscriptionResult, TranscriptionError> {
        self.manager.transcribe(request).await
    }
}
