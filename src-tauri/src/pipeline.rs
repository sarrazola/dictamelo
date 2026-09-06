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
use std::sync::Arc;
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
    // Con Pro la credencial es la licencia, no una API key del usuario: no hay nada que revisar.
    if state.uses_cloud() {
        return start_stream(app, &state, &settings, &lang).await;
    }
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

    start_stream(app, &state, &settings, &lang).await
}

/// Abre el micrófono y pasa a «grabando». Se separa porque el camino Pro se salta las
/// comprobaciones de API key pero necesita exactamente lo mismo a partir de aquí.
async fn start_stream(app: &AppHandle, state: &AppState, settings: &Settings, lang: &str) {
    if platform::permissions_status().microphone == PermissionState::Denied {
        fail(app, t(lang, "err.mic_denied"));
        app_windows::show_settings(app);
        return;
    }
    match state.recorder.start(settings.input_device.clone()).await {
        Ok(()) => {
            let generation = set_status(app, Status::Recording);
            sound(app, SoundKind::Start);
            spawn_level_monitor(app.clone(), generation);
            let max_seconds = if state.is_free_cloud() {
                settings.max_recording_secs.min(119)
            } else if state.uses_cloud() {
                settings.max_recording_secs.min(599)
            } else {
                settings.max_recording_secs
            };
            spawn_watchdog(app.clone(), generation, max_seconds);
        }
        Err(e) => fail(app, e.localized(lang)),
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
    let source = match transcription_source(&state, &settings).await {
        Ok(pair) => pair,
        Err(e) => {
            fail(app, tf(&lang, "err.keychain", &[("e", &e)]));
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
        model: source.model.clone(),
        language: settings.language_code(),
        prompt: settings.vocabulary_prompt(),
    };
    let started = Instant::now();
    let result = transcribe_with_retry(source.provider.as_ref(), source.api_key.as_deref(), &request).await;

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
                match clean_text(&state, &settings, &source, &result.text, result.cleanup_receipt.as_deref()).await {
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
            // History records the provider/model that processed this request, even if the
            // user changes their preferred personal provider while the request is in flight.
            let mut delivery_settings = settings.clone();
            delivery_settings.provider = source.provider.info().id;
            delivery_settings.model = source.model.clone();
            deliver(app, text.clone(), result.language, audio.duration_secs(), &delivery_settings, cleanup_failed).await;
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

/// One route is captured before a request. A later UI mode change applies to the next
/// request, not to cleanup, billing credentials, file conversion or history for this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptionRoute {
    OwnKey,
    FreeCloud,
    ProCloud,
}

impl TranscriptionRoute {
    fn from_flags(configured: bool, own_key: bool, pro: bool, signed_in: bool) -> Self {
        if !configured || own_key {
            Self::OwnKey
        } else if pro {
            Self::ProCloud
        } else if signed_in {
            Self::FreeCloud
        } else {
            Self::OwnKey
        }
    }
}

pub(crate) struct TranscriptionSource {
    pub provider: Arc<dyn TranscriptionProvider>,
    pub api_key: Option<String>,
    pub route: TranscriptionRoute,
    pub model: String,
}

fn source_model(route: TranscriptionRoute, personal_model: &str, provider: &crate::transcription::ProviderInfo) -> String {
    if route == TranscriptionRoute::OwnKey { personal_model.to_string() } else { provider.default_model.clone() }
}

pub(crate) async fn transcription_source(
    state: &AppState,
    settings: &Settings,
) -> Result<TranscriptionSource, String> {
    let configured = crate::cloud_config::configured();
    let pro = state.is_pro();
    let signed_in = configured && !settings.use_own_key && !pro && state.account.signed_in();
    let route = TranscriptionRoute::from_flags(configured, settings.use_own_key, pro, signed_in);
    let (provider, api_key) = match route {
        TranscriptionRoute::ProCloud => (state.backend_provider.clone(), crate::license::stored_key(&state.secrets)),
        TranscriptionRoute::FreeCloud => (state.backend_provider.clone(), Some(format!("Bearer {}", state.account.token().await?))),
        TranscriptionRoute::OwnKey => {
            let provider = state.providers.get(&settings.provider)
                .ok_or_else(|| format!("Unknown transcription provider: {}", settings.provider))?;
            let key = state.api_key_for(&settings.provider).map_err(|e| e.to_string())?;
            (provider, key)
        }
    };
    let model = source_model(route, &settings.model, &provider.info());
    Ok(TranscriptionSource { provider, api_key, route, model })
}

/// Cleanup keeps the transcription request's captured route and cloud credential.
pub(crate) async fn clean_text(state: &AppState, settings: &Settings, source: &TranscriptionSource, text: &str, cleanup_receipt: Option<&str>) -> Result<String, TranscriptionError> {
    let (cleaner, api_key, model) = if source.route != TranscriptionRoute::OwnKey {
        let cleaner = state.backend_cleaner.clone();
        let model = cleaner.info().default_model;
        (cleaner, source.api_key.clone(), model)
    } else {
        let cleaner = state
            .cleaners
            .get(&settings.cleanup_provider)
            .ok_or_else(|| TranscriptionError::Rejected(format!("Unknown cleanup provider: {}", settings.cleanup_provider)))?;
        let key_provider = cleaner.info().key_provider;
        let key = state
            .api_key_for(&key_provider)
            .map_err(|e| TranscriptionError::Rejected(e.to_string()))?;
        (cleaner, key, settings.cleanup_model.clone())
    };
    cleaner
        .clean(api_key.as_deref(), &model, &settings.cleanup_system_prompt(), text, cleanup_receipt)
        .await
}

pub(crate) async fn transcribe_with_retry(
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

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn personal_keys_and_unconfigured_builds_never_select_hosted_billing() {
        for configured in [false, true] {
            for pro in [false, true] {
                for signed_in in [false, true] {
                    assert_eq!(TranscriptionRoute::from_flags(configured, true, pro, signed_in), TranscriptionRoute::OwnKey);
                    assert_eq!(TranscriptionRoute::from_flags(false, false, pro, signed_in), TranscriptionRoute::OwnKey);
                }
            }
        }
        assert_eq!(TranscriptionRoute::from_flags(true, false, true, true), TranscriptionRoute::ProCloud);
        assert_eq!(TranscriptionRoute::from_flags(true, false, false, true), TranscriptionRoute::FreeCloud);
        assert_eq!(TranscriptionRoute::from_flags(true, false, false, false), TranscriptionRoute::OwnKey);
    }

    #[test]
    fn cloud_request_and_history_metadata_do_not_inherit_a_personal_model() {
        let provider = crate::transcription::dictamelo::DictameloProvider::new(crate::transcription::shared_http_client()).info();
        assert_eq!(provider.id, "dictamelo");
        for route in [TranscriptionRoute::FreeCloud, TranscriptionRoute::ProCloud] {
            assert_eq!(source_model(route, "gpt-4o-transcribe", &provider), "whisper-large-v3-turbo");
            assert_eq!(source_model(route, "whisper-large-v3", &provider), "whisper-large-v3-turbo");
        }
        assert_eq!(source_model(TranscriptionRoute::OwnKey, "gpt-4o-transcribe", &provider), "gpt-4o-transcribe");
    }
}
