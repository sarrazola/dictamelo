//! Transcripción de archivos de audio arrastrados o elegidos por el usuario.
//!
//! Sin backend adicional: los formatos que el proveedor acepta y pesan poco se suben tal cual;
//! el resto se convierte en local (CoreAudio en macOS) a WAV 16 kHz mono y, si es largo, se parte
//! en tramos cortando en silencios. Los temporales se borran; el archivo original nunca se toca.

use crate::audio::{self, wav, RawRecording, TARGET_SAMPLE_RATE};
use crate::i18n::{t, tf};
use crate::platform::{self, PlatformError};
use crate::settings::Settings;
use crate::state::AppState;
use crate::transcription::TranscriptionRequest;
use crate::util::lock;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// Cuántos trabajos se recuerdan (los más recientes primero).
const MAX_JOBS: usize = 20;
/// Por debajo de este tamaño, y en formato nativo del proveedor, el archivo se sube sin tocar.
const DIRECT_UPLOAD_MAX_BYTES: u64 = 24 * 1024 * 1024;
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
    lock(&app.state::<AppState>().file_jobs).retain(|j| matches!(j.stage, Stage::Converting | Stage::Transcribing));
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
    let (provider, api_key, temp_dir) = {
        let state = app.state::<AppState>();
        let provider = state
            .providers
            .get(&settings.provider)
            .ok_or_else(|| tf(&lang, "err.provider_unknown", &[("p", &settings.provider)]))?;
        let api_key = state
            .api_key_for(&settings.provider)
            .map_err(|e| tf(&lang, "err.keychain", &[("e", &e.to_string())]))?;
        (provider, api_key, state.temp_dir.clone())
    };
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let size = std::fs::metadata(path).map(|m| m.len()).map_err(|e| tf(&lang, "file.read_failed", &[("e", &e.to_string())]))?;
    let request = |audio_path: PathBuf| TranscriptionRequest {
        audio_path,
        model: settings.model.clone(),
        language: settings.language_code(),
        prompt: settings.vocabulary_prompt(),
    };

    // 1) Formato nativo y tamaño razonable: se sube tal cual.
    if NATIVE_FORMATS.contains(&extension.as_str()) && size <= DIRECT_UPLOAD_MAX_BYTES {
        if !update(app, id, |j| {
            j.stage = Stage::Transcribing;
            j.chunk = 1;
            j.chunks = 1;
        }) {
            return Err("cancelado".into());
        }
        let result = crate::pipeline::transcribe_with_retry(provider.as_ref(), api_key.as_deref(), &request(path.to_path_buf()))
            .await
            .map_err(|e| e.localized(&lang))?;
        return Ok((result.text.trim().to_string(), result.duration_secs.unwrap_or(0.0) as f32));
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
        let result = crate::pipeline::transcribe_with_retry(provider.as_ref(), api_key.as_deref(), &request(chunk_path.clone())).await;
        let _ = std::fs::remove_file(&chunk_path);
        texts.push(result.map_err(|e| e.localized(&lang))?.text.trim().to_string());
    }
    Ok((texts.join(" ").trim().to_string(), duration_secs))
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
