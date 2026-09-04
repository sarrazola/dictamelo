//! Idioma preferido del sistema.

use objc2_foundation::NSLocale;

/// Código del idioma preferido del usuario (p. ej. "es", "en-US"). "en" si no se puede leer.
pub fn system_language() -> String {
    let languages = NSLocale::preferredLanguages();
    languages
        .iter()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "en".to_string())
}
