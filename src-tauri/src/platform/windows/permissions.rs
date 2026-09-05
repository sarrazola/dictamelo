//! Permisos en Windows: no hay permiso de Accesibilidad, y el micrófono se gobierna con los
//! interruptores de Configuración → Privacidad y seguridad → Micrófono (que se leen del registro).

use super::super::{PermissionKind, PermissionState, PermissionsStatus, PlatformError};
use super::registry_string;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

const CONSENT_STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

pub fn permissions_status() -> PermissionsStatus {
    PermissionsStatus { microphone: microphone_state(), accessibility: PermissionState::NotApplicable }
}

/// Interruptores del micrófono: el del dispositivo (HKLM), el del usuario y el de «apps de
/// escritorio» (las no empaquetadas, como esta). Si alguno está en «Deny», WASAPI entrega silencio
/// o falla al abrir el micrófono.
pub fn microphone_state() -> PermissionState {
    let denied = [
        registry_string(HKEY_LOCAL_MACHINE, CONSENT_STORE, "Value"),
        registry_string(HKEY_CURRENT_USER, CONSENT_STORE, "Value"),
        registry_string(HKEY_CURRENT_USER, &format!(r"{CONSENT_STORE}\NonPackaged"), "Value"),
    ]
    .iter()
    .any(|value| value.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("Deny")));
    if denied {
        PermissionState::Denied
    } else {
        PermissionState::Granted
    }
}

/// Windows no muestra ningún diálogo a las apps de escritorio: el estado depende solo de los
/// interruptores, así que se responde de inmediato.
pub fn request_microphone_permission(on_result: Box<dyn FnOnce(bool) + Send + 'static>) {
    on_result(microphone_state().is_ok());
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
