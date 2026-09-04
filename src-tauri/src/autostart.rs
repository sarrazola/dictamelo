//! Abrir Dictado al iniciar sesión (LaunchAgent en macOS, registro en Windows).

use crate::state::AppState;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

pub fn set_enabled(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| format!("No se pudo cambiar el inicio automático: {e}"))?;
    log::info!("Inicio con el sistema: {}", if enabled { "activado" } else { "desactivado" });
    Ok(())
}

/// Al arrancar, deja el registro del sistema como dice la configuración (p. ej. tras mover la app).
pub fn sync_with_settings(app: &AppHandle) {
    let wanted = app.state::<AppState>().settings().launch_at_login;
    let current = app.autolaunch().is_enabled().unwrap_or(false);
    if wanted != current {
        if let Err(e) = set_enabled(app, wanted) {
            log::warn!("{e}");
        }
    }
}
