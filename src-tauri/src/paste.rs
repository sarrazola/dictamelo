//! Inserta el texto donde está el cursor conservando el portapapeles del usuario.

use crate::clipboard::{self, ClipboardError};
use crate::platform::{self, PlatformError};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PasteError {
    #[error("{0}")]
    Clipboard(#[from] ClipboardError),
    #[error("{0}")]
    Keystroke(#[from] PlatformError),
}

#[derive(Debug, Clone, Copy)]
pub struct PasteOutcome {
    pub clipboard_restored: bool,
}

/// Tiempo para que el sistema propague el nuevo contenido antes de enviar ⌘V.
const SETTLE_BEFORE_KEYSTROKE: Duration = Duration::from_millis(60);
/// Tiempo para que la app destino lea el portapapeles antes de restaurarlo.
const WAIT_AFTER_KEYSTROKE: Duration = Duration::from_millis(450);

/// Copia `text`, envía el atajo de pegar y restaura el portapapeles anterior si nadie lo
/// modificó entre tanto. Si el pegado falla, el texto queda en el portapapeles.
pub async fn paste_text(text: &str, restore_clipboard: bool) -> Result<PasteOutcome, PasteError> {
    let cb = clipboard::backend();
    let snapshot = if restore_clipboard { Some(cb.snapshot()?) } else { None };
    let our_change = cb.write_text(text)?;
    tokio::time::sleep(SETTLE_BEFORE_KEYSTROKE).await;

    tokio::task::spawn_blocking(platform::send_paste_keystroke)
        .await
        .map_err(|e| PlatformError::Other(e.to_string()))??;

    tokio::time::sleep(WAIT_AFTER_KEYSTROKE).await;
    let mut clipboard_restored = false;
    if let Some(snapshot) = snapshot {
        if cb.change_count()? == our_change {
            cb.restore(&snapshot)?;
            clipboard_restored = true;
        } else {
            log::info!("El portapapeles cambió durante el pegado; se conserva el contenido nuevo");
        }
    }
    Ok(PasteOutcome { clipboard_restored })
}

pub fn copy_text(text: &str) -> Result<(), ClipboardError> {
    clipboard::backend().write_text(text).map(|_| ())
}
