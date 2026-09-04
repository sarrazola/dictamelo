//! Escritura de WAV temporales (PCM 16 bits, mono).

use std::path::{Path, PathBuf};

pub fn write_wav_mono_i16(path: &Path, samples: &[i16], sample_rate: u32) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    {
        let mut w = writer.get_i16_writer(samples.len() as u32);
        for &s in samples {
            w.write_sample(s);
        }
        w.flush()?;
    }
    writer.finalize()
}

/// Ruta única para un WAV temporal dentro de `dir`.
pub fn new_temp_path(dir: &Path) -> PathBuf {
    dir.join(format!("dictado-{}.wav", uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_readable_wav() {
        let dir = std::env::temp_dir();
        let path = new_temp_path(&dir);
        let samples: Vec<i16> = (0..1600).map(|i| ((i % 100) as i16 - 50) * 300).collect();
        write_wav_mono_i16(&path, &samples, 16_000).unwrap();

        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 16_000);
        let read: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(read, samples);
        std::fs::remove_file(path).unwrap();
    }
}
