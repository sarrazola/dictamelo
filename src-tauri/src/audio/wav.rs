//! Escritura de WAV temporales (PCM 16 bits, mono).

use std::path::{Path, PathBuf};

/// Own only a uniquely named app-generated file, never an imported source file.
pub struct TempAudio(PathBuf);

impl TempAudio {
    pub fn new(dir: &Path) -> Self { Self(new_temp_path(dir)) }
    pub fn path(&self) -> &Path { &self.0 }
}

impl Drop for TempAudio {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Could not remove temporary audio: {error}");
            }
        }
    }
}

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
    dir.join(format!("dictamelo-{}.wav", uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_audio_is_removed_when_its_owner_leaves_an_error_branch() {
        let temporary = TempAudio::new(&std::env::temp_dir());
        let path = temporary.path().to_path_buf();
        std::fs::write(&path, b"partial WAV").unwrap();
        drop(temporary);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn temporary_output_outlives_a_cancelled_conversion_waiter() {
        let (path_tx, path_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let waiter = tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                let temporary = TempAudio::new(&std::env::temp_dir());
                std::fs::write(temporary.path(), b"partial WAV").unwrap();
                path_tx.send(temporary.path().to_path_buf()).unwrap();
                finish_rx.recv().unwrap();
                drop(temporary);
                let _ = done_tx.send(());
            }).await.unwrap();
        });
        let path = path_rx.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(path.exists(), "a running decoder must keep ownership of its file");
        finish_tx.send(()).unwrap();
        done_rx.await.unwrap();
        assert!(!path.exists());
    }

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
