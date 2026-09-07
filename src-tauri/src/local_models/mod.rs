//! Downloadable speech models. The catalog is data; inference and storage are separate.

mod runtime;
pub mod messages;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

use crate::transcription::{TranscriptionError, TranscriptionRequest, TranscriptionResult};

pub const DEFAULT_MODEL: &str = "parakeet-v3";
pub type ProgressCallback = Arc<dyn Fn(LocalModelInfo) + Send + Sync>;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub engine: String,
    pub languages: Vec<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub filename: String,
    pub download_url: String,
    pub source_url: String,
    pub artifact_source_url: String,
    pub license: String,
    pub requires_language: bool,
    pub recommended: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog { schema_version: u32, models: Vec<CatalogModel> }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub engine: String,
    pub languages: Vec<String>,
    pub size_bytes: u64,
    pub installed: bool,
    pub status: String,
    pub progress: f64,
    pub downloaded_bytes: u64,
    pub error: Option<String>,
    pub requires_language: bool,
    pub license: String,
    pub source_url: String,
    pub artifact_source_url: String,
    pub recommended: bool,
    pub available: bool,
}

#[derive(Clone)]
struct DownloadState { status: &'static str, bytes: u64, error: Option<String> }

struct ActiveDownload { id: String, cancelled: Arc<AtomicBool> }

pub struct LocalModelManager {
    root: PathBuf,
    _store_lock: std::fs::File,
    catalog: Vec<CatalogModel>,
    http: reqwest::Client,
    active: Mutex<Option<ActiveDownload>>,
    states: Mutex<HashMap<String, DownloadState>>,
    runtime: Arc<runtime::Runtime>,
}

impl LocalModelManager {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let catalog: Catalog = serde_json::from_str(include_str!("catalog.json")).map_err(|e| e.to_string())?;
        if catalog.schema_version != 1 { return Err("Unsupported local model catalog version".into()); }
        validate_catalog(&catalog.models)?;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let store_lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .open(root.join(".model-store.lock")).map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&store_lock).map_err(|_| "Another running app is using this local model folder".to_string())?;
        // An interrupted download is never a usable model. Only our own temporary suffix is cleaned.
        for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
            if entry.file_name().to_string_lossy().ends_with(".download-part") && entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("Dictamelo/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build().map_err(|e| e.to_string())?;
        Ok(Self { root, _store_lock: store_lock, catalog: catalog.models, http, active: Mutex::new(None), states: Mutex::new(HashMap::new()), runtime: Arc::new(runtime::Runtime::default()) })
    }

    pub fn catalog(&self) -> &[CatalogModel] { &self.catalog }

    fn model(&self, id: &str) -> Result<&CatalogModel, String> {
        self.catalog.iter().find(|m| m.id == id).ok_or_else(|| "Unknown local model".into())
    }

    fn installed(&self, model: &CatalogModel) -> bool {
        if self.root.join(format!("{}.invalid", model.id)).exists() { return false; }
        std::fs::symlink_metadata(self.root.join(&model.filename))
            .map(|m| m.file_type().is_file() && m.len() == model.size_bytes).unwrap_or(false)
    }

    fn info(&self, model: &CatalogModel) -> LocalModelInfo {
        let installed = self.installed(model);
        let state = self.states.lock().unwrap_or_else(|e| e.into_inner()).get(&model.id).cloned();
        let (status, bytes, error) = match state {
            Some(s) if !installed => (s.status, s.bytes, s.error),
            _ if installed => ("ready", model.size_bytes, None),
            _ => ("not_downloaded", 0, None),
        };
        LocalModelInfo { id: model.id.clone(), name: model.name.clone(), description: model.description.clone(),
            engine: model.engine.clone(), languages: model.languages.clone(), size_bytes: model.size_bytes,
            installed, status: status.into(), progress: (bytes as f64 / model.size_bytes as f64).min(1.0),
            downloaded_bytes: bytes, error, requires_language: model.requires_language, license: model.license.clone(),
            source_url: model.source_url.clone(), artifact_source_url: model.artifact_source_url.clone(),
            recommended: model.recommended, available: cfg!(target_os = "macos") }
    }

    pub fn list(&self) -> Vec<LocalModelInfo> { self.catalog.iter().map(|model| self.info(model)).collect() }

    fn progress(&self, model: &CatalogModel, status: &'static str, bytes: u64, error: Option<String>, callback: &ProgressCallback) {
        self.states.lock().unwrap_or_else(|e| e.into_inner()).insert(model.id.clone(), DownloadState {status, bytes, error});
        callback(self.info(model));
    }

    pub async fn download(&self, id: &str, callback: ProgressCallback) -> Result<(), String> {
        if self.runtime.is_stopping() { return Err("Local transcription is shutting down".into()); }
        if !cfg!(target_os = "macos") { return Err("Local models are currently available on macOS".into()); }
        let model = self.model(id)?;
        if self.installed(model) { callback(self.info(model)); return Ok(()); }
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            if self.runtime.is_stopping() { return Err("Local transcription is shutting down".into()); }
            if active.is_some() { return Err("Another model download is already in progress".into()); }
            *active = Some(ActiveDownload { id: id.into(), cancelled: cancelled.clone() });
        }
        let partial = self.root.join(format!("{}.{}.download-part", model.id, uuid::Uuid::new_v4()));
        let guard = DownloadGuard { manager: self, partial: partial.clone(), id: id.into() };
        self.progress(model, "downloading", 0, None, &callback);
        let result = self.download_inner(model, &partial, &cancelled, &callback).await;
        // Clear the active slot before the terminal event lets the UI start another download.
        drop(guard);
        match result {
            Ok(()) => {
                self.progress(model, "ready", model.size_bytes, None, &callback); Ok(())
            }
            Err(_) if cancelled.load(Ordering::Relaxed) => {
                self.progress(model, "not_downloaded", 0, None, &callback);
                Ok(())
            }
            Err(error) => { self.progress(model, "error", 0, Some(error.clone()), &callback); Err(error) }
        }
    }

    async fn download_inner(&self, model: &CatalogModel, partial: &Path, cancelled: &AtomicBool, callback: &ProgressCallback) -> Result<(), String> {
        let free = fs2::available_space(&self.root).map_err(|e| e.to_string())?;
        if free < model.size_bytes.saturating_add(128 * 1024 * 1024) { return Err("Not enough free disk space for this model".into()); }
        let mut response = cancellable(self.http.get(&model.download_url).send(), cancelled).await?
            .map_err(|e| format!("Model download could not connect: {e}"))?
            .error_for_status().map_err(|e| format!("Model download was refused: {e}"))?;
        if response.content_length().is_some_and(|len| len != model.size_bytes) { return Err("Model download size does not match the catalog".into()); }
        let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(partial).await.map_err(|e| e.to_string())?;
        let mut verifier = DownloadVerifier::new(model.size_bytes, &model.sha256);
        let mut last_update = Instant::now();
        loop {
            let chunk = cancellable(response.chunk(), cancelled).await?.map_err(|e| format!("Model download interrupted: {e}"))?;
            let Some(chunk) = chunk else { break; };
            verifier.push(&chunk)?;
            file.write_all(&chunk).await.map_err(|e| format!("Could not save model: {e}"))?;
            if last_update.elapsed() >= Duration::from_millis(150) {
                self.progress(model, "downloading", verifier.bytes, None, callback);
                last_update = Instant::now();
            }
        }
        self.progress(model, "verifying", verifier.bytes, None, callback);
        verifier.finish()?;
        if cancelled.load(Ordering::Relaxed) { return Err("Download cancelled".into()); }
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        if cancelled.load(Ordering::Relaxed) { return Err("Download cancelled".into()); }
        // The temporary file lives beside the destination, so publishing it is atomic.
        tokio::fs::rename(partial, self.root.join(&model.filename)).await.map_err(|e| e.to_string())?;
        let marker = self.root.join(format!("{}.invalid", model.id));
        if marker.exists() { tokio::fs::remove_file(marker).await.map_err(|e| e.to_string())?; }
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.model(id)?;
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = active.as_ref().filter(|a| a.id == id) { active.cancelled.store(true, Ordering::Relaxed); }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let model = self.model(id)?;
        // Keep this guard until deletion completes: a concurrent download cannot publish this path.
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|a| a.id == id) { return Err("Cancel the download before deleting this model".into()); }
        self.runtime.remove(id, || {
            let path = self.root.join(&model.filename);
            match std::fs::remove_file(path) { Ok(()) => Ok(()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), Err(e) => Err(e.to_string()) }
        })?;
        let _ = std::fs::remove_file(self.root.join(format!("{}.invalid", id)));
        self.states.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
        Ok(())
    }

    pub fn cancel_transcription_for(&self, audio_path: &Path) { self.runtime.cancel_for(audio_path); }

    /// Release native model/GPU resources before Tauri invokes process exit or restart.
    pub fn shutdown(&self) {
        self.runtime.request_shutdown();
        {
            let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(download) = active.as_ref() { download.cancelled.store(true, Ordering::Relaxed); }
        }
        self.runtime.finish_shutdown();
    }

    pub fn validate_ready(&self, id: &str, language: Option<&str>) -> Result<(), String> {
        if self.runtime.is_stopping() { return Err("Local transcription is shutting down".into()); }
        if !cfg!(target_os = "macos") { return Err("Local models are currently available on macOS".into()); }
        let model = self.model(id)?;
        if !self.installed(model) { return Err("Download the selected local model first".into()); }
        validate_language(model, language)
    }

    pub async fn transcribe(&self, request: &TranscriptionRequest) -> Result<TranscriptionResult, TranscriptionError> {
        let model = self.model(&request.model).map_err(TranscriptionError::Local)?.clone();
        if !self.installed(&model) { return Err(TranscriptionError::Local("Download the selected local model first".into())); }
        validate_language(&model, request.language.as_deref()).map_err(TranscriptionError::Local)?;
        let job = self.runtime.begin(&request.audio_path).map_err(TranscriptionError::Local)?;
        let request = request.clone();
        let path = self.root.join(&model.filename);
        let runtime = self.runtime.clone();
        let model_id = model.id.clone();
        match tokio::task::spawn_blocking(move || runtime.transcribe(&model, &path, &request, job)).await
            .map_err(|_| TranscriptionError::Local("Local transcription stopped unexpectedly".into()))? {
            Ok(result) => Ok(result),
            Err(runtime::InferenceError::Cancelled) => Err(TranscriptionError::Cancelled),
            Err(runtime::InferenceError::InvalidModel(error)) => {
                // Persist the failed-integrity state so the UI offers a repair download, even after restart.
                std::fs::write(self.root.join(format!("{}.invalid", model_id)), b"Model integrity verification failed\n")
                    .map_err(TranscriptionError::Io)?;
                self.states.lock().unwrap_or_else(|e| e.into_inner()).insert(model_id, DownloadState {status: "error", bytes: 0, error: Some(error.clone())});
                Err(TranscriptionError::Local(error))
            }
            Err(runtime::InferenceError::Failed(error)) => Err(TranscriptionError::Local(error)),
        }
    }
}

struct DownloadGuard<'a> { manager: &'a LocalModelManager, partial: PathBuf, id: String }
impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.partial);
        self.manager.active.lock().unwrap_or_else(|e| e.into_inner()).take();
        // A dropped async task must not leave a permanently downloading card.
        if let Some(state) = self.manager.states.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&self.id) {
            if matches!(state.status, "downloading" | "verifying") { state.status = "not_downloaded"; state.bytes = 0; }
        }
    }
}

async fn cancellable<F: std::future::Future>(future: F, cancelled: &AtomicBool) -> Result<F::Output, String> {
    tokio::pin!(future);
    loop {
        if cancelled.load(Ordering::Relaxed) { return Err("Download cancelled".into()); }
        tokio::select! { result = &mut future => return Ok(result), _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
    }
}

struct DownloadVerifier { expected_bytes: u64, expected_hash: String, bytes: u64, hasher: Sha256 }
impl DownloadVerifier {
    fn new(expected_bytes: u64, expected_hash: &str) -> Self { Self { expected_bytes, expected_hash: expected_hash.into(), bytes: 0, hasher: Sha256::new() } }
    fn push(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.bytes = self.bytes.checked_add(bytes.len() as u64).ok_or("Model download is too large")?;
        if self.bytes > self.expected_bytes { return Err("Model download exceeds the expected size".into()); }
        self.hasher.update(bytes); Ok(())
    }
    fn finish(self) -> Result<(), String> {
        if self.bytes != self.expected_bytes { return Err("Model download is incomplete".into()); }
        if format!("{:x}", self.hasher.finalize()) != self.expected_hash { return Err("Model checksum verification failed. Please download it again".into()); }
        Ok(())
    }
}

pub(super) fn verify_file(path: &Path, model: &CatalogModel, cancelled: impl Fn() -> bool) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file() || metadata.len() != model.size_bytes { return Err("Local model is missing or damaged. Download it again".into()); }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut verifier = DownloadVerifier::new(model.size_bytes, &model.sha256);
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        if cancelled() { return Err(runtime::CANCELLED.into()); }
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?; if n == 0 { break; } verifier.push(&buffer[..n])?;
    }
    verifier.finish()
}

fn validate_language(model: &CatalogModel, language: Option<&str>) -> Result<(), String> {
    let language = language.filter(|l| !l.is_empty() && *l != "auto");
    if model.requires_language && language.is_none() { return Err(format!("{} needs an explicit audio language: {}", model.name, model.languages.join(", "))); }
    if language.is_some_and(|lang| !model.languages.iter().any(|l| l == lang)) { return Err(format!("The selected language is not supported by {}", model.name)); }
    Ok(())
}

fn validate_catalog(models: &[CatalogModel]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut filenames = std::collections::HashSet::new();
    for model in models {
        let mut parts = Path::new(&model.filename).components();
        let url = reqwest::Url::parse(&model.download_url).map_err(|e| e.to_string())?;
        if model.id.is_empty() || !model.id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some()
            || model.size_bytes == 0 || model.sha256.len() != 64 || !model.sha256.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || url.scheme() != "https" || url.host_str() != Some("huggingface.co") || !url.username().is_empty() || url.password().is_some()
            || !matches!(model.engine.as_str(), "whisper" | "canary" | "parakeet") || !ids.insert(&model.id) || !filenames.insert(&model.filename)
        { return Err(format!("Invalid local model catalog entry: {}", model.id)); }
        let segments: Vec<_> = url.path_segments().ok_or("Invalid model URL")?.collect();
        if segments.len() != 5 || segments[2] != "resolve" || segments[3].len() != 40 || !segments[3].bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Model download URLs must pin a complete immutable revision".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Vec<CatalogModel> { serde_json::from_str::<Catalog>(include_str!("catalog.json")).unwrap().models }
    #[test]
    fn catalog_is_pinned_and_canary_requires_supported_language() {
        let models = catalog(); validate_catalog(&models).unwrap(); assert_eq!(models.len(), 6);
        let canary = models.iter().find(|m| m.engine == "canary").unwrap();
        assert!(validate_language(canary, None).is_err()); assert!(validate_language(canary, Some("it")).is_err()); assert!(validate_language(canary, Some("es")).is_ok());
        let mut unsafe_models = models.clone(); unsafe_models[0].filename = "../escape.gguf".into(); assert!(validate_catalog(&unsafe_models).is_err());
        unsafe_models = models; unsafe_models[0].download_url = "https://huggingface.co/owner/model/resolve/main/model.gguf".into(); assert!(validate_catalog(&unsafe_models).is_err());
    }
    #[test]
    fn download_verification_rejects_truncation_corruption_and_oversize() {
        let hash = format!("{:x}", Sha256::digest(b"model"));
        let mut correct = DownloadVerifier::new(5, &hash); correct.push(b"mo").unwrap(); correct.push(b"del").unwrap(); correct.finish().unwrap();
        let mut short = DownloadVerifier::new(5, &hash); short.push(b"mod").unwrap(); assert!(short.finish().is_err());
        let mut corrupt = DownloadVerifier::new(5, &hash); corrupt.push(b"xxxxx").unwrap(); assert!(corrupt.finish().is_err());
        let mut oversize = DownloadVerifier::new(5, &hash); assert!(oversize.push(b"models").is_err());
    }
    #[tokio::test]
    async fn cancellation_interrupts_a_stalled_download() {
        let flag = Arc::new(AtomicBool::new(false)); let other = flag.clone();
        tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(20)).await; other.store(true, Ordering::Relaxed); });
        assert!(tokio::time::timeout(Duration::from_secs(1), cancellable(std::future::pending::<()>(), &flag)).await.unwrap().is_err());
    }
    #[test]
    fn interrupted_downloads_and_unknown_delete_are_safe() {
        let root = std::env::temp_dir().join(format!("dictamelo-model-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap(); std::fs::write(root.join("stale.download-part"), b"partial").unwrap(); std::fs::write(root.join("keep.txt"), b"user data").unwrap();
        let manager = LocalModelManager::new(root.clone()).unwrap();
        assert!(!root.join("stale.download-part").exists()); assert!(root.join("keep.txt").exists()); assert!(manager.delete("../keep.txt").is_err());
        assert!(manager.list().iter().all(|m| !m.installed));
        assert!(LocalModelManager::new(root.clone()).is_err());
        drop(manager); assert!(LocalModelManager::new(root.clone()).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn failed_download_never_publishes_a_model_and_can_be_retried() {
        use tokio::io::AsyncReadExt;
        let root = std::env::temp_dir().join(format!("dictamelo-download-test-{}", uuid::Uuid::new_v4()));
        let mut manager = LocalModelManager::new(root.clone()).unwrap();
        manager.http = reqwest::Client::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut model = catalog().remove(0);
        model.size_bytes = 5; model.sha256 = format!("{:x}", Sha256::digest(b"model"));
        model.download_url = format!("http://{address}/fixture"); manager.catalog = vec![model.clone()];
        let server = tokio::spawn(async move {
            for body in [b"wrong", b"model"] {
                let (mut stream, _) = listener.accept().await.unwrap(); let mut request = [0; 1024]; let _ = stream.read(&mut request).await;
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n").await.unwrap(); stream.write_all(body).await.unwrap();
            }
        });
        assert!(manager.download(&model.id, Arc::new(|_| {})).await.is_err());
        assert!(!root.join(&model.filename).exists());
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| entry.unwrap().file_name() == ".model-store.lock"));
        manager.download(&model.id, Arc::new(|_| {})).await.unwrap();
        assert_eq!(std::fs::read(root.join(&model.filename)).unwrap(), b"model");
        assert!(manager.list()[0].installed); server.await.unwrap(); drop(manager); std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn same_size_corrupt_model_becomes_repairable_and_repair_is_verified() {
        use tokio::io::AsyncReadExt;
        let root = std::env::temp_dir().join(format!("dictamelo-model-repair-{}", uuid::Uuid::new_v4()));
        let mut manager = LocalModelManager::new(root.clone()).unwrap();
        manager.http = reqwest::Client::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut model = catalog().remove(0); model.size_bytes = 5; model.sha256 = format!("{:x}", Sha256::digest(b"model"));
        model.download_url = format!("http://{address}/fixture"); manager.catalog = vec![model.clone()];
        std::fs::write(root.join(&model.filename), b"wrong").unwrap();
        assert!(manager.list()[0].installed);
        let request = TranscriptionRequest { audio_path: Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tests/fixtures/english-speech.wav"), model: model.id.clone(), language: Some("en".into()), prompt: None };
        assert!(manager.transcribe(&request).await.is_err());
        assert!(!manager.list()[0].installed); assert_eq!(manager.list()[0].status, "error");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap(); let mut request = [0; 1024]; let _ = stream.read(&mut request).await;
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nmodel").await.unwrap();
        });
        manager.download(&model.id, Arc::new(|_| {})).await.unwrap();
        assert_eq!(std::fs::read(root.join(&model.filename)).unwrap(), b"model");
        assert!(manager.list()[0].installed); server.await.unwrap(); drop(manager); std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn shutdown_cancels_downloads_and_rejects_new_work() {
        let root = std::env::temp_dir().join(format!("dictamelo-model-shutdown-{}", uuid::Uuid::new_v4()));
        let manager = LocalModelManager::new(root.clone()).unwrap();
        let id = manager.catalog()[0].id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        *manager.active.lock().unwrap() = Some(ActiveDownload { id: id.clone(), cancelled: cancelled.clone() });
        manager.shutdown();
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(manager.download(&id, Arc::new(|_| panic!("download started after shutdown"))).await.is_err());
        assert!(manager.validate_ready(&id, Some("en")).is_err());
        manager.shutdown(); drop(manager); std::fs::remove_dir_all(root).unwrap();
    }

    /// Downloads pinned public model weights, then runs the licensed fixture entirely offline.
    /// The opt-in leaves weights in dist so native UI checks can reuse the exact verified files.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "downloads local speech models; requires DICTAMELO_LOCAL_TESTS=1"]
    async fn local_models_transcribe_the_licensed_english_fixture() {
        assert_eq!(std::env::var("DICTAMELO_LOCAL_TESTS").as_deref(), Ok("1"));
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root = std::env::var_os("DICTAMELO_LOCAL_TEST_DIR").map(PathBuf::from).unwrap_or_else(|| repo.join("dist/local-model-smoke/models"));
        let manager = Arc::new(LocalModelManager::new(root.clone()).unwrap());
        let ids = std::env::var("DICTAMELO_LOCAL_TEST_MODELS").unwrap_or_else(|_| manager.catalog().iter().map(|model| model.id.as_str()).collect::<Vec<_>>().join(","));
        let audio = std::env::var_os("DICTAMELO_LOCAL_TEST_AUDIO").map(PathBuf::from).unwrap_or_else(|| repo.join("tests/fixtures/english-speech.wav"));
        let language = std::env::var("DICTAMELO_LOCAL_TEST_LANGUAGE").unwrap_or_else(|_| "en".into());
        let expected = std::env::var("DICTAMELO_LOCAL_TEST_EXPECTED").unwrap_or_else(|_| "middle,classes,gospel".into());
        let mut report = Vec::new();
        for id in ids.split(',') {
            let started = Instant::now();
            manager.download(id, Arc::new(|_| {})).await.unwrap_or_else(|error| panic!("{id} download: {error}"));
            let download_secs = started.elapsed().as_secs_f64();
            let request = TranscriptionRequest { audio_path: audio.clone(), model: id.into(), language: Some(language.clone()), prompt: None };
            let started = Instant::now();
            let result = manager.transcribe(&request).await.unwrap_or_else(|error| panic!("{id} inference: {error}"));
            let elapsed = started.elapsed().as_secs_f64();
            let normalized = result.text.to_lowercase();
            assert!(expected.split(',').all(|word| normalized.contains(word)), "{id}: unexpected fixture transcription: {}", result.text);
            let warmed = Instant::now();
            let warm_result = manager.transcribe(&request).await.unwrap_or_else(|error| panic!("{id} warm inference: {error}"));
            let warm_secs = warmed.elapsed().as_secs_f64();
            assert!(expected.split(',').all(|word| warm_result.text.to_lowercase().contains(word)), "{id}: unexpected warm transcription: {}", warm_result.text);
            eprintln!("{id}: download {download_secs:.2}s, first inference {elapsed:.2}s, warm {warm_secs:.2}s, {}", result.text);
            report.push(serde_json::json!({ "id": id, "downloadSeconds": download_secs, "firstInferenceSeconds": elapsed, "warmInferenceSeconds": warm_secs, "text": result.text, "audioSeconds": result.duration_secs, "passed": true }));
            let report_name = std::env::var("DICTAMELO_LOCAL_TEST_REPORT").unwrap_or_else(|_| "results.json".into());
            std::fs::write(root.parent().unwrap().join(report_name), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "runs downloaded local speech models; requires DICTAMELO_LOCAL_TESTS=1"]
    async fn local_models_handle_silence_and_bounded_window_audio() {
        assert_eq!(std::env::var("DICTAMELO_LOCAL_TESTS").as_deref(), Ok("1"));
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root = std::env::var_os("DICTAMELO_LOCAL_TEST_DIR").map(PathBuf::from).unwrap_or_else(|| repo.join("dist/local-model-release/models"));
        let manager = LocalModelManager::new(root.clone()).unwrap();
        let parent = root.parent().unwrap();
        let silence = parent.join("digital-silence.wav");
        let repeated = parent.join("repeated-english-speech.wav");
        let mut original = hound::WavReader::open(repo.join("tests/fixtures/english-speech.wav")).unwrap();
        let samples: Vec<i16> = original.samples::<i16>().map(Result::unwrap).collect();
        let spec = original.spec();
        let mut writer = hound::WavWriter::create(&silence, spec).unwrap();
        for _ in 0..5 * 16_000 { writer.write_sample(0_i16).unwrap(); } writer.finalize().unwrap();
        let mut writer = hound::WavWriter::create(&repeated, spec).unwrap();
        for _ in 0..6 {
            for sample in &samples { writer.write_sample(*sample).unwrap(); }
            for _ in 0..16_000 { writer.write_sample(0_i16).unwrap(); }
        }
        writer.finalize().unwrap();
        let mut report = Vec::new();
        for model in manager.catalog() {
            let request = TranscriptionRequest { audio_path: silence.clone(), model: model.id.clone(), language: Some("en".into()), prompt: None };
            let result = manager.transcribe(&request).await.unwrap(); assert!(result.text.is_empty(), "{} hallucinated on digital silence", model.id);
            if matches!(model.engine.as_str(), "canary" | "parakeet") {
                let started = Instant::now();
                let result = manager.transcribe(&TranscriptionRequest { audio_path: repeated.clone(), ..request }).await.unwrap();
                let text = result.text.to_lowercase();
                assert_eq!(text.matches("gospel").count(), 6, "{} lost a repetition: {}", model.id, result.text);
                assert_eq!(text.matches("middle classes").count(), 6, "{} lost words: {}", model.id, result.text);
                eprintln!("{} preserved all six speech repetitions across local model windows", model.id);
                report.push(serde_json::json!({"id":model.id,"audioSeconds":result.duration_secs,"inferenceSeconds":started.elapsed().as_secs_f64(),"repetitions":6,"text":result.text,"passed":true}));
            }
        }
        std::fs::write(parent.join("silence-and-windowing.json"),serde_json::to_vec_pretty(&serde_json::json!({"silenceModelsPassed":manager.catalog().len(),"longAudio":report})).unwrap()).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "runs a downloaded Whisper model in a child process; requires DICTAMELO_LOCAL_TESTS=1"]
    async fn local_models_shutdown_before_process_exit() {
        assert_eq!(std::env::var("DICTAMELO_LOCAL_TESTS").as_deref(), Ok("1"));
        if std::env::var("DICTAMELO_LOCAL_SHUTDOWN_CHILD").as_deref() == Ok("1") {
            let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let root = std::env::var_os("DICTAMELO_LOCAL_TEST_DIR").map(PathBuf::from).unwrap_or_else(|| repo.join("dist/local-model-release/models"));
            let manager = LocalModelManager::new(root).unwrap();
            let request = TranscriptionRequest { audio_path: repo.join("tests/fixtures/english-speech.wav"), model: "whisper-base".into(), language: Some("en".into()), prompt: None };
            let result = manager.transcribe(&request).await.unwrap();
            assert!(result.text.to_lowercase().contains("gospel"));
            manager.shutdown(); manager.shutdown();
            assert!(manager.transcribe(&request).await.is_err());
            println!("LOCAL_SHUTDOWN_OK");
            // Tauri also exits without dropping managed Rust state. Native global destructors
            // must find no live Metal resources even while this manager remains in scope.
            std::process::exit(0);
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "local_models::tests::local_models_shutdown_before_process_exit", "--ignored", "--nocapture"])
            .env("DICTAMELO_LOCAL_SHUTDOWN_CHILD", "1")
            .env("DICTAMELO_LOCAL_TEST_BACKEND", "metal")
            .output().unwrap();
        assert!(output.status.success(), "Metal shutdown child failed: {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("LOCAL_SHUTDOWN_OK"));
    }
}
