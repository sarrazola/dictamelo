//! Transcripción de archivos de audio arrastrados o elegidos por el usuario.
//!
//! Sin backend adicional: los formatos que el proveedor acepta y pesan poco se suben tal cual;
//! el resto se convierte en local (CoreAudio en macOS) a WAV 16 kHz mono y, si es largo, se parte
//! en tramos cortando en silencios. Los temporales se borran; el archivo original nunca se toca.

use crate::audio::{self, wav, RawRecording, TARGET_SAMPLE_RATE};
use crate::i18n::{t, tf};
use crate::platform::{self, PlatformError};
use crate::pipeline::{TranscriptionPlan, TranscriptionRoute};
use crate::settings::Settings;
use crate::state::AppState;
use crate::transcription::{TranscriptionError, TranscriptionRequest, TranscriptionResult};
use crate::util::lock;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// Cuántos trabajos se recuerdan (los más recientes primero).
const MAX_JOBS: usize = 20;
/// Por debajo de este tamaño, y en formato nativo del proveedor, el archivo se sube sin tocar.
const DIRECT_UPLOAD_MAX_BYTES: u64 = 24 * 1024 * 1024;
const FREE_UPLOAD_MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Formatos que la API de Groq/OpenAI decodifica por sí misma.
const NATIVE_FORMATS: [&str; 10] = ["mp3", "mp4", "mpeg", "mpga", "m4a", "ogg", "oga", "wav", "webm", "flac"];
/// Duración máxima de cada tramo (10 min de WAV 16 kHz mono ≈ 19 MB, bajo el límite de 25 MB).
const CHUNK_SECS: u32 = 600;

/// Extensiones que se ofrecen en el diálogo de apertura.
pub const PICKER_EXTENSIONS: [&str; 20] = [
    "mp3", "m4a", "wav", "aac", "flac", "ogg", "oga", "opus", "webm", "mp4", "m4v", "mov", "aiff", "aif", "aifc",
    "caf", "m4b", "amr", "3gp", "mpga",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Queued,
    Converting,
    Transcribing,
    Cleaning,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileJob {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub stage: Stage,
    pub chunk: u32,
    pub chunks: u32,
    pub text: String,
    pub error: Option<String>,
    pub cleanup_warning: Option<String>,
    pub duration_secs: f32,
}

/// Añade archivos a la cola y arranca su procesamiento en orden.
pub fn enqueue(app: &AppHandle, paths: Vec<PathBuf>) {
    let state = app.state::<AppState>();
    // Every file in this batch keeps the destination chosen when the user added it.
    let plan = TranscriptionPlan::capture(&state, state.settings());
    let mut ids = Vec::new();
    {
        let mut jobs = lock(&state.file_jobs);
        for path in paths {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let job = FileJob {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                path: path.to_string_lossy().into_owned(),
                size_bytes,
                stage: Stage::Queued,
                chunk: 0,
                chunks: 0,
                text: String::new(),
                error: None,
                cleanup_warning: None,
                duration_secs: 0.0,
            };
            ids.push(job.id.clone());
            jobs.insert(0, job);
        }
        prune_completed_jobs(&mut jobs);
    }
    emit(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _worker = state.file_worker.lock().await;
        for id in ids {
            process(&app, &id, &plan).await;
        }
    });
}

fn prune_completed_jobs(jobs: &mut Vec<FileJob>) {
    let mut completed = 0;
    jobs.retain(|job| {
        if matches!(job.stage, Stage::Done | Stage::Failed) {
            completed += 1;
            completed <= MAX_JOBS
        } else {
            true // A display-history limit must never discard accepted work.
        }
    });
}

fn append_completed_text(job: &mut FileJob, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        if !job.text.is_empty() { job.text.push(' '); }
        job.text.push_str(text);
    }
}

pub fn remove(app: &AppHandle, id: &str) {
    lock(&app.state::<AppState>().file_jobs).retain(|j| j.id != id);
    emit(app);
}

pub fn clear(app: &AppHandle) {
    lock(&app.state::<AppState>().file_jobs).retain(|j| matches!(j.stage, Stage::Converting | Stage::Transcribing | Stage::Cleaning));
    emit(app);
}

fn emit(app: &AppHandle) {
    let jobs = lock(&app.state::<AppState>().file_jobs).clone();
    let _ = app.emit("file-jobs-changed", &jobs);
}

fn update(app: &AppHandle, id: &str, f: impl FnOnce(&mut FileJob)) -> bool {
    let found = {
        let state = app.state::<AppState>();
        let mut jobs = lock(&state.file_jobs);
        match jobs.iter_mut().find(|j| j.id == id) {
            Some(job) => {
                f(job);
                prune_completed_jobs(&mut jobs);
                true
            }
            None => false,
        }
    };
    if found {
        emit(app);
    }
    found
}

async fn process(app: &AppHandle, id: &str, plan: &TranscriptionPlan) {
    let job = lock(&app.state::<AppState>().file_jobs).iter().find(|j| j.id == id).cloned();
    let Some(job) = job else { return }; // lo quitaron antes de empezar
    log::info!("Transcribiendo archivo «{}» ({} bytes)", job.name, job.size_bytes);
    match run(app, plan, Path::new(&job.path), id).await {
        Ok((text, duration_secs)) => {
            log::info!("Archivo «{}» listo: {} caracteres, {:.0}s de audio", job.name, text.chars().count(), duration_secs);
            update(app, id, |j| {
                j.stage = Stage::Done;
                j.text = text;
                j.duration_secs = duration_secs;
                j.chunk = j.chunks;
            });
        }
        Err(error) => {
            log::warn!("Archivo «{}» falló: {error}", job.name);
            update(app, id, |j| {
                j.stage = Stage::Failed;
                j.error = Some(error);
            });
        }
    }
}

async fn run(app: &AppHandle, plan: &TranscriptionPlan, path: &Path, id: &str) -> Result<(String, f32), String> {
    let settings = &plan.settings;
    let lang = settings.ui_lang();
    let (source, temp_dir) = {
        let state = app.state::<AppState>();
        (crate::pipeline::transcription_source(&state, plan).await?, state.temp_dir.clone())
    };
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let size = std::fs::metadata(path).map(|m| m.len()).map_err(|e| tf(&lang, "file.read_failed", &[("e", &e.to_string())]))?;
    let request = |audio_path: PathBuf| TranscriptionRequest {
        audio_path,
        model: source.model.clone(),
        language: settings.language_code(),
        prompt: settings.vocabulary_prompt(),
    };

    // 1) Formato nativo y tamaño razonable: se sube tal cual.
    if direct_upload(source.route, &extension, size)? {
        if !update(app, id, |j| {
            j.stage = Stage::Transcribing;
            j.chunk = 1;
            j.chunks = 1;
        }) {
            return Err("cancelado".into());
        }
        let result = crate::pipeline::transcribe_with_retry(source.provider.as_ref(), source.api_key.as_deref(), &request(path.to_path_buf()))
            .await
            .map_err(|e| e.localized(&lang))?;
        let text = finish_transcript(app, settings, &source, &result, id).await?;
        return Ok((text, result.duration_secs.unwrap_or(0.0) as f32));
    }

    // 2) Conversión local a WAV 16 kHz mono.
    update(app, id, |j| j.stage = Stage::Converting);
    let (input, conversion_dir) = (path.to_path_buf(), temp_dir.clone());
    // Keep the guard in the blocking task too: cancellation of its async waiter
    // must not remove the file early while the decoder is still writing it.
    let (converted_audio, converted) = tokio::task::spawn_blocking(move || {
        let output = wav::TempAudio::new(&conversion_dir);
        let result = platform::decode_audio_to_wav(&input, output.path());
        (output, result)
    })
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = converted {
        return Err(match e {
            PlatformError::Unsupported(_) => t(&lang, "file.unsupported").into(),
            other => tf(&lang, "file.convert_failed", &[("e", &other.to_string())]),
        });
    }
    let samples = read_wav_as_16k_mono(converted_audio.path());
    drop(converted_audio);
    let samples = samples.map_err(|e| tf(&lang, "file.read_failed", &[("e", &e)]))?;
    if samples.is_empty() {
        return Err(t(&lang, "file.empty").into());
    }
    let duration_secs = samples.len() as f32 / TARGET_SAMPLE_RATE as f32;

    // 3) Tramos de como mucho CHUNK_SECS, cortados en silencios, transcritos en orden.
    let ranges = audio::split_for_upload(&samples, TARGET_SAMPLE_RATE, CHUNK_SECS);
    let total = ranges.len() as u32;
    let mut texts = Vec::with_capacity(ranges.len());
    for (index, range) in ranges.into_iter().enumerate() {
        if !update(app, id, |j| {
            j.stage = Stage::Transcribing;
            j.chunk = index as u32 + 1;
            j.chunks = total;
        }) {
            return Err("cancelado".into());
        }
        let chunk_audio = wav::TempAudio::new(&temp_dir);
        wav::write_wav_mono_i16(chunk_audio.path(), &samples[range], TARGET_SAMPLE_RATE).map_err(|e| e.to_string())?;
        let result = crate::pipeline::transcribe_with_retry(source.provider.as_ref(), source.api_key.as_deref(), &request(chunk_audio.path().to_path_buf())).await;
        let result = result.map_err(|e| e.localized(&lang))?;
        let text = finish_transcript(app, settings, &source, &result, id).await?;
        update(app, id, |job| append_completed_text(job, &text));
        texts.push(text);
    }
    Ok((texts.join(" ").trim().to_string(), duration_secs))
}

/// Clean each completed upload using the same captured route. Failed cleanup never
/// retries transcription or removes the transcript that was already produced.
async fn finish_transcript(
    app: &AppHandle,
    settings: &Settings,
    source: &crate::pipeline::TranscriptionSource,
    result: &TranscriptionResult,
    id: &str,
) -> Result<String, String> {
    if !settings.should_clean_transcript() || result.text.trim().is_empty() {
        return Ok(result.text.trim().to_string());
    }
    if !update(app, id, |job| job.stage = Stage::Cleaning) {
        return Err("cancelado".into());
    }
    let state = app.state::<AppState>();
    let cleaned = crate::pipeline::clean_text(&state, settings, source, &result.text, result.cleanup_receipt.as_deref()).await;
    let (text, error) = cleanup_or_original(&result.text, cleaned);
    if let Some(error) = error {
        log::warn!("File cleanup failed; original transcript retained: {error}");
        let warning = t(&settings.ui_lang(), "file.cleanup_failed").to_string();
        let mut first_warning = false;
        update(app, id, |job| {
            first_warning = job.cleanup_warning.is_none();
            job.cleanup_warning = Some(warning.clone());
        });
        if first_warning { let _ = app.emit("file-cleanup-warning", &warning); }
    }
    Ok(text)
}

fn cleanup_or_original(original: &str, result: Result<String, TranscriptionError>) -> (String, Option<TranscriptionError>) {
    match result {
        Ok(cleaned) if !cleaned.trim().is_empty() => (cleaned.trim().to_string(), None),
        Ok(_) => (original.trim().to_string(), None),
        Err(error) => (original.trim().to_string(), Some(error)),
    }
}

/// The original Free Cloud file must already be a small WAV. Do not silently turn
/// unsupported original formats into eligible files through the paid/BYOK converter.
fn direct_upload(route: TranscriptionRoute, extension: &str, size: u64) -> Result<bool, String> {
    if route == TranscriptionRoute::FreeCloud {
        if extension != "wav" || size > FREE_UPLOAD_MAX_BYTES {
            return Err("Free Cloud accepts WAV files up to two minutes and 4 MB. Use Pro or your own API key for other files.".into());
        }
        // PCM encoding and actual duration are independently validated by the server.
        return Ok(true);
    }
    Ok(route == TranscriptionRoute::OwnKey
        && NATIVE_FORMATS.contains(&extension)
        && size <= DIRECT_UPLOAD_MAX_BYTES)
}

/// Lee un WAV cualquiera y lo deja como PCM 16 bits mono a 16 kHz.
fn read_wav_as_16k_mono(path: &Path) -> Result<Vec<i16>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.sample_rate == TARGET_SAMPLE_RATE && spec.channels == 1 && spec.bits_per_sample == 16 && spec.sample_format == hound::SampleFormat::Int {
        return reader.samples::<i16>().collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string());
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|v| v as f32 / max)).collect::<Result<_, _>>()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
    }
    .map_err(|e| e.to_string())?;
    let raw = RawRecording { samples, sample_rate: spec.sample_rate, channels: spec.channels };
    Ok(audio::prepare(&raw).samples)
}

#[cfg(test)]
mod route_tests {
    use super::*;

    fn job(index: usize, stage: Stage) -> FileJob {
        FileJob {
            id: index.to_string(), name: "fixture.wav".into(), path: "fixture.wav".into(),
            size_bytes: 0, stage, chunk: 0, chunks: 2, text: String::new(),
            error: None, cleanup_warning: None, duration_secs: 0.0,
        }
    }

    #[test]
    fn history_limit_never_discards_accepted_or_active_jobs() {
        let mut jobs = (0..60).map(|i| job(i, Stage::Queued)).collect::<Vec<_>>();
        jobs[30].stage = Stage::Converting;
        jobs[31].stage = Stage::Transcribing;
        jobs[32].stage = Stage::Cleaning;
        jobs.extend((60..90).map(|i| job(i, Stage::Done)));
        jobs.extend((90..100).map(|i| job(i, Stage::Failed)));
        prune_completed_jobs(&mut jobs);
        assert_eq!(jobs.len(), 60 + MAX_JOBS);
        for (i, job) in jobs.iter().take(60).enumerate() { assert_eq!(job.id, i.to_string()); }
        assert_eq!(jobs.last().unwrap().id, "79");
        // The limit must also hold when queued jobs become completed.
        jobs[59].stage = Stage::Done;
        prune_completed_jobs(&mut jobs);
        assert_eq!(jobs.len(), 59 + MAX_JOBS);
        assert_eq!(jobs.last().unwrap().id, "78");
    }

    #[test]
    fn later_chunk_failure_retains_completed_text() {
        let mut result = job(0, Stage::Transcribing);
        append_completed_text(&mut result, " First chunk. ");
        append_completed_text(&mut result, " ");
        append_completed_text(&mut result, " Second chunk. ");
        result.stage = Stage::Failed;
        result.error = Some("later chunk failed".into());
        assert_eq!(result.text, "First chunk. Second chunk.");
        assert_eq!(result.stage, Stage::Failed);
    }

    #[test]
    fn free_original_formats_cannot_bypass_limits_through_conversion() {
        for extension in ["caf", "aiff", "mp3", "m4a", "flac"] {
            assert!(direct_upload(TranscriptionRoute::FreeCloud, extension, 1024).is_err());
        }
        assert_eq!(direct_upload(TranscriptionRoute::FreeCloud, "wav", FREE_UPLOAD_MAX_BYTES), Ok(true));
        assert!(direct_upload(TranscriptionRoute::FreeCloud, "wav", FREE_UPLOAD_MAX_BYTES + 1).is_err());
    }

    #[test]
    fn cleanup_failure_retains_completed_file_transcript() {
        let raw = " eh send it Thursday no Friday ";
        let (text, warning) = cleanup_or_original(raw, Err(TranscriptionError::Timeout));
        assert_eq!(text, raw.trim());
        assert!(warning.is_some());
        assert_eq!(cleanup_or_original(raw, Ok(String::new())).0, raw.trim());
        assert_eq!(cleanup_or_original(raw, Ok(" Send it Friday. ".into())).0, "Send it Friday.");
    }

    #[test]
    fn bundled_english_audio_fixture_decodes() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/english-speech.wav");
        let samples = read_wav_as_16k_mono(&fixture).expect("decode committed speech fixture");
        assert_eq!(samples.len(), 93_680);
        assert!(samples.iter().any(|sample| sample.unsigned_abs() > 100));
        assert_eq!(direct_upload(TranscriptionRoute::FreeCloud, "wav", std::fs::metadata(&fixture).unwrap().len()), Ok(true));
        let chunks = audio::split_for_upload(&samples, TARGET_SAMPLE_RATE, CHUNK_SECS);
        assert_eq!(chunks, vec![0..93_680]);
    }

    #[test]
    fn pro_always_converts_and_personal_keys_keep_native_uploads() {
        // Local engines always receive normalized 16 kHz mono PCM, even for WAV inputs.
        assert_eq!(direct_upload(TranscriptionRoute::Local, "mp3", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::Local, "wav", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::ProCloud, "mp3", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::ProCloud, "wav", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "mp3", 1024), Ok(true));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "caf", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "wav", DIRECT_UPLOAD_MAX_BYTES + 1), Ok(false));
    }
}
