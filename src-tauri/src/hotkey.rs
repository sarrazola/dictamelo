//! Global shortcuts share hold/toggle controls; macOS also supports standalone Fn.

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
    #[cfg(target_os = "macos")]
    if hotkey.trim().eq_ignore_ascii_case("fn") { return Ok("Fn".into()); }
    let shortcut = Shortcut::from_str(hotkey.trim()).map_err(|e| format!("Atajo inválido: {e}"))?;
    Ok(shortcut.into_string())
}

/// Sustituye el atajo registrado por `hotkey`.
pub fn apply(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let normalized = validate(hotkey)?;
    let state = app.state::<AppState>();
    let mut registered = crate::util::lock(&state.registered_hotkey);
    let shortcuts = app.global_shortcut();
    #[cfg(target_os = "macos")]
    if normalized == "Fn" {
        let state = app.state::<AppState>();
        state.fn_hotkey.enable(app).map_err(|_| crate::i18n::t(&state.settings().ui_lang(), "err.fn_permission").to_string())?;
        if let Err(error) = shortcuts.unregister_all() {
            state.fn_hotkey.disable();
            return Err(error.to_string());
        }
        *registered = Some(normalized);
        pipeline::enqueue_control(app, crate::recording_control::Event::Reset);
        return Ok(());
    }
    let shortcut = Shortcut::from_str(&normalized).map_err(|e| e.to_string())?;
    // Keep the old shortcut alive if the replacement is already owned by another app.
    if !shortcuts.is_registered(shortcut) {
        shortcuts.on_shortcut(shortcut, |app, _shortcut, event| match event.state {
            ShortcutState::Pressed => pipeline::hotkey_pressed(app),
            ShortcutState::Released => pipeline::hotkey_released(app),
        }).map_err(|e| format!("No se pudo registrar el atajo «{hotkey}»: {e}"))?;
    }
    if let Some(old) = registered.as_deref() {
        if let Ok(previous) = Shortcut::from_str(old) {
            if previous != shortcut {
                if let Err(error) = shortcuts.unregister(previous) {
                    let _ = shortcuts.unregister(shortcut);
                    return Err(error.to_string());
                }
            }
        }
    }
    *registered = Some(normalized);
    #[cfg(target_os = "macos")]
    app.state::<AppState>().fn_hotkey.disable();
    pipeline::enqueue_control(app, crate::recording_control::Event::Reset);
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
    let state = app.state::<AppState>();
    let mut registered = crate::util::lock(&state.registered_hotkey);
    #[cfg(target_os = "macos")]
    app.state::<AppState>().fn_hotkey.disable();
    pipeline::enqueue_control(app, crate::recording_control::Event::Reset);
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("No se pudo suspender el atajo: {e}");
    } else { *registered = None; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn validates_standalone_fn() { assert_eq!(validate(" fn ").unwrap(), "Fn"); }

    #[test]
    fn validates_hotkeys() {
        assert_eq!(validate("Alt+Shift+Space").unwrap(), "shift+alt+Space");
        assert_eq!(validate("Control+Alt+Super+KeyD").unwrap(), "control+alt+super+KeyD");
        assert!(validate("F13").is_ok());
        assert!(validate("").is_err());
        assert!(validate("Alt+NoExiste").is_err());
    }
}
