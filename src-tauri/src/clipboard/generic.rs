//! Backend genérico (Windows/Linux) basado en `arboard`. Solo conserva texto plano y
//! emula el contador de cambios con un hash del contenido. Pendiente de probar en Windows;
//! en Windows lo ideal es usar `GetClipboardSequenceNumber` y conservar todos los formatos.

use super::{ClipboardBackend, ClipboardError, ClipboardItem, ClipboardSnapshot};
use std::hash::{Hash, Hasher};

const TEXT_TYPE: &str = "text/plain";

pub struct ArboardClipboard;

impl ArboardClipboard {
    fn open() -> Result<arboard::Clipboard, ClipboardError> {
        arboard::Clipboard::new().map_err(|e| ClipboardError(e.to_string()))
    }

    fn fingerprint(text: Option<&str>) -> i64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish() as i64
    }
}

impl ClipboardBackend for ArboardClipboard {
    fn change_count(&self) -> Result<i64, ClipboardError> {
        Ok(Self::fingerprint(self.read_text()?.as_deref()))
    }

    fn snapshot(&self) -> Result<ClipboardSnapshot, ClipboardError> {
        let text = self.read_text()?;
        let items = text
            .as_ref()
            .map(|t| vec![ClipboardItem { representations: vec![(TEXT_TYPE.into(), t.as_bytes().to_vec())] }])
            .unwrap_or_default();
        Ok(ClipboardSnapshot { items, change_count: Self::fingerprint(text.as_deref()) })
    }

    fn write_text(&self, text: &str) -> Result<i64, ClipboardError> {
        Self::open()?.set_text(text).map_err(|e| ClipboardError(e.to_string()))?;
        Ok(Self::fingerprint(Some(text)))
    }

    fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        match Self::open()?.get_text() {
            Ok(t) => Ok(Some(t)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(ClipboardError(e.to_string())),
        }
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), ClipboardError> {
        let mut cb = Self::open()?;
        let text = snapshot
            .items
            .iter()
            .flat_map(|i| i.representations.iter())
            .find(|(t, _)| t == TEXT_TYPE)
            .and_then(|(_, bytes)| String::from_utf8(bytes.clone()).ok());
        match text {
            Some(t) => cb.set_text(t).map_err(|e| ClipboardError(e.to_string())),
            None => cb.clear().map_err(|e| ClipboardError(e.to_string())),
        }
    }
}
