//! Configuración persistente del usuario (JSON en el directorio de configuración de la app).

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_HOTKEY: &str = "Alt+Shift+Space";
pub const DEFAULT_PROVIDER: &str = "groq";
pub const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Atajo global en el formato del plugin (p. ej. "Alt+Shift+Space").
    pub hotkey: String,
    /// Identificador del proveedor de transcripción ("groq", "openai", ...).
    pub provider: String,
    /// Use a personal provider key instead of the signed-in free allowance.
    pub use_own_key: bool,
    /// Identificador del modelo dentro del proveedor.
    pub model: String,
    /// Código ISO-639-1 del idioma del dictado, o "auto" para detección automática.
    pub language: String,
    /// Idioma de la interfaz y de los mensajes ("auto" = el del sistema).
    pub ui_language: String,
    /// Pegar automáticamente donde estaba el cursor. Si es `false`, solo se copia al portapapeles.
    pub auto_paste: bool,
    /// Restaurar el contenido anterior del portapapeles después de pegar.
    pub restore_clipboard: bool,
    /// Mostrar el indicador flotante de estado.
    pub show_overlay: bool,
    /// Nombre del dispositivo de entrada; `None` = micrófono predeterminado del sistema.
    pub input_device: Option<String>,
    /// Cantidad máxima de entradas guardadas en el historial.
    pub max_history: usize,
    /// Duración máxima de una grabación, en segundos.
    pub max_recording_secs: u32,
    /// Abrir Dictámelo al iniciar sesión.
    pub launch_at_login: bool,
    /// Sonido sutil al empezar y terminar de grabar.
    pub play_sounds: bool,
    /// Nombres y términos que el modelo suele confundir; se pasan como contexto a Whisper.
    pub vocabulary: String,
    /// Limpiar el texto con un modelo de lenguaje (muletillas, puntuación, autocorrecciones).
    pub cleanup_enabled: bool,
    /// Proveedor del modelo de limpieza (por ahora solo "groq").
    pub cleanup_provider: String,
    /// Modelo de limpieza dentro del proveedor.
    pub cleanup_model: String,
    /// Instrucciones de limpieza personalizadas; vacío = las predeterminadas.
    pub cleanup_prompt: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            provider: DEFAULT_PROVIDER.to_string(),
            use_own_key: false,
            model: DEFAULT_MODEL.to_string(),
            language: "auto".to_string(),
            ui_language: "auto".to_string(),
            auto_paste: true,
            restore_clipboard: true,
            show_overlay: true,
            input_device: None,
            max_history: 50,
            max_recording_secs: 300,
            launch_at_login: false,
            play_sounds: true,
            vocabulary: String::new(),
            cleanup_enabled: false,
            cleanup_provider: "groq".to_string(),
            cleanup_model: "openai/gpt-oss-120b".to_string(),
            cleanup_prompt: String::new(),
        }
    }
}

impl Settings {
    /// Carga la configuración; ante cualquier problema devuelve los valores por defecto.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str::<Settings>(&contents) {
                Ok(settings) => settings.sanitized(),
                Err(e) => {
                    log::warn!("settings.json inválido ({e}); se usan valores por defecto");
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Vocabulario listo para enviar como contexto (`None` si está vacío).
    pub fn vocabulary_prompt(&self) -> Option<String> {
        let v = self.vocabulary.trim();
        (!v.is_empty()).then(|| v.to_string())
    }

    /// Instrucciones de limpieza en uso (las del usuario o las predeterminadas), más el vocabulario.
    pub fn cleanup_system_prompt(&self) -> String {
        crate::cleanup::build_system_prompt(&self.cleanup_prompt, &self.vocabulary)
    }

    /// Idioma resuelto de la interfaz (nunca "auto").
    pub fn ui_lang(&self) -> String {
        crate::i18n::resolve(&self.ui_language)
    }

    /// Idioma para la API (`None` = detección automática).
    pub fn language_code(&self) -> Option<String> {
        let lang = self.language.trim();
        if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(lang.to_lowercase())
        }
    }

    /// Normaliza valores fuera de rango o vacíos.
    pub fn sanitized(mut self) -> Self {
        self.max_history = self.max_history.clamp(1, 500);
        self.max_recording_secs = self.max_recording_secs.clamp(5, 900);
        if self.hotkey.trim().is_empty() {
            self.hotkey = DEFAULT_HOTKEY.to_string();
        }
        if self.provider.trim().is_empty() {
            self.provider = DEFAULT_PROVIDER.to_string();
        }
        if self.model.trim().is_empty() {
            self.model = DEFAULT_MODEL.to_string();
        }
        if self.language.trim().is_empty() {
            self.language = "auto".to_string();
        }
        if self.ui_language.trim().is_empty() {
            self.ui_language = "auto".to_string();
        }
        if matches!(self.input_device.as_deref(), Some("")) {
            self.input_device = None;
        }
        if self.cleanup_provider.trim().is_empty() {
            self.cleanup_provider = "groq".to_string();
        }
        if self.cleanup_model.trim().is_empty() {
            self.cleanup_model = "openai/gpt-oss-120b".to_string();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json() {
        let dir = std::env::temp_dir().join(format!("dictado-settings-{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");
        let s = Settings { hotkey: "Control+Shift+F13".into(), language: "es".into(), auto_paste: false, ..Settings::default() };
        s.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded, s);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("dictado-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ esto no es json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn language_code_handles_auto() {
        let mut s = Settings::default();
        assert_eq!(s.language_code(), None);
        s.language = "ES".into();
        assert_eq!(s.language_code(), Some("es".into()));
    }

    #[test]
    fn sanitized_clamps_values() {
        let s = Settings { max_history: 0, max_recording_secs: 99_999, input_device: Some(String::new()), ..Default::default() }.sanitized();
        assert_eq!(s.max_history, 1);
        assert_eq!(s.max_recording_secs, 900);
        assert_eq!(s.input_device, None);
    }
}
