//! Mezcla a mono y remuestreo sin dependencias externas.
//!
//! Para reducir la tasa se aplica primero un filtro FIR pasa-bajos (sinc enventanado con
//! Hamming) y después interpolación lineal. Es más que suficiente para voz.

use std::f32::consts::PI;

/// Promedia los canales intercalados en una sola señal.
pub fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Remuestrea `input` de `from` Hz a `to` Hz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() || from == 0 || to == 0 {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (input.len() as f64 / ratio).floor().max(1.0) as usize;

    // Al bajar la tasa filtramos por debajo de la nueva frecuencia de Nyquist para evitar aliasing.
    let taps = if to < from {
        Some(lowpass_taps(0.45 * to as f32 / from as f32))
    } else {
        None
    };
    let value_at = |i: usize| -> f32 {
        match &taps {
            Some(h) => fir_at(input, h, i),
            None => input[i],
        }
    };

    let mut out = Vec::with_capacity(out_len);
    let mut cache: (usize, f32) = (usize::MAX, 0.0);
    let cached = |i: usize, cache: &mut (usize, f32)| -> f32 {
        if cache.0 != i {
            *cache = (i, value_at(i));
        }
        cache.1
    };
    for n in 0..out_len {
        let pos = n as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = cached(idx, &mut cache);
        let b = if idx + 1 < input.len() { cached(idx + 1, &mut cache) } else { a };
        out.push(a + (b - a) * frac);
    }
    out
}

const TAPS: usize = 63;

/// Coeficientes de un FIR pasa-bajos; `cutoff` es la frecuencia de corte normalizada (0..0.5).
fn lowpass_taps(cutoff: f32) -> Vec<f32> {
    let m = (TAPS - 1) as f32 / 2.0;
    let mut h = vec![0f32; TAPS];
    let mut sum = 0f32;
    for (n, coef) in h.iter_mut().enumerate() {
        let x = n as f32 - m;
        let sinc = if x.abs() < 1e-6 { 2.0 * cutoff } else { (2.0 * PI * cutoff * x).sin() / (PI * x) };
        let window = 0.54 - 0.46 * (2.0 * PI * n as f32 / (TAPS - 1) as f32).cos();
        *coef = sinc * window;
        sum += *coef;
    }
    for coef in &mut h {
        *coef /= sum;
    }
    h
}

fn fir_at(input: &[f32], h: &[f32], i: usize) -> f32 {
    let half = (h.len() / 2) as isize;
    let mut acc = 0f32;
    for (k, &coef) in h.iter().enumerate() {
        let j = i as isize + k as isize - half;
        if j >= 0 && (j as usize) < input.len() {
            acc += coef * input[j as usize];
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: u32, secs: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| (2.0 * PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn mono_mixdown_averages_channels() {
        let stereo = [1.0, 0.0, 0.5, 0.5, -1.0, 1.0];
        assert_eq!(to_mono(&stereo, 2), vec![0.5, 0.5, 0.0]);
        assert_eq!(to_mono(&[0.3, 0.4], 1), vec![0.3, 0.4]);
    }

    #[test]
    fn downsampling_keeps_voice_band() {
        let input = sine(440.0, 48_000, 1.0);
        let out = resample(&input, 48_000, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1);
        // La amplitud de un tono de 440 Hz debe conservarse (RMS ≈ 0.707).
        let r = rms(&out[1000..15000]);
        assert!((r - 0.707).abs() < 0.03, "rms={r}");
    }

    #[test]
    fn downsampling_attenuates_aliasing_band() {
        // Un tono de 11 kHz no cabe en 16 kHz: debe quedar muy atenuado, no plegado.
        let input = sine(11_000.0, 48_000, 1.0);
        let out = resample(&input, 48_000, 16_000);
        let r = rms(&out[1000..15000]);
        assert!(r < 0.05, "rms={r}");
    }

    #[test]
    fn non_integer_ratio_and_upsampling() {
        let input = sine(300.0, 44_100, 0.5);
        let out = resample(&input, 44_100, 16_000);
        assert!((out.len() as i64 - 8_000).abs() <= 1);
        let up = resample(&sine(300.0, 8_000, 0.5), 8_000, 16_000);
        assert!((up.len() as i64 - 8_000).abs() <= 1);
        assert!((rms(&up[500..7500]) - 0.707).abs() < 0.03);
    }

    #[test]
    fn same_rate_is_identity() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(resample(&input, 16_000, 16_000), input);
        assert!(resample(&[], 48_000, 16_000).is_empty());
    }
}
