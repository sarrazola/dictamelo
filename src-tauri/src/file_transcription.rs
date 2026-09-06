//! Transcripción de archivos de audio arrastrados o elegidos por el usuario.
//!
//! Sin backend adicional: los formatos que el proveedor acepta y pesan poco se suben tal cual;
//! el resto se convierte en local (CoreAudio en macOS) a WAV 16 kHz mono y, si es largo, se parte
//! en tramos cortando en silencios. Los temporales se borran; el archivo original nunca se toca.

use crate::audio::{self, wav, RawRecording, TARGET_SAMPLE_RATE};
use crate::i18n::{t, tf};
use crate::platform::{self, PlatformError};
use crate::pipeline::TranscriptionRoute;
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
        jobs.truncate(MAX_JOBS);
    }
    emit(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for id in ids {
            process(&app, &id).await;
        }
    });
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

async fn process(app: &AppHandle, id: &str) {
    let job = lock(&app.state::<AppState>().file_jobs).iter().find(|j| j.id == id).cloned();
    let Some(job) = job else { return }; // lo quitaron antes de empezar
    let settings = app.state::<AppState>().settings();
    log::info!("Transcribiendo archivo «{}» ({} bytes)", job.name, job.size_bytes);
    match run(app, &settings, Path::new(&job.path), id).await {
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

async fn run(app: &AppHandle, settings: &Settings, path: &Path, id: &str) -> Result<(String, f32), String> {
    let lang = settings.ui_lang();
    let (source, temp_dir) = {
        let state = app.state::<AppState>();
        (crate::pipeline::transcription_source(&state, settings).await?, state.temp_dir.clone())
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
    let wav_path = wav::new_temp_path(&temp_dir);
    let (input, output) = (path.to_path_buf(), wav_path.clone());
    let converted = tokio::task::spawn_blocking(move || platform::decode_audio_to_wav(&input, &output))
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = converted {
        let _ = std::fs::remove_file(&wav_path);
        return Err(match e {
            PlatformError::Unsupported(_) => t(&lang, "file.unsupported").into(),
            other => tf(&lang, "file.convert_failed", &[("e", &other.to_string())]),
        });
    }
    let samples = read_wav_as_16k_mono(&wav_path);
    let _ = std::fs::remove_file(&wav_path);
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
        let chunk_path = wav::new_temp_path(&temp_dir);
        wav::write_wav_mono_i16(&chunk_path, &samples[range], TARGET_SAMPLE_RATE).map_err(|e| e.to_string())?;
        let result = crate::pipeline::transcribe_with_retry(source.provider.as_ref(), source.api_key.as_deref(), &request(chunk_path.clone())).await;
        let _ = std::fs::remove_file(&chunk_path);
        let result = result.map_err(|e| e.localized(&lang))?;
        texts.push(finish_transcript(app, settings, &source, &result, id).await?);
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
    if !settings.cleanup_enabled || result.text.trim().is_empty() {
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
    Ok(route != TranscriptionRoute::ProCloud
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
        assert_eq!(direct_upload(TranscriptionRoute::ProCloud, "mp3", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::ProCloud, "wav", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "mp3", 1024), Ok(true));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "caf", 1024), Ok(false));
        assert_eq!(direct_upload(TranscriptionRoute::OwnKey, "wav", DIRECT_UPLOAD_MAX_BYTES + 1), Ok(false));
    }
}
