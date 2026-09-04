//! Flujo principal: pulsar atajo → grabar → soltar → transcribir → pegar, con recuperación
//! de errores (red, permisos, micrófono) y limpieza de temporales.

use crate::audio::{self, wav, PreparedAudio};
use crate::history::HistoryEntry;
use crate::i18n::{t, tf};
use crate::paste::{self, PasteError};
use crate::platform::{self, PermissionState, PlatformError, SoundKind};
use crate::settings::Settings;
use crate::state::{AppState, PendingTranscription};
use crate::status::Status;
use crate::transcription::{TranscriptionError, TranscriptionProvider, TranscriptionRequest, TranscriptionResult};
use crate::util::lock;
use crate::{app_windows, tray};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Grabaciones más cortas se descartan (pulsaciones accidentales).
const MIN_RECORDING_SECS: f32 = 0.35;
const DONE_VISIBLE: Duration = Duration::from_millis(2200);
const ERROR_VISIBLE: Duration = Duration::from_secs(6);
const LEVEL_INTERVAL: Duration = Duration::from_millis(80);
const RETRY_DELAY: Duration = Duration::from_millis(1500);

pub fn hotkey_pressed(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { start_recording(&app).await });
}

pub fn hotkey_released(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { stop_and_transcribe(&app).await });
}

/// Cambia el estado y lo propaga (evento, bandeja, indicador). Devuelve la "generación".
pub fn set_status(app: &AppHandle, status: Status) -> u64 {
    let state = app.state::<AppState>();
    let generation = state.status_generation.fetch_add(1, Ordering::SeqCst) + 1;
    *lock(&state.status) = status.clone();
    match &status {
        Status::Error { message } => log::warn!("Estado: error: {message}"),
        other => log::info!("Estado: {}", other.label(&state.settings().ui_lang())),
    }
    let _ = app.emit("status", &status);
    tray::update(app, &status);
    app_windows::update_overlay(app, &status);

    let auto_reset = match &status {
        Status::Done { .. } => Some(DONE_VISIBLE),
        Status::Error { .. } => Some(ERROR_VISIBLE),
        _ => None,
    };
    if let Some(after) = auto_reset {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(after).await;
            if app.state::<AppState>().status_generation.load(Ordering::SeqCst) == generation {
                set_status(&app, Status::Idle);
            }
        });
    }
    generation
}

/// Idioma actual de la interfaz, para los mensajes que ve el usuario.
fn settings_lang(app: &AppHandle) -> String {
    app.state::<AppState>().settings().ui_lang()
}

pub fn current_status(app: &AppHandle) -> Status {
    lock(&app.state::<AppState>().status).clone()
}

fn fail(app: &AppHandle, message: impl Into<String>) {
    set_status(app, Status::Error { message: message.into() });
    sound(app, SoundKind::Error);
}

/// Sonido de aviso, si el usuario los tiene activados.
fn sound(app: &AppHandle, kind: SoundKind) {
    if app.state::<AppState>().settings().play_sounds {
        platform::play_sound(app, kind);
    }
}

/// Esc durante la grabación: descarta el audio sin transcribir.
pub fn cancel_recording(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if !matches!(*lock(&state.status), Status::Recording) {
            return;
        }
        let lang = state.settings().ui_lang();
        log::info!("Grabación cancelada con Esc");
        // Primero el estado, para que al soltar el atajo no se intente transcribir.
        set_status(&app, Status::Done { message: t(&lang, "msg.cancelled").into() });
        state.recorder.cancel();
        sound(&app, SoundKind::Stop);
    });
}

async fn start_recording(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.hotkey_suspended.load(Ordering::SeqCst) {
        return;
    }
    {
        let status = lock(&state.status);
        if status.is_busy() {
            log::debug!("Pulsación ignorada (estado: {})", status.label(&settings_lang(app)));
            return;
        }
    }
    let settings = state.settings();

    let lang = settings.ui_lang();
    let Some(provider) = state.providers.get(&settings.provider) else {
        fail(app, tf(&lang, "err.provider_unknown", &[("p", &settings.provider)]));
        return;
    };
    let info = provider.info();
    if info.requires_api_key {
        match state.api_key_for(&info.id) {
            Ok(Some(key)) if !key.trim().is_empty() => {}
            Ok(_) => {
                fail(app, tf(&lang, "err.api_key_missing", &[("p", &info.name)]));
                app_windows::show_settings(app);
                return;
            }
            Err(e) => {
                fail(app, tf(&lang, "err.keychain", &[("e", &e.to_string())]));
                return;
            }
        }
    }

    match platform::permissions_status().microphone {
        PermissionState::Denied => {
            fail(app, t(&lang, "err.mic_denied"));
            app_windows::show_settings(app);
            return;
        }
        PermissionState::NotDetermined => {
            let handle = app.clone();
            platform::request_microphone_permission(Box::new(move |granted| {
                log::info!("Permiso de micrófono {}", if granted { "concedido" } else { "denegado" });
                let _ = handle.emit("permissions-changed", granted);
            }));
            fail(app, t(&lang, "err.mic_pending"));
            return;
        }
        _ => {}
    }

    match state.recorder.start(settings.input_device.clone()).await {
        Ok(()) => {
            let generation = set_status(app, Status::Recording);
            sound(app, SoundKind::Start);
            spawn_level_monitor(app.clone(), generation);
            spawn_watchdog(app.clone(), generation, settings.max_recording_secs);
        }
        Err(e) => fail(app, e.localized(&lang)),
    }
}

fn spawn_level_monitor(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(LEVEL_INTERVAL).await;
            let state = app.state::<AppState>();
            if state.status_generation.load(Ordering::SeqCst) != generation {
                break;
            }
            let _ = app.emit("audio-level", state.recorder.level());
        }
    });
}

fn spawn_watchdog(app: AppHandle, generation: u64, max_secs: u32) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(u64::from(max_secs))).await;
        if app.state::<AppState>().status_generation.load(Ordering::SeqCst) == generation {
            log::info!("Duración máxima ({max_secs}s) alcanzada; se detiene la grabación");
            stop_and_transcribe(&app).await;
        }
    });
}

async fn stop_and_transcribe(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.recorder.is_recording() {
        return;
    }
    if !matches!(*lock(&state.status), Status::Recording) {
        state.recorder.cancel();
        return;
    }
    let lang = state.settings().ui_lang();
    let raw = match state.recorder.stop().await {
        Ok(raw) => raw,
        Err(e) => {
            fail(app, e.localized(&lang));
            return;
        }
    };
    sound(app, SoundKind::Stop);
    let prepared = audio::prepare(&raw);
    let secs = prepared.duration_secs();
    log::info!("Grabación de {secs:.2}s ({} Hz, {} canal(es))", raw.sample_rate, raw.channels);
    if secs < MIN_RECORDING_SECS {
        set_status(app, Status::Done { message: t(&lang, "msg.too_short").into() });
        return;
    }
    set_status(app, Status::Transcribing);
    transcribe_and_deliver(app, prepared).await;
}

/// Transcribe y entrega el audio. Devuelve el texto si la transcripción tuvo éxito.
pub(crate) async fn transcribe_and_deliver(app: &AppHandle, audio: PreparedAudio) -> Option<String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let lang = settings.ui_lang();
    let Some(provider) = state.providers.get(&settings.provider) else {
        fail(app, tf(&lang, "err.provider_unknown", &[("p", &settings.provider)]));
        return None;
    };
    let api_key = match state.api_key_for(&settings.provider) {
        Ok(key) => key,
        Err(e) => {
            fail(app, tf(&lang, "err.keychain", &[("e", &e.to_string())]));
            return None;
        }
    };

    let path = wav::new_temp_path(&state.temp_dir);
    if let Err(e) = wav::write_wav_mono_i16(&path, &audio.samples, audio.sample_rate) {
        fail(app, tf(&lang, "err.temp_write", &[("e", &e.to_string())]));
        return None;
    }
    let request = TranscriptionRequest {
        audio_path: path.clone(),
        model: settings.model.clone(),
        language: settings.language_code(),
        prompt: settings.vocabulary_prompt(),
    };
    let started = Instant::now();
    let result = transcribe_with_retry(provider.as_ref(), api_key.as_deref(), &request).await;

    // El audio temporal se elimina siempre, haya ido bien o mal.
    match std::fs::remove_file(&path) {
        Ok(()) => log::debug!("Audio temporal eliminado: {}", path.display()),
        Err(e) => log::warn!("No se pudo eliminar {}: {e}", path.display()),
    }

    match result {
        Ok(result) => {
            log::info!(
                "Transcripción lista en {:.1}s ({} caracteres, idioma {:?})",
                started.elapsed().as_secs_f32(),
                result.text.chars().count(),
                result.language
            );
            *lock(&state.last_failed) = None;
            tray::set_retry_enabled(app, false);
            if result.text.trim().is_empty() {
                set_status(app, Status::Done { message: t(&lang, "msg.no_speech").into() });
                return Some(String::new());
            }
            let mut text = result.text.trim().to_string();
            let mut cleanup_failed = false;
            if settings.cleanup_enabled {
                set_status(app, Status::Cleaning);
                let started = Instant::now();
                match clean_text(&state, &settings, &text).await {
                    Ok(cleaned) if !cleaned.trim().is_empty() => {
                        log::info!("Texto limpio en {:.1}s ({} → {} caracteres)", started.elapsed().as_secs_f32(), text.chars().count(), cleaned.chars().count());
                        text = cleaned.trim().to_string();
                    }
                    Ok(_) => log::info!("La limpieza devolvió vacío; se conserva el texto original"),
                    Err(e) => {
                        // La limpieza es un extra: si falla, se pega el texto tal cual y se avisa.
                        log::warn!("Limpieza con IA fallida: {e}");
                        cleanup_failed = true;
                    }
                }
            }
            deliver(app, text.clone(), result.language, audio.duration_secs(), &settings, cleanup_failed).await;
            Some(text)
        }
        Err(e) => {
            let attempts = lock(&state.last_failed).as_ref().map(|p| p.attempts).unwrap_or(0) + 1;
            *lock(&state.last_failed) = Some(PendingTranscription { audio, attempts });
            tray::set_retry_enabled(app, true);
            fail(app, tf(&lang, "err.retry_hint", &[("e", &e.localized(&lang))]));
            None
        }
    }
}

/// Pasa el texto por el modelo de limpieza configurado.
async fn clean_text(state: &AppState, settings: &Settings, text: &str) -> Result<String, TranscriptionError> {
    let cleaner = state
        .cleaners
        .get(&settings.cleanup_provider)
        .ok_or_else(|| TranscriptionError::Rejected(format!("limpiador desconocido: {}", settings.cleanup_provider)))?;
    let key_provider = cleaner.info().key_provider;
    let api_key = state
        .api_key_for(&key_provider)
        .map_err(|e| TranscriptionError::Rejected(e.to_string()))?;
    cleaner
        .clean(api_key.as_deref(), &settings.cleanup_model, &settings.cleanup_system_prompt(), text)
        .await
}

async fn transcribe_with_retry(
    provider: &dyn TranscriptionProvider,
    api_key: Option<&str>,
    request: &TranscriptionRequest,
) -> Result<TranscriptionResult, TranscriptionError> {
    match provider.transcribe(api_key, request).await {
        Err(e) if e.is_retryable() => {
            log::warn!("Fallo transitorio ({e}); reintentando en {:.1}s", RETRY_DELAY.as_secs_f32());
            tokio::time::sleep(RETRY_DELAY).await;
            provider.transcribe(api_key, request).await
        }
        other => other,
    }
}

async fn deliver(
    app: &AppHandle,
    text: String,
    language: Option<String>,
    duration_secs: f32,
    settings: &Settings,
    cleanup_failed: bool,
) {
    let lang = settings.ui_lang();
    let mut pasted = false;
    if settings.auto_paste {
        set_status(app, Status::Pasting);
        match paste::paste_text(&text, settings.restore_clipboard).await {
            Ok(outcome) => {
                pasted = true;
                let key = if cleanup_failed {
                    "msg.pasted_uncleaned"
                } else if outcome.clipboard_restored || !settings.restore_clipboard {
                    "msg.pasted"
                } else {
                    "msg.pasted_kept"
                };
                set_status(app, Status::Done { message: t(&lang, key).into() });
            }
            Err(PasteError::Keystroke(PlatformError::AccessibilityDenied)) => {
                fail(app, t(&lang, "err.ax_denied"));
                app_windows::show_settings(app);
            }
            Err(e) => {
                let _ = paste::copy_text(&text);
                fail(app, tf(&lang, "err.paste_failed", &[("e", &e.to_string())]));
            }
        }
    } else {
        match paste::copy_text(&text) {
            Ok(()) => {
                set_status(app, Status::Done { message: t(&lang, "msg.copied").into() });
            }
            Err(e) => fail(app, tf(&lang, "err.copy_failed", &[("e", &e.to_string())])),
        }
    }
    record_history(app, text, language, duration_secs, settings, pasted);
}

fn record_history(app: &AppHandle, text: String, language: Option<String>, duration_secs: f32, settings: &Settings, pasted: bool) {
    let state = app.state::<AppState>();
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        text,
        duration_ms: (duration_secs * 1000.0) as u64,
        provider: settings.provider.clone(),
        model: settings.model.clone(),
        language,
        pasted,
    };
    if let Err(e) = lock(&state.history).push(entry, settings.max_history) {
        log::warn!("No se pudo guardar el historial: {e}");
    }
    let _ = app.emit("history-changed", ());
}

/// Reintenta la última transcripción fallida (el audio se conserva en memoria, no en disco).
pub async fn retry_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    if lock(&state.status).is_busy() {
        return;
    }
    let pending = lock(&state.last_failed).take();
    match pending {
        Some(pending) => {
            log::info!("Reintentando transcripción (intento {})", pending.attempts + 1);
            set_status(app, Status::Transcribing);
            // Conservamos el conteo de intentos si vuelve a fallar.
            *lock(&state.last_failed) = Some(PendingTranscription { audio: pending.audio.clone(), attempts: pending.attempts });
            let _ = transcribe_and_deliver(app, pending.audio).await;
        }
        None => {
            let lang = state.settings().ui_lang();
            set_status(app, Status::Done { message: t(&lang, "msg.nothing_retry").into() });
        }
    }
}

/// Al arrancar: si falta la API key o algún permiso, abre la ventana de configuración.
pub fn startup_checks(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let key_ok = state
        .api_key_for(&settings.provider)
        .ok()
        .flatten()
        .is_some_and(|k| !k.trim().is_empty());
    let permissions = platform::permissions_status();
    log::info!(
        "Arranque: api_key={key_ok} micrófono={:?} accesibilidad={:?}",
        permissions.microphone,
        permissions.accessibility
    );
    if !key_ok || !permissions.all_granted() {
        app_windows::show_settings(app);
    }
}
