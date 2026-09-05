//! Sonidos de aviso con los sonidos del sistema (sin assets propios): los de dictado por voz de
//! Windows (`Speech On/Off`) y, si no existen, los alias del esquema de sonidos del usuario.

use super::super::SoundKind;
use super::wide;
use std::path::PathBuf;
use tauri::AppHandle;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_FILENAME, SND_NODEFAULT};

/// Reproduce un sonido corto del sistema sin bloquear (se toca en un hilo aparte).
pub fn play_sound(_app: &AppHandle, kind: SoundKind) {
    let (file, alias) = match kind {
        SoundKind::Start => ("Speech On.wav", "SystemAsterisk"),
        SoundKind::Stop => ("Speech Off.wav", "SystemAsterisk"),
        SoundKind::Error => ("Speech Misrecognition.wav", "SystemHand"),
    };
    let spawned = std::thread::Builder::new().name("dictamelo-sound".into()).spawn(move || {
        let media = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Media")
            .join(file);
        let played = media.is_file() && {
            let path = wide(&media.to_string_lossy());
            // SAFETY: cadena UTF-16 terminada en NUL; sin módulo de recursos.
            unsafe { PlaySoundW(PCWSTR(path.as_ptr()), None, SND_FILENAME | SND_NODEFAULT) }.as_bool()
        };
        if !played {
            let name = wide(alias);
            // SAFETY: ídem, con un alias del esquema de sonidos.
            if !unsafe { PlaySoundW(PCWSTR(name.as_ptr()), None, SND_ALIAS | SND_NODEFAULT) }.as_bool() {
                log::debug!("No se pudo reproducir el sonido «{file}» ni el alias «{alias}»");
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("No se pudo crear el hilo del sonido: {e}");
    }
}
