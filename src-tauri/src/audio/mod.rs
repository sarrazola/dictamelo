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

/// Divide una grabación larga en tramos de como mucho `max_secs`, cortando en el punto más
/// silencioso de los 5 s anteriores a cada frontera para no partir palabras.
pub fn split_for_upload(samples: &[i16], sample_rate: u32, max_secs: u32) -> Vec<std::ops::Range<usize>> {
    let max_len = (sample_rate * max_secs) as usize;
    if samples.len() <= max_len || max_len == 0 {
        return vec![0..samples.len()];
    }
    let search = (sample_rate * 5) as usize;
    let window = (sample_rate as usize * 3) / 10; // 300 ms
    let mut ranges = Vec::new();
    let mut start = 0;
    while samples.len() - start > max_len {
        let target = start + max_len;
        let lo = target.saturating_sub(search).max(start + window);
        let mut best = target;
        let mut best_energy = f64::MAX;
        let mut pos = lo;
        while pos + window <= target {
            let energy: f64 = samples[pos..pos + window].iter().map(|s| (*s as f64) * (*s as f64)).sum();
            // `<=`: entre varios mínimos iguales (p. ej. sin silencios) se prefiere el más tardío,
            // para aprovechar al máximo la longitud del tramo.
            if energy <= best_energy {
                best_energy = energy;
                best = pos + window / 2;
            }
            pos += window / 3;
        }
        ranges.push(start..best);
        start = best;
    }
    ranges.push(start..samples.len());
    ranges
}

#[cfg(test)]
mod split_tests {
    use super::split_for_upload;

    #[test]
    fn short_audio_is_one_chunk() {
        let samples = vec![1000i16; 16_000 * 30];
        assert_eq!(split_for_upload(&samples, 16_000, 600), vec![0..samples.len()]);
    }

    #[test]
    fn long_audio_cuts_at_silence_and_covers_everything() {
        let rate = 16_000u32;
        // 25 s de "voz" con un silencio de 1 s justo a los 8,5 s; tramos de 10 s.
        let mut samples = vec![8000i16; (rate * 25) as usize];
        let silence = (rate as f32 * 8.5) as usize..(rate as f32 * 9.5) as usize;
        for s in &mut samples[silence.clone()] {
            *s = 0;
        }
        let ranges = split_for_upload(&samples, rate, 10);
        assert_eq!(ranges.len(), 3, "{ranges:?}");
        assert!(silence.contains(&ranges[0].end), "el primer corte debería caer en el silencio: {:?}", ranges[0]);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges.last().unwrap().end, samples.len());
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert!(ranges.iter().all(|r| r.len() <= (rate * 10) as usize));
    }
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
