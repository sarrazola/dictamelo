//! Captura y preparación de audio: micrófono → mono 16 kHz PCM 16 bits → WAV temporal.

pub mod recorder;
pub mod resample;
pub mod wav;

pub use recorder::{RawRecording, Recorder};

use cpal::traits::{DeviceTrait, HostTrait};
use std::path::Path;

/// Tasa de muestreo que enviamos a los proveedores (óptima para Whisper).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Audio listo para enviar: mono, 16 kHz, PCM 16 bits.
#[derive(Debug, Clone)]
pub struct PreparedAudio {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}

impl PreparedAudio {
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }
}

/// Convierte una grabación cruda (cualquier tasa/canales) al formato de envío.
pub fn prepare(raw: &RawRecording) -> PreparedAudio {
    let mono = resample::to_mono(&raw.samples, raw.channels as usize);
    let resampled = if raw.sample_rate == TARGET_SAMPLE_RATE {
        mono
    } else {
        resample::resample(&mono, raw.sample_rate, TARGET_SAMPLE_RATE)
    };
    let samples = resampled
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
        .collect();
    PreparedAudio { samples, sample_rate: TARGET_SAMPLE_RATE }
}

/// Nombres de los dispositivos de entrada disponibles.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect(),
        Err(e) => {
            log::warn!("No se pudieron enumerar los dispositivos de entrada: {e}");
            Vec::new()
        }
    }
}

/// Borra WAV temporales que hayan quedado de sesiones anteriores (p. ej. tras un cierre abrupto).
pub fn cleanup_temp_dir(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "wav") && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    if removed > 0 {
        log::info!("Eliminados {removed} archivos de audio temporales antiguos");
    }
}
