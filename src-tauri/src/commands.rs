//! Comandos invocables desde la interfaz (Tauri IPC). Devuelven mensajes en español.

use crate::history::HistoryEntry;
use crate::platform::{self, PermissionKind, PermissionsStatus};
use crate::settings::{Settings, DEFAULT_HOTKEY};
use crate::state::AppState;
use crate::status::Status;
use crate::transcription::ProviderInfo;
use crate::util::{lock, write};
use crate::{app_windows, audio, hotkey, paste, pipeline, tray};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    let new = settings.sanitized();
    hotkey::validate(&new.hotkey)?;
    let provider = state
        .providers
        .get(&new.provider)
        .ok_or_else(|| format!("Proveedor desconocido: {}", new.provider))?;
    let info = provider.info();
    if !info.models.iter().any(|m| m.id == new.model) {
        return Err(format!("El modelo «{}» no existe en {}", new.model, info.name));
    }

    if let Some(cleaner) = state.cleaners.get(&new.cleanup_provider) {
        if !cleaner.info().models.iter().any(|m| m.id == new.cleanup_model) {
            return Err(format!("El modelo de limpieza «{}» no existe", new.cleanup_model));
        }
    } else {
        return Err(format!("Limpiador desconocido: {}", new.cleanup_provider));
    }

    let old = state.settings();
    if old.launch_at_login != new.launch_at_login {
        crate::autostart::set_enabled(&app, new.launch_at_login)?;
    }
    new.save(&state.settings_path).map_err(|e| format!("No se pudo guardar la configuración: {e}"))?;
    *write(&state.settings) = new.clone();

    if old.hotkey != new.hotkey && !state.hotkey_suspended.load(Ordering::SeqCst) {
        hotkey::apply(&app, &new.hotkey)?;
    }
    if old.auto_paste != new.auto_paste {
        tray::set_autopaste_checked(&app, new.auto_paste);
    }
    // El menú de la barra muestra el atajo y usa el idioma de la interfaz.
    if old.ui_language != new.ui_language || old.hotkey != new.hotkey {
        tray::relabel(&app);
    }
    if !new.show_overlay {
        app_windows::hide_overlay(&app);
    }
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

#[tauri::command]
pub fn get_status(app: AppHandle) -> Status {
    pipeline::current_status(&app)
}

#[tauri::command]
pub fn get_providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state.providers.list()
}

#[tauri::command]
pub fn get_cleaners(state: State<'_, AppState>) -> Vec<crate::cleanup::CleanerInfo> {
    state.cleaners.list()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub configured: bool,
    /// Últimos caracteres de la clave, para que el usuario la reconozca sin mostrarla.
    pub hint: Option<String>,
}

#[tauri::command]
pub fn get_api_key_status(state: State<'_, AppState>, provider: String) -> Result<ApiKeyStatus, String> {
    match state.secrets.get(&provider).map_err(|e| e.to_string())? {
        Some(key) if !key.trim().is_empty() => {
            let key = key.trim();
            let hint = if key.chars().count() > 8 {
                Some(format!("…{}", key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>()))
            } else {
                None
            };
            Ok(ApiKeyStatus { configured: true, hint })
        }
        _ => Ok(ApiKeyStatus { configured: false, hint: None }),
    }
}

#[tauri::command]
pub fn set_api_key(state: State<'_, AppState>, provider: String, api_key: String) -> Result<(), String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("La API key está vacía".into());
    }
    state.providers.get(&provider).ok_or_else(|| format!("Proveedor desconocido: {provider}"))?;
    state.secrets.set(&provider, key).map_err(|e| format!("No se pudo guardar en el llavero: {e}"))?;
    log::info!("API key de {provider} guardada en el llavero");
    Ok(())
}

#[tauri::command]
pub fn delete_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    state.secrets.delete(&provider).map_err(|e| format!("No se pudo eliminar del llavero: {e}"))?;
    log::info!("API key de {provider} eliminada del llavero");
    Ok(())
}

#[tauri::command]
pub fn get_permissions() -> PermissionsStatus {
    platform::permissions_status()
}

#[tauri::command]
pub fn request_microphone_permission(app: AppHandle) {
    platform::request_microphone_permission(Box::new(move |granted| {
        log::info!("Permiso de micrófono {}", if granted { "concedido" } else { "denegado" });
        let _ = app.emit("permissions-changed", granted);
    }));
}

#[tauri::command]
pub fn request_accessibility_permission() -> bool {
    platform::request_accessibility_permission()
}

#[tauri::command]
pub fn open_permission_settings(kind: PermissionKind) -> Result<(), String> {
    platform::open_permission_settings(kind).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<HistoryEntry> {
    lock(&state.history).entries()
}

#[tauri::command]
pub fn delete_history_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    lock(&state.history).remove(&id).map_err(|e| e.to_string())?;
    let _ = app.emit("history-changed", ());
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    lock(&state.history).clear().map_err(|e| e.to_string())?;
    let _ = app.emit("history-changed", ());
    Ok(())
}

#[tauri::command]
pub fn copy_history_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let text = lock(&state.history)
        .get(&id)
        .map(|e| e.text.clone())
        .ok_or_else(|| "La entrada ya no existe".to_string())?;
    paste::copy_text(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_input_devices() -> Vec<String> {
    audio::list_input_devices()
}

#[tauri::command]
pub fn validate_hotkey(hotkey: String) -> Result<String, String> {
    hotkey::validate(&hotkey)
}

#[tauri::command]
pub fn begin_hotkey_capture(app: AppHandle, state: State<'_, AppState>) {
    state.hotkey_suspended.store(true, Ordering::SeqCst);
    hotkey::suspend(&app);
}

#[tauri::command]
pub fn end_hotkey_capture(app: AppHandle, state: State<'_, AppState>) {
    state.hotkey_suspended.store(false, Ordering::SeqCst);
    hotkey::apply_from_settings(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub default_hotkey: String,
    pub log_dir: String,
    pub config_dir: String,
    /// Idiomas de interfaz disponibles (códigos ISO-639-1).
    pub ui_languages: Vec<String>,
    /// Idioma realmente en uso, ya resuelto si la preferencia es "auto".
    pub resolved_ui_language: String,
    /// Instrucciones de limpieza predeterminadas, para mostrarlas y poder restablecerlas.
    pub default_cleanup_prompt: String,
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        default_hotkey: DEFAULT_HOTKEY.into(),
        log_dir: state.log_dir.display().to_string(),
        config_dir: state.config_dir.display().to_string(),
        ui_languages: crate::i18n::LANGS.iter().map(|s| s.to_string()).collect(),
        resolved_ui_language: state.settings().ui_lang(),
        default_cleanup_prompt: crate::cleanup::DEFAULT_PROMPT.to_string(),
    }
}

#[tauri::command]
pub fn open_log_dir(state: State<'_, AppState>) -> Result<(), String> {
    tauri_plugin_opener::open_path(&state.log_dir, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn retry_last_transcription(app: AppHandle) {
    tauri::async_runtime::spawn(async move { pipeline::retry_last(&app).await });
}

// ---------- Licencia Pro ----------

#[tauri::command]
pub async fn get_license_status(app: AppHandle) -> crate::license::LicenseStatus {
    let secrets = app.state::<AppState>().secrets.clone();
    crate::license::validate(secrets).await
}

#[tauri::command]
pub async fn activate_license(app: AppHandle, key: String) -> Result<crate::license::LicenseStatus, String> {
    let secrets = app.state::<AppState>().secrets.clone();
    // El nombre identifica este equipo en el panel de licencias del proveedor.
    let status = crate::license::activate(secrets, &key, &device_label()).await?;
    let _ = app.emit("license-changed", &status);
    Ok(status)
}

#[tauri::command]
pub async fn deactivate_license(app: AppHandle) -> Result<(), String> {
    let secrets = app.state::<AppState>().secrets.clone();
    crate::license::deactivate(secrets).await?;
    let _ = app.emit("license-changed", crate::license::LicenseStatus::default());
    Ok(())
}

#[tauri::command]
pub fn open_checkout() -> Result<(), String> {
    tauri_plugin_opener::open_url(crate::license::CHECKOUT_URL, None::<&str>).map_err(|e| e.to_string())
}

/// Nombre legible de este equipo, para distinguir activaciones en el panel de licencias.
fn device_label() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH))
}

// ---------- Archivos de audio ----------

#[tauri::command]
pub fn transcribe_files(app: AppHandle, paths: Vec<String>) {
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(std::path::PathBuf::from).filter(|p| p.is_file()).collect();
    if !paths.is_empty() {
        crate::file_transcription::enqueue(&app, paths);
    }
}

/// Abre el selector de archivos del sistema y encola lo elegido.
#[tauri::command]
pub fn pick_audio_files(app: AppHandle) {
    use tauri_plugin_dialog::DialogExt;
    let handle = app.clone();
    app.dialog()
        .file()
        .add_filter("Audio", &crate::file_transcription::PICKER_EXTENSIONS)
        .pick_files(move |picked| {
            let paths: Vec<std::path::PathBuf> =
                picked.unwrap_or_default().into_iter().filter_map(|p| p.into_path().ok()).collect();
            if !paths.is_empty() {
                crate::file_transcription::enqueue(&handle, paths);
            }
        });
}

#[tauri::command]
pub fn get_file_jobs(state: State<'_, AppState>) -> Vec<crate::file_transcription::FileJob> {
    lock(&state.file_jobs).clone()
}

#[tauri::command]
pub fn remove_file_job(app: AppHandle, id: String) {
    crate::file_transcription::remove(&app, &id);
}

#[tauri::command]
pub fn clear_file_jobs(app: AppHandle) {
    crate::file_transcription::clear(&app);
}

#[tauri::command]
pub fn copy_file_transcript(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let text = lock(&state.file_jobs).iter().find(|j| j.id == id).map(|j| j.text.clone()).ok_or("")?;
    paste::copy_text(&text).map_err(|e| e.to_string())
}

/// Guarda la transcripción como .txt donde el usuario elija.
#[tauri::command]
pub fn save_file_transcript(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    let (name, text) = lock(&state.file_jobs)
        .iter()
        .find(|j| j.id == id)
        .map(|j| (j.name.clone(), j.text.clone()))
        .ok_or("")?;
    let suggested = format!("{}.txt", std::path::Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(name));
    app.dialog().file().set_file_name(suggested).add_filter("Texto", &["txt"]).save_file(move |target| {
        if let Some(path) = target.and_then(|p| p.into_path().ok()) {
            if let Err(e) = std::fs::write(&path, text) {
                log::warn!("No se pudo guardar la transcripción en {}: {e}", path.display());
            }
        }
    });
    Ok(())
}

/// El indicador flotante informa el tamaño de su contenido para ajustar la ventana.
#[tauri::command]
pub fn overlay_layout(app: AppHandle, width: f64, height: f64) {
    app_windows::layout_overlay(&app, width, height);
}

/// La interfaz avisa cuando terminó de cargar (diagnóstico: confirma que el webview está vivo).
#[tauri::command]
pub fn ui_ready(window: tauri::WebviewWindow) {
    log::info!("Interfaz lista: ventana «{}»", window.label());
}

/// Abre un enlace externo (solo https, para no ejecutar esquemas arbitrarios desde la UI).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Solo se permiten enlaces https".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}
