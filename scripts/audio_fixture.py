"""Shared, offline validation and transcript matching for the licensed speech fixture."""
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import unicodedata
import wave


FIXTURE_DIRECTORY = Path(__file__).resolve().parents[1] / "tests" / "fixtures"


@dataclass(frozen=True)
class AudioFixture:
    path: Path
    transcript: str
    word_count: int
    sample_rate_hz: int
    frame_count: int
    duration_seconds: float
    sha256: str


def normalized_words(text: str) -> list[str]:
    """Ignore formatting and the conventional Mr./Mister spelling, not spoken content."""
    text = unicodedata.normalize("NFKC", text).casefold()
    text = re.sub(r"\bmr\.?\b", "mister", text)
    return re.findall(r"[a-z]+(?:'[a-z]+)?", text.replace("’", "'"))


def load_fixture(directory: Path = FIXTURE_DIRECTORY) -> AudioFixture:
    """Validate the committed WAV and provenance without network, devices or secrets."""
    directory = Path(directory)
    metadata = json.loads((directory / "english-speech.json").read_text(encoding="utf-8"))
    if metadata.get("schema_version") != 1 or metadata.get("filename") != "english-speech.wav":
        raise ValueError("Unsupported speech fixture metadata")
    if metadata.get("license") != "CC-BY-4.0" or not metadata.get("attribution"):
        raise ValueError("Speech fixture license attribution is missing")
    if not (directory / "LICENSE-LibriSpeech.txt").is_file():
        raise ValueError("Speech fixture license file is missing")
    path = directory / metadata["filename"]
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != metadata.get("sha256") or len(data) != metadata.get("size_bytes"):
        raise ValueError("Speech fixture checksum or byte count changed")
    # The committed file is canonical PCM: an exact RIFF/fmt/data layout without
    # optional chunks. This catches partial files before a permissive decoder does.
    if len(data) < 44 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError("Speech fixture is not a RIFF WAVE file")
    if data[12:16] != b"fmt " or data[36:40] != b"data":
        raise ValueError("Speech fixture is not canonical PCM WAV")
    riff_size, = struct.unpack_from("<I", data, 4)
    fmt_size, encoding, channels, rate, byte_rate, block_align, bits = struct.unpack_from(
        "<IHHIIHH", data, 16
    )
    payload_size, = struct.unpack_from("<I", data, 40)
    if (fmt_size, encoding, channels, rate, byte_rate, block_align, bits) != (
        16, 1, 1, 16000, 32000, 2, 16
    ):
        raise ValueError("Speech fixture must be mono 16 kHz signed PCM16")
    if riff_size != len(data) - 8 or payload_size != len(data) - 44 or payload_size % 2:
        raise ValueError("Speech fixture is truncated or has inconsistent WAV sizes")
    with wave.open(str(path), "rb") as audio:
        frames = audio.getnframes()
        pcm = audio.readframes(frames)
    if len(pcm) != payload_size or hashlib.sha256(pcm).hexdigest() != metadata.get("pcm_sha256"):
        raise ValueError("Speech fixture decoded samples differ from the provenance")
    duration = frames / rate
    if not 5 <= duration <= 12:
        raise ValueError("Speech fixture must contain five to twelve seconds of speech")
    for key, actual in (
        ("sample_rate_hz", rate), ("channels", channels), ("sample_width_bytes", bits // 8),
        ("frame_count", frames), ("duration_seconds", duration),
    ):
        if metadata.get(key) != actual:
            raise ValueError(f"Speech fixture metadata mismatch: {key}")
    samples = [sample for sample, in struct.iter_unpack("<h", pcm)]
    rms = math.sqrt(sum(sample * sample for sample in samples) / len(samples))
    if rms < 100 or max(abs(sample) for sample in samples) < 1000:
        raise ValueError("Speech fixture is silent or too quiet for a useful upload test")
    transcript = metadata.get("transcript", "")
    if metadata.get("language") != "en" or len(normalized_words(transcript)) != metadata.get("word_count"):
        raise ValueError("Speech fixture transcript metadata is inconsistent")
    if not transcript.strip():
        raise ValueError("Speech fixture needs its source transcript")
    return AudioFixture(path, transcript, metadata["word_count"], rate, frames, duration, digest)


def assert_transcript(actual: str, expected: str, max_word_error_rate: float = 0.20) -> dict:
    """Reject empty/unrelated/partial ASR output while tolerating punctuation and small errors.

    The error only reports metrics, never response text or credentials. This helper
    assesses the raw transcription, not optional rewriting by a cleanup model.
    """
    if not isinstance(actual, str) or len(actual) > 10000:
        raise AssertionError("Transcription is missing or unexpectedly large")
    reference = normalized_words(expected)
    hypothesis = normalized_words(actual)
    if not reference or not 0 <= max_word_error_rate <= 1:
        raise ValueError("Invalid transcript comparison configuration")
    if not hypothesis or len(hypothesis) > len(reference) * 3:
        raise AssertionError("Transcription is empty or has an unexpected word count")
    previous = list(range(len(hypothesis) + 1))
    for i, reference_word in enumerate(reference, 1):
        current = [i]
        for j, actual_word in enumerate(hypothesis, 1):
            current.append(min(
                previous[j] + 1,
                current[j - 1] + 1,
                previous[j - 1] + (reference_word != actual_word),
            ))
        previous = current
    edits = previous[-1]
    rate = edits / len(reference)
    result = {
        "expected_words": len(reference), "actual_words": len(hypothesis),
        "word_errors": edits, "word_error_rate": rate,
    }
    if rate > max_word_error_rate:
        raise AssertionError(f"Speech fixture transcription mismatch: WER={rate:.3f}")
    return result
