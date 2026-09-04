//! Portapapeles nativo (NSPasteboard) con instantánea completa de todos los tipos.

use crate::clipboard::{ClipboardBackend, ClipboardError, ClipboardItem, ClipboardSnapshot};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

pub struct MacClipboard;

pub fn clipboard_backend() -> Box<dyn ClipboardBackend> {
    Box::new(MacClipboard)
}

impl ClipboardBackend for MacClipboard {
    fn change_count(&self) -> Result<i64, ClipboardError> {
        Ok(NSPasteboard::generalPasteboard().changeCount() as i64)
    }

    fn snapshot(&self) -> Result<ClipboardSnapshot, ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let change_count = pasteboard.changeCount() as i64;
        let mut items = Vec::new();
        if let Some(pb_items) = pasteboard.pasteboardItems() {
            for item in pb_items.iter() {
                let mut representations = Vec::new();
                for ty in item.types().iter() {
                    if let Some(data) = item.dataForType(&ty) {
                        representations.push((ty.to_string(), data.to_vec()));
                    }
                }
                items.push(ClipboardItem { representations });
            }
        }
        Ok(ClipboardSnapshot { items, change_count })
    }

    fn write_text(&self, text: &str) -> Result<i64, ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: `NSPasteboardTypeString` es una constante global de AppKit.
        let ok = unsafe { pasteboard.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) };
        if !ok {
            return Err(ClipboardError("no se pudo escribir en el portapapeles".into()));
        }
        Ok(pasteboard.changeCount() as i64)
    }

    fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        // SAFETY: ver `write_text`.
        Ok(unsafe { pasteboard.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string()))
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        if snapshot.items.is_empty() {
            return Ok(());
        }
        let mut objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = Vec::new();
        for item in &snapshot.items {
            let pb_item = NSPasteboardItem::new();
            for (ty, bytes) in &item.representations {
                let data = NSData::with_bytes(bytes);
                pb_item.setData_forType(&data, &NSString::from_str(ty));
            }
            objects.push(ProtocolObject::from_retained(pb_item));
        }
        let array = NSArray::from_retained_slice(&objects);
        if pasteboard.writeObjects(&array) {
            Ok(())
        } else {
            Err(ClipboardError("no se pudo restaurar el portapapeles".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modifica el portapapeles real del usuario (lo deja como estaba). Se ejecuta solo
    /// con `DICTAMELO_CLIPBOARD_TESTS=1`.
    #[test]
    fn snapshot_and_restore_roundtrip() {
        if std::env::var("DICTAMELO_CLIPBOARD_TESTS").is_err() {
            eprintln!("omitido: define DICTAMELO_CLIPBOARD_TESTS=1");
            return;
        }
        let cb = MacClipboard;
        let original = cb.snapshot().unwrap();

        let before = cb.write_text("contenido previo del usuario").unwrap();
        let snap = cb.snapshot().unwrap();
        assert_eq!(snap.change_count, before);
        assert!(snap.items.iter().any(|i| i.representations.iter().any(|(t, _)| t == "public.utf8-plain-text")));

        let ours = cb.write_text("texto dictado").unwrap();
        assert!(ours > before);
        assert_eq!(cb.read_text().unwrap().as_deref(), Some("texto dictado"));
        assert_eq!(cb.change_count().unwrap(), ours);

        cb.restore(&snap).unwrap();
        assert_eq!(cb.read_text().unwrap().as_deref(), Some("contenido previo del usuario"));

        cb.restore(&original).unwrap();
    }
}
