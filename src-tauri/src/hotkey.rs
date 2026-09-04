//! Registro del atajo global (mantener presionado = grabar) con `tauri-plugin-global-shortcut`.

use crate::i18n::tf;
use crate::pipeline;
use crate::settings::DEFAULT_HOTKEY;
use crate::state::AppState;
use crate::status::Status;
use std::str::FromStr;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Comprueba que el atajo sea válido y devuelve su forma normalizada.
pub fn validate(hotkey: &str) -> Result<String, String> {
    let shortcut = Shortcut::from_str(hotkey.trim()).map_err(|e| format!("Atajo inválido: {e}"))?;
    Ok(shortcut.into_string())
}

/// Sustituye el atajo registrado por `hotkey`.
pub fn apply(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().map_err(|e| e.to_string())?;
    let shortcut = Shortcut::from_str(hotkey.trim()).map_err(|e| format!("Atajo inválido: {e}"))?;
    shortcuts
        .on_shortcut(shortcut, |app, _shortcut, event| match event.state {
            ShortcutState::Pressed => pipeline::hotkey_pressed(app),
            ShortcutState::Released => pipeline::hotkey_released(app),
        })
        .map_err(|e| format!("No se pudo registrar el atajo «{hotkey}»: {e}"))?;
    log::info!("Atajo global registrado: {hotkey}");
    Ok(())
}

/// Registra el atajo guardado; si falla, intenta con el predeterminado.
pub fn apply_from_settings(app: &AppHandle) {
    let hotkey = app.state::<AppState>().settings().hotkey;
    if let Err(e) = apply(app, &hotkey) {
        log::error!("{e}");
        if hotkey != DEFAULT_HOTKEY {
            let lang = app.state::<AppState>().settings().ui_lang();
            match apply(app, DEFAULT_HOTKEY) {
                Ok(()) => pipeline::set_status(
                    app,
                    Status::Error {
                        message: tf(&lang, "err.hotkey_failed", &[("k", &hotkey), ("d", DEFAULT_HOTKEY)]),
                    },
                ),
                Err(e2) => pipeline::set_status(app, Status::Error { message: e2 }),
            };
        } else {
            pipeline::set_status(app, Status::Error { message: e });
        }
    }
}

/// Desregistra el atajo mientras la UI captura uno nuevo.
pub fn suspend(app: &AppHandle) {
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("No se pudo suspender el atajo: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_hotkeys() {
        assert_eq!(validate("Alt+Shift+Space").unwrap(), "shift+alt+Space");
        assert_eq!(validate("Control+Alt+Super+KeyD").unwrap(), "control+alt+super+KeyD");
        assert!(validate("F13").is_ok());
        assert!(validate("").is_err());
        assert!(validate("Alt+NoExiste").is_err());
    }
}
