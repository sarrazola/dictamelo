//! Portapapeles nativo (Win32) con instantánea de todos los formatos y detección de cambios
//! mediante `GetClipboardSequenceNumber`.

use super::wide;
use crate::clipboard::{ClipboardBackend, ClipboardError, ClipboardItem, ClipboardSnapshot};
use std::time::Duration;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW,
    GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

pub struct WindowsClipboard;

pub fn clipboard_backend() -> Box<dyn ClipboardBackend> {
    Box::new(WindowsClipboard)
}

const CF_UNICODETEXT: u32 = 13;
/// Formatos cuyo «dato» es un handle GDI o de metarchivo, no un bloque de memoria: no se pueden
/// copiar como bytes. Windows sintetiza CF_BITMAP a partir de CF_DIB, así que las imágenes no se pierden.
const CF_BITMAP: u32 = 2;
const CF_METAFILEPICT: u32 = 3;
const CF_ENHMETAFILE: u32 = 14;
const CF_OWNERDISPLAY: u32 = 0x80;
const CF_DSPBITMAP: u32 = 0x82;
const CF_DSPMETAFILEPICT: u32 = 0x83;
const CF_DSPENHMETAFILE: u32 = 0x8E;
const CF_PRIVATEFIRST: u32 = 0x200;
const CF_PRIVATELAST: u32 = 0x2FF;
const CF_GDIOBJFIRST: u32 = 0x300;
const CF_GDIOBJLAST: u32 = 0x3FF;
/// Desde aquí son formatos registrados por nombre (`RegisterClipboardFormat`); su número puede
/// cambiar entre sesiones, por eso la instantánea guarda el nombre.
const CF_REGISTERED_MIN: u32 = 0xC000;
/// Con este formato presente, el historial del portapapeles (Win+V) y la nube ignoran el
/// contenido: restaurar lo anterior no crea una entrada duplicada.
const EXCLUDE_FROM_HISTORY: &str = "ExcludeClipboardContentFromMonitorProcessing";

fn is_handle_format(format: u32) -> bool {
    matches!(
        format,
        CF_BITMAP | CF_METAFILEPICT | CF_ENHMETAFILE | CF_OWNERDISPLAY | CF_DSPBITMAP | CF_DSPMETAFILEPICT | CF_DSPENHMETAFILE
    ) || (CF_PRIVATEFIRST..=CF_PRIVATELAST).contains(&format)
        || (CF_GDIOBJFIRST..=CF_GDIOBJLAST).contains(&format)
}

/// Portapapeles abierto; se cierra al soltar. Otra app puede tenerlo abierto un instante, así que
/// se reintenta unas cuantas veces.
struct Opened;

impl Opened {
    fn new() -> Result<Opened, ClipboardError> {
        let mut last_error = None;
        for _ in 0..12 {
            // SAFETY: sin ventana propietaria; el cierre se garantiza en `Drop`.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Opened),
                Err(e) => last_error = Some(e),
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        Err(ClipboardError(format!(
            "no se pudo abrir el portapapeles: {}",
            last_error.map(|e| e.message()).unwrap_or_default().trim()
        )))
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        // SAFETY: emparejado con el `OpenClipboard` de `new`.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn format_name(format: u32) -> String {
    if format < CF_REGISTERED_MIN {
        return format!("cf:{format}");
    }
    let mut buf = [0u16; 256];
    // SAFETY: búfer válido; devuelve la cantidad de caracteres escritos.
    let len = unsafe { GetClipboardFormatNameW(format, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        format!("cf:{format}")
    }
}

fn format_id(name: &str) -> Option<u32> {
    if let Some(id) = name.strip_prefix("cf:") {
        return id.parse().ok();
    }
    let name = wide(name);
    // SAFETY: cadena UTF-16 terminada en NUL.
    let id = unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) };
    (id != 0).then_some(id)
}

/// Copia el contenido de un bloque HGLOBAL del portapapeles (sin tomar posesión de él).
fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let hglobal = HGLOBAL(handle.0);
    // SAFETY: el handle lo entregó GetClipboardData y el portapapeles sigue abierto.
    unsafe {
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            return None;
        }
        let size = GlobalSize(hglobal);
        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
        let _ = GlobalUnlock(hglobal);
        Some(bytes)
    }
}

/// Reserva un bloque HGLOBAL movible (como exige el portapapeles) con `bytes`.
fn write_global(bytes: &[u8]) -> Result<HGLOBAL, ClipboardError> {
    // SAFETY: se reserva al menos 1 byte y se copian exactamente `bytes.len()` bytes en el bloque.
    unsafe {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| ClipboardError(e.message()))?;
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = GlobalFree(Some(hglobal));
            return Err(ClipboardError("no se pudo bloquear la memoria del portapapeles".into()));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
        let _ = GlobalUnlock(hglobal);
        Ok(hglobal)
    }
}

/// Coloca `bytes` bajo `format` en el portapapeles (ya abierto); el sistema pasa a ser el dueño del bloque.
fn set_data(format: u32, bytes: &[u8]) -> Result<(), ClipboardError> {
    let hglobal = write_global(bytes)?;
    // SAFETY: el portapapeles lo abrió quien llama; si falla, el bloque sigue siendo nuestro y se libera.
    if let Err(e) = unsafe { SetClipboardData(format, Some(HANDLE(hglobal.0))) } {
        unsafe {
            let _ = GlobalFree(Some(hglobal));
        }
        return Err(ClipboardError(format!("no se pudo escribir el formato {format}: {}", e.message().trim())));
    }
    Ok(())
}

fn sequence_number() -> i64 {
    // SAFETY: función del sistema sin efectos secundarios.
    i64::from(unsafe { GetClipboardSequenceNumber() })
}

impl ClipboardBackend for WindowsClipboard {
    fn change_count(&self) -> Result<i64, ClipboardError> {
        Ok(sequence_number())
    }

    fn snapshot(&self) -> Result<ClipboardSnapshot, ClipboardError> {
        let _open = Opened::new()?;
        let change_count = sequence_number();
        let mut representations = Vec::new();
        let mut format = 0u32;
        loop {
            // SAFETY: portapapeles abierto; 0 termina la enumeración.
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            if is_handle_format(format) {
                continue;
            }
            // SAFETY: portapapeles abierto; el handle pertenece al sistema y solo se lee.
            let Ok(handle) = (unsafe { GetClipboardData(format) }) else { continue };
            if handle.0.is_null() {
                continue;
            }
            if let Some(bytes) = read_global(handle) {
                representations.push((format_name(format), bytes));
            }
        }
        let items = if representations.is_empty() { Vec::new() } else { vec![ClipboardItem { representations }] };
        Ok(ClipboardSnapshot { items, change_count })
    }

    fn write_text(&self, text: &str) -> Result<i64, ClipboardError> {
        let utf16 = wide(text);
        // SAFETY: el mismo búfer visto como bytes (u16 → 2 bytes, little-endian).
        let bytes = unsafe { std::slice::from_raw_parts(utf16.as_ptr() as *const u8, utf16.len() * 2) };
        {
            let _open = Opened::new()?;
            // SAFETY: portapapeles abierto.
            unsafe { EmptyClipboard() }.map_err(|e| ClipboardError(e.message()))?;
            set_data(CF_UNICODETEXT, bytes)?;
        }
        // Se lee con el portapapeles ya cerrado: es el número que verá cualquier cambio posterior.
        Ok(sequence_number())
    }

    fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        let _open = Opened::new()?;
        // SAFETY: portapapeles abierto; un error significa que no hay texto.
        let Ok(handle) = (unsafe { GetClipboardData(CF_UNICODETEXT) }) else { return Ok(None) };
        if handle.0.is_null() {
            return Ok(None);
        }
        let Some(bytes) = read_global(handle) else { return Ok(None) };
        let utf16: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|&c| c != 0)
            .collect();
        Ok(Some(String::from_utf16_lossy(&utf16)))
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), ClipboardError> {
        let _open = Opened::new()?;
        // SAFETY: portapapeles abierto.
        unsafe { EmptyClipboard() }.map_err(|e| ClipboardError(e.message()))?;
        let mut restored = 0;
        for item in &snapshot.items {
            for (name, bytes) in &item.representations {
                let Some(format) = format_id(name) else { continue };
                match set_data(format, bytes) {
                    Ok(()) => restored += 1,
                    Err(e) => log::warn!("No se pudo restaurar el formato «{name}»: {e}"),
                }
            }
        }
        if restored > 0 {
            if let Some(format) = format_id(EXCLUDE_FROM_HISTORY) {
                let _ = set_data(format, &0u32.to_le_bytes());
            }
        }
        Ok(())
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
        let cb = WindowsClipboard;
        let original = cb.snapshot().unwrap();

        let before = cb.write_text("contenido previo del usuario").unwrap();
        let snap = cb.snapshot().unwrap();
        assert_eq!(snap.change_count, before);
        assert!(snap.items.iter().any(|i| i.representations.iter().any(|(t, _)| t == "cf:13")));

        let ours = cb.write_text("texto dictado").unwrap();
        assert!(ours > before);
        assert_eq!(cb.read_text().unwrap().as_deref(), Some("texto dictado"));
        assert_eq!(cb.change_count().unwrap(), ours);

        cb.restore(&snap).unwrap();
        assert_eq!(cb.read_text().unwrap().as_deref(), Some("contenido previo del usuario"));
        assert!(cb.change_count().unwrap() > ours);

        cb.restore(&original).unwrap();
    }

    #[test]
    fn format_names_roundtrip() {
        assert_eq!(format_name(CF_UNICODETEXT), "cf:13");
        assert_eq!(format_id("cf:13"), Some(13));
        let id = format_id("Dictamelo.Test.Format").unwrap();
        assert!(id >= CF_REGISTERED_MIN);
        assert_eq!(format_name(id), "Dictamelo.Test.Format");
        assert!(is_handle_format(CF_BITMAP) && is_handle_format(0x310) && !is_handle_format(8));
    }
}
