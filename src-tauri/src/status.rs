//! Estado visible de la app (barra de menú, indicador flotante y ventana de configuración).

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

    pub fn label(&self) -> String {
        match self {
            Status::Idle => "Listo".into(),
            Status::Recording => "Grabando…".into(),
            Status::Transcribing => "Transcribiendo…".into(),
            Status::Pasting => "Pegando…".into(),
            Status::Done { message } => message.clone(),
            Status::Error { message } => format!("Error: {message}"),
        }
    }
}
