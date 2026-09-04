//! Permisos de macOS (TCC): micrófono vía AVFoundation y Accesibilidad vía AXIsProcessTrusted.

use super::super::{PermissionKind, PermissionState, PermissionsStatus, PlatformError};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use std::sync::Mutex;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

pub fn permissions_status() -> PermissionsStatus {
    PermissionsStatus { microphone: microphone_state(), accessibility: accessibility_state() }
}

pub fn microphone_state() -> PermissionState {
    // SAFETY: llamada de solo lectura a AVFoundation; `AVMediaTypeAudio` es una constante global.
    let status = unsafe {
        match AVMediaTypeAudio {
            Some(media) => AVCaptureDevice::authorizationStatusForMediaType(media),
            None => return PermissionState::NotDetermined,
        }
    };
    match status {
        AVAuthorizationStatus::Authorized => PermissionState::Granted,
        AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted => PermissionState::Denied,
        _ => PermissionState::NotDetermined,
    }
}

pub fn accessibility_state() -> PermissionState {
    // SAFETY: función pura del sistema sin argumentos.
    if unsafe { AXIsProcessTrusted() } != 0 {
        PermissionState::Granted
    } else {
        PermissionState::Denied
    }
}

/// Muestra el diálogo del sistema para el micrófono (solo la primera vez) y avisa el resultado.
pub fn request_microphone_permission(on_result: Box<dyn FnOnce(bool) + Send + 'static>) {
    let callback = Mutex::new(Some(on_result));
    let block = block2::RcBlock::new(move |granted: Bool| {
        if let Some(cb) = callback.lock().unwrap_or_else(|e| e.into_inner()).take() {
            cb(granted.as_bool());
        }
    });
    // SAFETY: el bloque se copia/retiene del lado de Objective-C hasta que se invoca.
    unsafe {
        if let Some(media) = AVMediaTypeAudio {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &block);
        }
    }
}

/// Pide Accesibilidad: macOS muestra su diálogo que lleva a Ajustes del Sistema.
/// Devuelve `true` si ya estaba concedido.
pub fn request_accessibility_permission() -> bool {
    // SAFETY: construimos el diccionario {kAXTrustedCheckOptionPrompt: true} con tipos CF válidos.
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options: CFDictionary<CFString, CFBoolean> =
            CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

pub fn open_permission_settings(kind: PermissionKind) -> Result<(), PlatformError> {
    let url = match kind {
        PermissionKind::Microphone => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        PermissionKind::Accessibility => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    };
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| PlatformError::Other(format!("no se pudo abrir Ajustes del Sistema: {e}")))
}
