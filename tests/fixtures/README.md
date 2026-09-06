# English speech upload fixture

`english-speech.wav` contains **5.855 seconds of human English speech**, mono signed PCM16 at 16,000 Hz (93,680 frames; 187,404 bytes). The expected 17-word transcript is copied from the source corpus's transcript file, not generated for this project:

> MISTER QUILTER IS THE APOSTLE OF THE MIDDLE CLASSES AND WE ARE GLAD TO WELCOME HIS GOSPEL

## Provenance and license

- **Source:** [LibriSpeech ASR corpus, OpenSLR SLR12](https://www.openslr.org/12/), utterance `1272-128104-0000` in `dev-clean`.
- **Attribution:** LibriSpeech (c) 2014 by Vassil Panayotov.
- **License:** [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/). The archive's original notice is preserved as [LICENSE-LibriSpeech.txt](LICENSE-LibriSpeech.txt). This audio is distributed under CC BY 4.0, separately from the application's MIT license; it is not labeled CC0.
- **Retrieved:** September 5, 2026, directly from the [official archive](https://www.openslr.org/resources/12/dev-clean.tar.gz). Audio member: `LibriSpeech/dev-clean/1272/128104/1272-128104-0000.flac`; transcript member: `LibriSpeech/dev-clean/1272/128104/1272-128104.trans.txt`.
- **Modification:** decode the complete FLAC utterance to canonical PCM WAV at its original sample rate. No crop, gain change, concatenation or synthetic speech was added. The fixture is not an endorsement by the source creators.

[english-speech.json](english-speech.json) records the source FLAC, WAV and decoded-PCM SHA-256 digests, exact transcript, format and attribution. The WAV digest is `799f78ed4beb4de7ceae3a809262d4ce242394342ccd1d58cef7d49dbc2def46`.

To reproduce after extracting those source members, decode FLAC to raw PCM with FFmpeg, then write a canonical 44-byte WAV header with Python's `wave` module:

```sh
ffmpeg -v error -i 1272-128104-0000.flac -f s16le -acodec pcm_s16le -ac 1 -ar 16000 speech.pcm
python3 - <<'PY'
from pathlib import Path
import wave
with wave.open('english-speech.wav', 'wb') as output:
    output.setnchannels(1)
    output.setsampwidth(2)
    output.setframerate(16000)
    output.writeframes(Path('speech.pcm').read_bytes())
PY
```

## Regression checks

Run `python3 scripts/check-audio-fixture.py` from the repository root. It validates integrity, WAV structure, non-silence and attribution, then checks the transcript comparator's rejection behavior. It uses only the Python standard library: no API, microphone, account, network or OS credential access. Both release build scripts run this check and the **complete Rust test suite**, which includes the real native WAV upload decoder, before packaging/signing. Windows runs that suite for the requested `--target`, after toolchain setup; a cross-compiled target needs a compatible test runner rather than skipping execution. The Mac regression workflow also runs all native and backend unit tests without cloud accounts or provider credentials.

Native decoder tests can load this same WAV. Live cloud tests must be explicit opt-ins, use a dedicated synthetic test account and assess the raw ASR result with `scripts/audio_fixture.py`'s `assert_transcript`. The comparator allows up to 20% word error rate after case/punctuation and `Mr.`/`Mister` normalization. It rejects empty, unrelated, heavily truncated and repeated output instead of merely checking that a response is non-empty.

Offline checks do not prove live transcription or cleanup. Record the live endpoint, account isolation, word accounting and cleanup-receipt results separately when running the authorized cloud regression.

For the explicit live regression, run `python3 scripts/test-free-cleanup-live.py --live --project-ref iburiyhhfodndqgmsaot` with its required server-side test configuration. Setting `DICTAMELO_LIVE_REGRESSION=1` and `DICTAMELO_TEST_PROJECT_REF=iburiyhhfodndqgmsaot` also enables that step in either release build script. It must remain opt-in: it creates disposable test identities and makes real provider requests, whereas ordinary source/PR checks require no secrets. Never supply a production user's session as the test account.
