//! Offline speech detection for microphone dictation on macOS.
//!
//! ASR models can invent text from room noise. Check the audio, not a blacklist of
//! words: a real spoken "thanks" must still be transcribed. Detection does not trim
//! or normalize the submitted recording, preserving quiet words and word boundaries.

use super::{PreparedAudio, TARGET_SAMPLE_RATE};

const FRAME_SAMPLES: usize = 256; // Earshot requires 16 ms at 16 kHz.
const MIN_VOICE_FRAMES: usize = 6; // 96 ms of voice in a short, coherent sequence.
const MAX_GAP_FRAMES: usize = 2;

pub fn contains_speech(audio: &PreparedAudio) -> bool {
    // Preparation owns resampling. Fail open for unexpected input rather than
    // silently rejecting speech if a future caller supplies another sample rate.
    if audio.sample_rate != TARGET_SAMPLE_RATE { return true; }
    let peak = audio.samples.iter().map(|sample| i32::from(*sample).abs()).max().unwrap_or(0);
    if peak == 0 { return false; }
    // Low-gain microphones must not lose quiet speech. Amplify only the detector's
    // scratch frame, never the recording sent to ASR; cap amplification at 40 dB.
    let gain = (16_384.0 / peak as f32).clamp(1.0, 100.0);
    let mut detector = earshot::Detector::default();
    let mut voice_frames = 0;
    let mut gap_frames = 0;
    let mut confident_run = 0;
    let mut confident_voice = false;
    for chunk in audio.samples.chunks(FRAME_SAMPLES) {
        let mut frame = [0i16; FRAME_SAMPLES];
        for (output, sample) in frame.iter_mut().zip(chunk) {
            *output = (f32::from(*sample) * gain).round() as i16;
        }
        // Keep a fresh detector for each recording. Do not accumulate unrelated
        // isolated clicks across a long recording into a false speech decision.
        // Isolated confident frames are not enough: clicks and notification tones
        // can look briefly like speech. Require 64 ms of confident voice within
        // a coherent sequence containing at least 96 ms of probable voice.
        let score = detector.predict_i16(&frame);
        confident_run = if score >= 0.8 { confident_run + 1 } else { 0 };
        confident_voice |= confident_run >= 4;
        if score >= 0.7 {
            voice_frames += 1;
            gap_frames = 0;
            if voice_frames >= MIN_VOICE_FRAMES && confident_voice { return true; }
        } else {
            gap_frames += 1;
            if gap_frames > MAX_GAP_FRAMES {
                voice_frames = 0;
                confident_voice = false;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(samples: Vec<i16>) -> PreparedAudio {
        PreparedAudio { samples, sample_rate: TARGET_SAMPLE_RATE }
    }

    fn fixture() -> Vec<i16> {
        hound::WavReader::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/english-speech.wav"))
            .unwrap().samples::<i16>().map(Result::unwrap).collect()
    }

    #[test]
    fn rejects_silence_and_low_microphone_noise_at_short_and_long_durations() {
        for count in [0, 100, 5600, 8000, 16_000, 16_000 * 8] {
            assert!(!contains_speech(&audio(vec![0; count])));
            let mut seed = 42u32;
            let noise = (0..count).map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as i16 - 128
            }).collect();
            assert!(!contains_speech(&audio(noise)), "noise length {count}");
            assert!(!contains_speech(&audio(vec![10; count])), "DC offset");
        }
    }

    #[test]
    fn rejects_isolated_key_clicks_without_a_word_blacklist() {
        let mut samples = vec![0; 32_000];
        for offset in [1000, 8000, 20_000] {
            samples[offset] = 20_000;
            samples[offset + 1] = -20_000;
        }
        assert!(!contains_speech(&audio(samples)));
    }

    #[test]
    fn preserves_real_speech_even_when_quiet_or_surrounded_by_silence() {
        let speech = fixture();
        for divisor in [1, 10, 100] {
            let quiet: Vec<_> = speech.iter().map(|sample| sample / divisor).collect();
            assert!(contains_speech(&audio(quiet.clone())), "gain 1/{divisor}");
            let mut padded = vec![0; 16_000 * 3];
            padded.extend(quiet);
            padded.extend(vec![0; 16_000 * 3]);
            assert!(contains_speech(&audio(padded)), "padded gain 1/{divisor}");
        }
        // Half a second of speech must not need a whole sentence to pass.
        assert!(contains_speech(&audio(speech[8000..16000].to_vec())));
    }

    #[test]
    fn each_recording_has_independent_detector_state() {
        assert!(contains_speech(&audio(fixture())));
        assert!(!contains_speech(&audio(vec![0; 16_000])));
    }

    #[test]
    fn recording_notification_sounds_are_not_speech() {
        // Read the OS assets in place; do not redistribute Apple's sound files.
        for name in ["Pop", "Tink"] {
            let path = std::path::PathBuf::from(format!("/System/Library/Sounds/{name}.aiff"));
            let temporary = super::super::wav::TempAudio::new(&std::env::temp_dir());
            crate::platform::decode_audio_to_wav(&path, temporary.path()).unwrap();
            let sound: Vec<_> = hound::WavReader::open(temporary.path()).unwrap()
                .samples::<i16>().map(Result::unwrap).collect();
            for divisor in [1, 3, 10, 100] {
                let mut samples = vec![0; 1600];
                samples.extend(sound.iter().map(|sample| sample / divisor));
                samples.extend(vec![0; 16_000]);
                assert!(!contains_speech(&audio(samples)), "{name} at gain 1/{divisor}");
            }
        }
    }
}
