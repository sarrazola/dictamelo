//! Conversión de cualquier audio que entienda CoreAudio a WAV 16 kHz mono, con `afconvert`
//! (viene con macOS; no hace falta instalar nada).

use super::super::PlatformError;
use std::path::Path;
use std::process::Command;

pub fn decode_audio_to_wav(input: &Path, output: &Path) -> Result<(), PlatformError> {
    let result = Command::new("afconvert")
        .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
        .arg(input)
        .arg(output)
        .output()
        .map_err(|e| PlatformError::Other(format!("afconvert: {e}")))?;
    if result.status.success() && output.exists() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
    // 'typ?', 'fmt?' y 'dta?' son los códigos de CoreAudio para formato/datos no reconocidos.
    if ["typ?", "fmt?", "dta?", "ptyp"].iter().any(|code| stderr.contains(code)) {
        Err(PlatformError::Unsupported(stderr))
    } else {
        Err(PlatformError::Other(if stderr.is_empty() { "afconvert falló".into() } else { stderr }))
    }
}
