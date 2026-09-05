//! Implementación Windows: `SendInput` (Ctrl+V), portapapeles Win32 con todos los formatos y
//! `GetClipboardSequenceNumber`, ventana flotante sin activación, sonidos con `PlaySound`, sondeo
//! de la tecla Esc, Media Foundation para convertir audio y el Administrador de credenciales
//! (vía `keyring`) para las API keys.

mod audio_decode;
mod clipboard;
mod keyboard;
mod locale;
mod permissions;
mod sound;
mod tray_icon;
mod window;

pub use audio_decode::decode_audio_to_wav;
pub use clipboard::clipboard_backend;
pub use keyboard::{install_cancel_key_monitor, press_hotkey_for_test, send_paste_keystroke};
pub use locale::system_language;
pub use permissions::{
    open_permission_settings, permissions_status, request_accessibility_permission,
    request_microphone_permission,
};
pub use sound::play_sound;
pub use tray_icon::tray_icon;
pub use window::{activate_app, configure_overlay_window, hide_window, refresh_window_shadow, show_window_without_focus};

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{RegGetValueW, HKEY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};

/// Texto como UTF-16 terminado en NUL, para las APIs «W».
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Valor de texto (REG_SZ) del registro; `None` si no existe o no es texto.
fn registry_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let subkey = wide(subkey);
    let value = wide(value);
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: búfer y tamaño coherentes; RegGetValueW garantiza el NUL final para REG_SZ.
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let chars = (size as usize / 2).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..chars]).trim_end_matches('\0').to_string())
}

/// Valor numérico (REG_DWORD) del registro; `None` si no existe o no es un DWORD.
fn registry_dword(root: HKEY, subkey: &str, value: &str) -> Option<u32> {
    let subkey = wide(subkey);
    let value = wide(value);
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `data` tiene exactamente `size` bytes.
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut data as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}
