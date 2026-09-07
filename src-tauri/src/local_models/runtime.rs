//! Native inference is lazy, serialized, and limited to one resident model.

use std::path::Path;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::atomic::AtomicU64;
use crate::transcription::{TranscriptionRequest, TranscriptionResult};
use super::CatalogModel;

pub const CANCELLED: &str = "Local transcription cancelled";

#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("Local transcription cancelled")]
    Cancelled,
    #[error("{0}")]
    InvalidModel(String),
    #[error("{0}")]
    Failed(String),
}
impl From<String> for InferenceError {
    fn from(value: String) -> Self { if value == CANCELLED { Self::Cancelled } else { Self::Failed(value) } }
}
impl From<&str> for InferenceError { fn from(value: &str) -> Self { value.to_owned().into() } }

#[cfg(any(target_os = "macos", target_os = "windows"))]
use transcribe_cpp::{CancelToken, Feature, Model, RunExtension, RunOptions, SessionOptions, TimestampKind, WhisperRunOptions};

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct Loaded { id: String, model: Model }

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct ActiveJob { token: CancelToken, audio_path: std::path::PathBuf }

#[derive(Default)]
pub struct Runtime {
    stopping: AtomicBool,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    loaded: Mutex<Option<Loaded>>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    active: Mutex<Option<ActiveJob>>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    generation: AtomicU64,
}

pub struct Job {
    runtime: Arc<Runtime>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    token: CancelToken,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    generation: u64,
}

impl Drop for Job {
    fn drop(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.runtime.active.lock().unwrap_or_else(|e| e.into_inner()).take();
            // A timer never keeps the manager alive or unloads a newer/active model.
            if let Some(handle) = tokio::runtime::Handle::try_current().ok().filter(|_| !self.runtime.is_stopping()) {
                let weak = Arc::downgrade(&self.runtime);
                let generation = self.generation;
                handle.spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                    if let Some(runtime) = weak.upgrade() { runtime.release_idle(generation); }
                });
            }
        }
    }
}

impl Runtime {
    pub fn begin(self: &Arc<Self>, audio_path: &Path) -> Result<Job, String> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            static LOGGING: std::sync::Once = std::sync::Once::new();
            LOGGING.call_once(transcribe_cpp::disable_logging);
            let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            if self.is_stopping() { return Err("Local transcription is shutting down".into()); }
            if active.is_some() { return Err("A local transcription is already in progress".into()); }
            let token = CancelToken::new();
            *active = Some(ActiveJob { token: token.clone(), audio_path: audio_path.to_owned() });
            let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(Job { runtime: self.clone(), token, generation })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Err("Local models are currently available on macOS and Windows".into())
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn release_idle(&self, generation: u64) -> bool {
        let Ok(active) = self.active.try_lock() else { return false; };
        if self.is_stopping() || active.is_some() || self.generation.load(Ordering::Relaxed) != generation { return false; }
        let Ok(mut loaded) = self.loaded.try_lock() else { return false; };
        loaded.take(); true
    }

    pub fn is_stopping(&self) -> bool { self.stopping.load(Ordering::Acquire) }

    pub fn request_shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if let Some(job) = self.active.lock().unwrap_or_else(|e| e.into_inner()).as_ref() { job.token.cancel(); }
    }

    /// Called before Tauri's process exit, which does not drop all managed Rust state.
    /// Never hold `active` while waiting: the inference job needs it during teardown.
    pub fn finish_shutdown(&self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.loaded.lock().unwrap_or_else(|e| e.into_inner()).take();
    }

    pub fn cancel_for(&self, audio_path: &Path) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if let Some(job) = self.active.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|job| job.audio_path == audio_path) { job.token.cancel(); }
    }

    pub fn remove(&self, id: &str, remove_file: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            if active.is_some() { return Err("Wait until local transcription finishes before deleting a model".into()); }
            let mut loaded = self.loaded.try_lock().map_err(|_| "The local model is busy")?;
            if loaded.as_ref().is_some_and(|m| m.id == id) { loaded.take(); }
            remove_file()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        { let _ = id; remove_file() }
    }

    pub fn transcribe(&self, catalog: &CatalogModel, path: &Path, request: &TranscriptionRequest, job: Job) -> Result<TranscriptionResult, InferenceError> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if self.is_stopping() || job.token.is_cancelled() { return Err(CANCELLED.into()); }
            let samples = read_audio(&request.audio_path)?;
            if is_digital_silence(&samples) {
                return Ok(TranscriptionResult { text: String::new(), language: request.language.clone(), duration_secs: Some(samples.len() as f64 / 16_000.0), cleanup_receipt: None });
            }
            let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
            // A queued job may have passed its first check before shutdown drained the cache.
            // The same mutex protects shutdown and every path that can create native resources.
            if self.is_stopping() || job.token.is_cancelled() { return Err(CANCELLED.into()); }
            if loaded.as_ref().map_or(true, |m| m.id != catalog.id) {
                // Free the previous model before allocating its replacement.
                loaded.take();
                #[cfg(test)]
                let hash_started = std::time::Instant::now();
                super::verify_file(path, catalog, || job.token.is_cancelled()).map_err(|error| {
                    if error == CANCELLED { InferenceError::Cancelled } else { InferenceError::InvalidModel(error) }
                })?;
                #[cfg(test)]
                let hash_seconds = hash_started.elapsed().as_secs_f64();
                if job.token.is_cancelled() { return Err(CANCELLED.into()); }
                let options = transcribe_cpp::ModelOptions { backend: backend_for_family(&catalog.engine), ..Default::default() };
                #[cfg(test)]
                let options = transcribe_cpp::ModelOptions { backend: match std::env::var("DICTAMELO_LOCAL_TEST_BACKEND").as_deref() {
                    Ok("cpu") => transcribe_cpp::Backend::Cpu,
                    Ok("cpu_accel") => transcribe_cpp::Backend::CpuAccel,
                    Ok("metal") => transcribe_cpp::Backend::Metal,
                    _ => options.backend,
                }, ..options };
                #[cfg(test)]
                let load_started = std::time::Instant::now();
                let model = Model::load_with(path, &options).map_err(|e| format!("Could not load the local model: {e}"))?;
                #[cfg(test)]
                eprintln!("LOCAL_PHASE {} backend={} hash={hash_seconds:.3}s load={:.3}s", catalog.id, model.backend(), load_started.elapsed().as_secs_f64());
                log::info!("Loaded local transcription model {} using {}", catalog.id, model.backend());
                *loaded = Some(Loaded {id: catalog.id.clone(), model});
            }
            if job.token.is_cancelled() { return Err(CANCELLED.into()); }
            let model = &loaded.as_ref().ok_or("Local model was not loaded")?.model;
            let caps = model.capabilities();
            let mut session = model.session_with(&SessionOptions {
                n_threads: std::thread::available_parallelism().map(|v| v.get().saturating_sub(2).clamp(1, 8) as i32).unwrap_or(4),
                ..Default::default()
            }).map_err(|e| format!("Could not start local transcription: {e}"))?;
            session.set_cancel_token(&job.token);
            let options = RunOptions {
                timestamps: TimestampKind::None,
                // Parakeet performs multilingual recognition without a language-selection input.
                language: if catalog.engine == "parakeet" { None } else { request.language.clone().filter(|l| !l.is_empty() && l != "auto") },
                family: if model.supports(Feature::InitialPrompt) { Some(RunExtension::Whisper(WhisperRunOptions {
                    initial_prompt: request.prompt.clone(), ..Default::default()
                })) } else { None },
                ..Default::default()
            };
            let maximum_samples = if model.supports(Feature::LongForm) { samples.len().max(1) } else {
                let limit_ms = if caps.max_audio_ms > 0 { caps.max_audio_ms.min(25_000) } else { 25_000 };
                limit_ms as usize * 16
            };
            let mut text = String::new();
            let mut language = None;
            for range in chunk_ranges(&samples, maximum_samples) {
                if job.token.is_cancelled() { return Err(CANCELLED.into()); }
                let result = session.run(&samples[range], &options).map_err(|error| {
                    if job.token.is_cancelled() { CANCELLED.into() } else { format!("Local transcription failed: {error}") }
                })?;
                #[cfg(test)]
                if std::env::var("DICTAMELO_LOCAL_TEST_RAW").as_deref() == Ok("1") { eprintln!("LOCAL_RAW {} {:?}", catalog.id, result.raw_text); }
                if job.token.is_cancelled() { return Err(CANCELLED.into()); }
                if !text.is_empty() && !result.text.trim().is_empty() { text.push(' '); }
                text.push_str(result.text.trim());
                if language.is_none() { language = result.language; }
            }
            Ok(TranscriptionResult { text, language: language.or_else(|| request.language.clone()), duration_secs: Some(samples.len() as f64 / 16_000.0), cleanup_receipt: None })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        { let _ = (catalog, path, request, job); Err("Local models are currently available on macOS and Windows".into()) }
    }
}

fn is_digital_silence(samples: &[f32]) -> bool { samples.iter().all(|sample| *sample == 0.0) }

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn backend_for_family(engine: &str) -> transcribe_cpp::Backend {
    // Windows distributes a static CPU engine without GPU drivers or backend DLLs.
    #[cfg(target_os = "windows")]
    { let _ = engine; transcribe_cpp::Backend::Cpu }
    // Short dictation with these encoders was faster on CPU in the macOS fixture checks.
    // Whisper benefits from Metal, with the runtime's CPU fallback kept available.
    #[cfg(target_os = "macos")]
    { match engine { "canary" | "parakeet" => transcribe_cpp::Backend::Cpu, _ => transcribe_cpp::Backend::Auto } }
}

fn read_audio(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("Could not open audio: {e}"))?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16_000 || spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err("Local transcription requires 16 kHz mono PCM16 WAV audio".into());
    }
    if reader.duration() > 900 * 16_000 { return Err("Split audio into sections of at most fifteen minutes before local transcription".into()); }
    reader.samples::<i16>().map(|sample| sample.map(|v| v as f32 / 32768.0).map_err(|e| e.to_string())).collect()
}

/// Split bounded-window models near low-energy speech gaps, without duplicating audio.
fn chunk_ranges(samples: &[f32], max_samples: usize) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let max_samples = max_samples.max(1);
    while start < samples.len() {
        let limit = (start + max_samples).min(samples.len());
        let mut end = limit;
        if limit < samples.len() && max_samples > 16_000 {
            let search_start = (limit.saturating_sub(5 * 16_000)).max(start + max_samples / 2);
            let mut best = f32::MAX;
            for position in (search_start..limit.saturating_sub(1600)).step_by(1600) {
                let energy: f32 = samples[position..position + 1600].iter().map(|v| v * v).sum();
                if energy < best { best = energy; end = position + 800; }
            }
        }
        ranges.push(start..end); start = end;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunking_preserves_every_sample_and_honors_model_limits() {
        let mut pcm = vec![0.5; 83 * 16_000]; pcm[23 * 16_000..24 * 16_000].fill(0.0);
        let ranges = chunk_ranges(&pcm, 25 * 16_000);
        assert!(ranges[0].end >= 23 * 16_000 && ranges[0].end <= 24 * 16_000);
        assert_eq!(ranges.iter().map(|r| r.len()).sum::<usize>(), pcm.len());
        for pair in ranges.windows(2) { assert_eq!(pair[0].end, pair[1].start); }
        assert!(ranges.iter().all(|r| r.len() <= 25 * 16_000));
        assert!(chunk_ranges(&[], 0).is_empty());
    }

    #[test]
    fn silence_guard_does_not_discard_quiet_nonzero_audio() {
        assert!(is_digital_silence(&vec![0.0; 16_000]));
        assert!(is_digital_silence(&[]));
        assert!(!is_digital_silence(&vec![1.0 / 32768.0; 16_000]));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn scoped_cancellation_does_not_abort_an_unrelated_file_job() {
        let runtime = Arc::new(Runtime::default());
        let job = runtime.begin(Path::new("file-upload.wav")).unwrap();
        runtime.cancel_for(Path::new("voice-dictation.wav")); assert!(!job.token.is_cancelled());
        assert!(runtime.begin(Path::new("another.wav")).is_err());
        runtime.cancel_for(Path::new("file-upload.wav")); assert!(job.token.is_cancelled());
        drop(job);
        assert!(runtime.begin(Path::new("next.wav")).is_ok());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn idle_unload_never_releases_a_newer_or_active_model() {
        let runtime = Arc::new(Runtime::default());
        let first = runtime.begin(Path::new("first.wav")).unwrap(); let old_generation = first.generation;
        assert!(!runtime.release_idle(old_generation)); drop(first);
        let second = runtime.begin(Path::new("second.wav")).unwrap(); let new_generation = second.generation;
        assert!(!runtime.release_idle(old_generation)); assert!(!runtime.release_idle(new_generation)); drop(second);
        assert!(!runtime.release_idle(old_generation)); assert!(runtime.release_idle(new_generation));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn short_dictation_families_use_the_cpu_backend() {
        assert_eq!(backend_for_family("canary"), transcribe_cpp::Backend::Cpu);
        assert_eq!(backend_for_family("parakeet"), transcribe_cpp::Backend::Cpu);
        #[cfg(target_os = "macos")]
        assert_eq!(backend_for_family("whisper"), transcribe_cpp::Backend::Auto);
        #[cfg(target_os = "windows")]
        assert_eq!(backend_for_family("whisper"), transcribe_cpp::Backend::Cpu);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn shutdown_cancels_queued_work_and_rejects_late_jobs() {
        let runtime = Arc::new(Runtime::default());
        let job = runtime.begin(Path::new("queued.wav")).unwrap();
        runtime.request_shutdown(); runtime.finish_shutdown();
        assert!(job.token.is_cancelled());
        assert!(runtime.begin(Path::new("late.wav")).is_err());
        let catalog: super::super::Catalog = serde_json::from_str(include_str!("catalog.json")).unwrap();
        let request = TranscriptionRequest { audio_path: "queued.wav".into(), model: catalog.models[0].id.clone(), language: Some("en".into()), prompt: None };
        assert!(matches!(runtime.transcribe(&catalog.models[0], Path::new("missing-model"), &request, job), Err(InferenceError::Cancelled)));
        runtime.request_shutdown(); runtime.finish_shutdown();
        assert!(runtime.loaded.lock().unwrap().is_none());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn shutdown_drains_native_work_without_holding_the_job_lock() {
        use std::sync::mpsc;
        use std::time::Duration;
        let runtime = Arc::new(Runtime::default());
        let job = runtime.begin(Path::new("active.wav")).unwrap();
        let token = job.token.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_runtime = runtime.clone();
        let worker = std::thread::spawn(move || {
            let loaded = worker_runtime.loaded.lock().unwrap();
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            drop(loaded); drop(job);
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        runtime.request_shutdown(); assert!(token.is_cancelled());
        let (finished_tx, finished_rx) = mpsc::channel();
        let shutdown_runtime = runtime.clone();
        let shutdown = std::thread::spawn(move || { shutdown_runtime.finish_shutdown(); finished_tx.send(()).unwrap(); });
        assert!(finished_rx.recv_timeout(Duration::from_millis(30)).is_err());
        release_tx.send(()).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap(); shutdown.join().unwrap();
        assert!(runtime.begin(Path::new("late.wav")).is_err());
    }
}
