//! Todo lo específico del sistema operativo pasa por este módulo.
//!
//! Cada plataforma expone la misma API (permisos, teclado, portapapeles, ventanas):
//! - `macos/`   implementación completa y probada.
//! - `windows/` esqueleto con la misma firma, pendiente de implementar y probar.
//! El resto de la app nunca usa `#[cfg(target_os)]` directamente.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Algunas variantes solo se construyen en ciertas plataformas.
pub enum PermissionState {
    Granted,
    Denied,
    NotDetermined,
    /// La plataforma no tiene este permiso (p. ej. Accesibilidad en Windows).
    NotApplicable,
}

impl PermissionState {
    pub fn is_ok(self) -> bool {
        matches!(self, PermissionState::Granted | PermissionState::NotApplicable)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsStatus {
    pub microphone: PermissionState,
    pub accessibility: PermissionState,
}

impl PermissionsStatus {
    pub fn all_granted(&self) -> bool {
        self.microphone.is_ok() && self.accessibility.is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    Microphone,
    Accessibility,
}

/// Sonidos cortos de aviso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundKind {
    Start,
    Stop,
    Error,
}

#[derive(Debug, Clone, thiserror::Error)]
#[allow(dead_code)] // Algunas variantes solo se construyen en ciertas plataformas.
pub enum PlatformError {
    #[error("Falta el permiso de Accesibilidad")]
    AccessibilityDenied,
    #[error("No disponible en este sistema: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use unsupported::*;
