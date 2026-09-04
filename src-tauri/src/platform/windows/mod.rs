//! Esqueleto para Windows. Mantiene la misma API que `macos/` para que el resto de la app
//! compile sin cambios. PENDIENTE (no implementado ni probado en esta versión):
//! - `send_paste_keystroke`: `SendInput` con Ctrl+V.
//! - Portapapeles con todos los formatos y `GetClipboardSequenceNumber`.
//! - Ventana flotante sin foco (`SW_SHOWNOACTIVATE`, `WS_EX_NOACTIVATE`).

use super::{PermissionKind, PermissionState, PermissionsStatus, PlatformError};
use crate::clipboard::ClipboardBackend;
use tauri::WebviewWindow;

pub fn permissions_status() -> PermissionsStatus {
    // Windows no requiere permiso explícito de accesibilidad; el micrófono se gestiona en
    // Configuración → Privacidad, y cpal falla si está bloqueado.
    PermissionsStatus { microphone: PermissionState::NotApplicable, accessibility: PermissionState::NotApplicable }
}

pub fn request_microphone_permission(on_result: Box<dyn FnOnce(bool) + Send + 'static>) {
    on_result(true);
}

pub fn request_accessibility_permission() -> bool {
    true
}

pub fn open_permission_settings(kind: PermissionKind) -> Result<(), PlatformError> {
    let url = match kind {
        PermissionKind::Microphone => "ms-settings:privacy-microphone",
        PermissionKind::Accessibility => "ms-settings:privacy",
    };
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| PlatformError::Other(e.to_string()))
}

pub fn send_paste_keystroke() -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported("el pegado automático en Windows aún no está implementado".into()))
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
