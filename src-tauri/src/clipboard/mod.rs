//! Abstracción del portapapeles con instantánea completa y detección de cambios.
//!
//! El flujo de pegado guarda una instantánea, escribe el texto, envía ⌘V y, si nadie más
//! tocó el portapapeles mientras tanto (`change_count` no cambió), restaura la instantánea.

#[cfg(not(target_os = "macos"))]
pub mod generic;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipboardSnapshot {
    pub items: Vec<ClipboardItem>,
    /// Contador de cambios del sistema en el momento de la instantánea.
    pub change_count: i64,
}

/// Un elemento del portapapeles con todas sus representaciones (tipo UTI/MIME → bytes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipboardItem {
    pub representations: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct ClipboardError(pub String);

#[allow(dead_code)] // `read_text` se usa en pruebas y en el backend genérico.
pub trait ClipboardBackend: Send + Sync {
    /// Contador que el sistema incrementa con cada escritura al portapapeles.
    fn change_count(&self) -> Result<i64, ClipboardError>;
    fn snapshot(&self) -> Result<ClipboardSnapshot, ClipboardError>;
    /// Escribe texto plano y devuelve el `change_count` resultante.
    fn write_text(&self, text: &str) -> Result<i64, ClipboardError>;
    fn read_text(&self) -> Result<Option<String>, ClipboardError>;
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), ClipboardError>;
}

pub fn backend() -> Box<dyn ClipboardBackend> {
    crate::platform::clipboard_backend()
}
