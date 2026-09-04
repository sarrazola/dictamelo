//! Otros sistemas (Linux): misma API con funcionalidad mínima para que compile.

use super::{PermissionKind, PermissionState, PermissionsStatus, PlatformError};
use crate::clipboard::ClipboardBackend;
use tauri::WebviewWindow;

pub fn permissions_status() -> PermissionsStatus {
    PermissionsStatus { microphone: PermissionState::NotApplicable, accessibility: PermissionState::NotApplicable }
}
pub fn request_microphone_permission(on_result: Box<dyn FnOnce(bool) + Send + 'static>) {
    on_result(true);
}
pub fn request_accessibility_permission() -> bool {
    true
}
pub fn open_permission_settings(_kind: PermissionKind) -> Result<(), PlatformError> {
    Ok(())
}
pub fn send_paste_keystroke() -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported("pegado automático no implementado en esta plataforma".into()))
}
pub fn press_hotkey_for_test(_hotkey: &str, _hold: std::time::Duration) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported("autodiagnóstico con atajo no implementado en esta plataforma".into()))
}

pub fn clipboard_backend() -> Box<dyn ClipboardBackend> {
    Box::new(crate::clipboard::generic::ArboardClipboard)
}
pub fn configure_overlay_window(_window: &WebviewWindow) -> Result<(), PlatformError> {
    Ok(())
}
pub fn show_window_without_focus(window: &WebviewWindow) -> Result<(), PlatformError> {
    window.show().map_err(|e| PlatformError::Other(e.to_string()))
}
pub fn hide_window(window: &WebviewWindow) -> Result<(), PlatformError> {
    window.hide().map_err(|e| PlatformError::Other(e.to_string()))
}
pub fn activate_app() {}

/// Idioma preferido del sistema, leído de las variables de entorno.
pub fn system_language() -> String {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .ok()
        .and_then(|v| v.split('.').next().map(str::to_string))
        .filter(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_else(|| "en".to_string())
}
