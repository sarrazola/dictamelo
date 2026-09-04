//! Flujo principal: pulsar atajo → grabar → soltar → transcribir → pegar, con recuperación
//! de errores (red, permisos, micrófono) y limpieza de temporales.

use crate::audio::{self, wav, PreparedAudio};
use crate::history::HistoryEntry;
use crate::paste::{self, PasteError};
use crate::platform::{self, PermissionState, PlatformError};
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
        other => log::info!("Estado: {}", other.label()),
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

pub fn current_status(app: &AppHandle) -> Status {
    lock(&app.state::<AppState>().status).clone()
}

fn fail(app: &AppHandle, message: impl Into<String>) {
    set_status(app, Status::Error { message: message.into() });
}

async fn start_recording(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.hotkey_suspended.load(Ordering::SeqCst) {
        return;
    }
    {
        let status = lock(&state.status);
        if status.is_busy() {
            log::debug!("Pulsación ignorada (estado: {})", status.label());
            return;
        }
    }
    let settings = state.settings();

    let Some(provider) = state.providers.get(&settings.provider) else {
        fail(app, format!("Proveedor desconocido: {}", settings.provider));
        return;
    };
    let info = provider.info();
    if info.requires_api_key {
        match state.api_key_for(&info.id) {
            Ok(Some(key)) if !key.trim().is_empty() => {}
            Ok(_) => {
                fail(app, format!("Configura tu API key de {} en Configuración", info.name));
                app_windows::show_settings(app);
                return;
            }
            Err(e) => {
                fail(app, format!("No se pudo leer la API key del llavero: {e}"));
                return;
            }
        }
    }

    match platform::permissions_status().microphone {
        PermissionState::Denied => {
            fail(app, "Sin acceso al micrófono. Actívalo en Ajustes del Sistema → Privacidad → Micrófono");
            app_windows::show_settings(app);
            return;
        }
        PermissionState::NotDetermined => {
            let handle = app.clone();
            platform::request_microphone_permission(Box::new(move |granted| {
                log::info!("Permiso de micrófono {}", if granted { "concedido" } else { "denegado" });
                let _ = handle.emit("permissions-changed", granted);
            }));
            fail(app, "Concede acceso al micrófono y vuelve a intentarlo");
            return;
        }
        _ => {}
    }

    match state.recorder.start(settings.input_device.clone()).await {
        Ok(()) => {
            let generation = set_status(app, Status::Recording);
            spawn_level_monitor(app.clone(), generation);
            spawn_watchdog(app.clone(), generation, settings.max_recording_secs);
        }
        Err(e) => fail(app, e.to_string()),
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
    let raw = match state.recorder.stop().await {
        Ok(raw) => raw,
        Err(e) => {
            fail(app, e.to_string());
            return;
        }
    };
    let prepared = audio::prepare(&raw);
    let secs = prepared.duration_secs();
    log::info!("Grabación de {secs:.2}s ({} Hz, {} canal(es))", raw.sample_rate, raw.channels);
    if secs < MIN_RECORDING_SECS {
        set_status(app, Status::Done { message: "Grabación demasiado corta".into() });
        return;
    }
    set_status(app, Status::Transcribing);
    transcribe_and_deliver(app, prepared).await;
}

/// Transcribe y entrega el audio. Devuelve el texto si la transcripción tuvo éxito.
pub(crate) async fn transcribe_and_deliver(app: &AppHandle, audio: PreparedAudio) -> Option<String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let Some(provider) = state.providers.get(&settings.provider) else {
        fail(app, format!("Proveedor desconocido: {}", settings.provider));
        return None;
    };
    let api_key = match state.api_key_for(&settings.provider) {
        Ok(key) => key,
        Err(e) => {
            fail(app, format!("No se pudo leer la API key del llavero: {e}"));
            return None;
        }
    };

    let path = wav::new_temp_path(&state.temp_dir);
    if let Err(e) = wav::write_wav_mono_i16(&path, &audio.samples, audio.sample_rate) {
        fail(app, format!("No se pudo escribir el audio temporal: {e}"));
        return None;
    }
    let request = TranscriptionRequest {
        audio_path: path.clone(),
        model: settings.model.clone(),
        language: settings.language_code(),
        prompt: None,
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
                set_status(app, Status::Done { message: "No se detectó voz".into() });
                return Some(String::new());
            }
            let text = result.text.trim().to_string();
            deliver(app, result, audio.duration_secs(), &settings).await;
            Some(text)
        }
        Err(e) => {
            let attempts = lock(&state.last_failed).as_ref().map(|p| p.attempts).unwrap_or(0) + 1;
            *lock(&state.last_failed) = Some(PendingTranscription { audio, attempts });
            tray::set_retry_enabled(app, true);
            fail(app, format!("{e}. Puedes reintentar desde el menú de la barra."));
            None
        }
    }
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

async fn deliver(app: &AppHandle, result: TranscriptionResult, duration_secs: f32, settings: &Settings) {
    let text = result.text.trim().to_string();
    let mut pasted = false;
    if settings.auto_paste {
        set_status(app, Status::Pasting);
        match paste::paste_text(&text, settings.restore_clipboard).await {
            Ok(outcome) => {
                pasted = true;
                let message = if outcome.clipboard_restored || !settings.restore_clipboard {
                    "Texto pegado"
                } else {
                    "Texto pegado (el portapapeles cambió y no se restauró)"
                };
                set_status(app, Status::Done { message: message.into() });
            }
            Err(PasteError::Keystroke(PlatformError::AccessibilityDenied)) => {
                fail(app, "Sin permiso de Accesibilidad: el texto quedó copiado en el portapapeles");
                app_windows::show_settings(app);
            }
            Err(e) => {
                let _ = paste::copy_text(&text);
                fail(app, format!("No se pudo pegar ({e}); el texto quedó en el portapapeles"));
            }
        }
    } else {
        match paste::copy_text(&text) {
            Ok(()) => {
                set_status(app, Status::Done { message: "Texto copiado al portapapeles".into() });
            }
            Err(e) => fail(app, format!("No se pudo copiar al portapapeles: {e}")),
        }
    }
    record_history(app, text, result.language, duration_secs, settings, pasted);
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
            set_status(app, Status::Done { message: "No hay nada que reintentar".into() });
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
