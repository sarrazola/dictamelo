//! Idioma preferido del usuario (el de la interfaz de Windows).

use windows::core::PWSTR;
use windows::Win32::Globalization::{GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};

/// Código del idioma preferido (p. ej. "es-CO", "en-US"). "en" si no se puede leer.
pub fn system_language() -> String {
    if let Some(lang) = preferred_ui_language() {
        return lang;
    }
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
    // SAFETY: búfer válido; devuelve la longitud incluido el NUL.
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len > 1 {
        return String::from_utf16_lossy(&buf[..len as usize - 1]);
    }
    "en".to_string()
}

/// Primer idioma de la lista de idiomas de visualización del usuario.
fn preferred_ui_language() -> Option<String> {
    let mut count = 0u32;
    let mut size = 0u32;
    // SAFETY: primera llamada sin búfer para conocer el tamaño; segunda con un búfer de ese tamaño.
    unsafe {
        GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut size).ok()?;
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u16; size as usize];
        GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, Some(PWSTR(buf.as_mut_ptr())), &mut size).ok()?;
        // Lista de cadenas terminadas en NUL, con un NUL extra al final.
        let first = buf.split(|&c| c == 0).next()?;
        let lang = String::from_utf16_lossy(first);
        (!lang.is_empty()).then_some(lang)
    }
}
