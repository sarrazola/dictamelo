//! Estado visible de la app (barra de menú, indicador flotante y ventana de configuración).

use crate::i18n::t;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Idle,
    Recording,
    Transcribing,
    Pasting,
    /// Resultado exitoso transitorio (vuelve a `Idle` solo).
    Done { message: String },
    /// Error transitorio (vuelve a `Idle` solo).
    Error { message: String },
}

impl Status {
    /// `true` mientras hay una operación en curso (no se aceptan nuevas pulsaciones).
    pub fn is_busy(&self) -> bool {
        matches!(self, Status::Recording | Status::Transcribing | Status::Pasting)
    }

    /// Texto corto del estado en el idioma indicado. Los mensajes de `Done`/`Error`
    /// ya vienen traducidos desde el pipeline.
    pub fn label(&self, lang: &str) -> String {
        match self {
            Status::Idle => t(lang, "status.idle").into(),
            Status::Recording => t(lang, "status.recording").into(),
            Status::Transcribing => t(lang, "status.transcribing").into(),
            Status::Pasting => t(lang, "status.pasting").into(),
            Status::Done { message } => message.clone(),
            Status::Error { message } => format!("{}: {message}", t(lang, "status.error")),
        }
    }
}
