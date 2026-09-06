"""Guard the real speech asset and the accuracy check used by the opt-in live test."""
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from audio_fixture import FIXTURE_DIRECTORY, assert_transcript, load_fixture


class AudioFixtureTests(unittest.TestCase):
    def changed_fixture(self, edit, update_hashes=False):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name)
        for name in ("english-speech.wav", "english-speech.json", "LICENSE-LibriSpeech.txt"):
            shutil.copyfile(FIXTURE_DIRECTORY / name, path / name)
        wav = path / "english-speech.wav"
        data = bytearray(wav.read_bytes())
        edit(data)
        wav.write_bytes(data)
        if update_hashes:
            metadata_path = path / "english-speech.json"
            metadata = json.loads(metadata_path.read_text())
            metadata.update(sha256=hashlib.sha256(data).hexdigest(), size_bytes=len(data),
                            pcm_sha256=hashlib.sha256(data[44:]).hexdigest())
            metadata_path.write_text(json.dumps(metadata))
        return path

    def test_committed_human_speech_is_small_valid_and_licensed(self):
        fixture = load_fixture()
        self.assertEqual((fixture.frame_count, fixture.sample_rate_hz), (93680, 16000))
        self.assertEqual(fixture.word_count, 17)
        self.assertEqual(fixture.duration_seconds, 5.855)
        self.assertLess(fixture.path.stat().st_size, 200000)

    def test_corruption_is_rejected_before_upload(self):
        path = self.changed_fixture(lambda data: data.__setitem__(100, data[100] ^ 1))
        with self.assertRaisesRegex(ValueError, "checksum"):
            load_fixture(path)

    def test_truncated_file_is_rejected_even_if_checksum_is_updated(self):
        path = self.changed_fixture(lambda data: data.__delitem__(slice(-20, None)), True)
        with self.assertRaisesRegex(ValueError, "truncated"):
            load_fixture(path)

    def test_wrong_sample_rate_is_rejected_even_if_checksum_is_updated(self):
        path = self.changed_fixture(lambda data: struct.pack_into("<I", data, 24, 8000), True)
        with self.assertRaisesRegex(ValueError, "16 kHz"):
            load_fixture(path)

    def test_silence_cannot_replace_the_speech_fixture(self):
        path = self.changed_fixture(lambda data: data.__setitem__(slice(44, None), bytes(len(data) - 44)), True)
        with self.assertRaisesRegex(ValueError, "silent"):
            load_fixture(path)

    def test_license_file_is_required_for_redistribution(self):
        path = self.changed_fixture(lambda data: None)
        (path / "LICENSE-LibriSpeech.txt").unlink()
        with self.assertRaisesRegex(ValueError, "license file"):
            load_fixture(path)

    def test_transcription_accepts_punctuation_case_and_mr_spelling(self):
        expected = load_fixture().transcript
        result = assert_transcript(
            "Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.",
            expected,
        )
        self.assertEqual(result["word_errors"], 0)

    def test_transcription_rejects_empty_unrelated_and_half_missing_output(self):
        expected = load_fixture().transcript
        for actual in ("", "Thank you for watching.", "Mister Quilter is the apostle", expected * 5):
            with self.subTest(kind=len(actual)), self.assertRaises(AssertionError):
                assert_transcript(actual, expected)

    def test_transcription_tolerates_one_word_error_without_echoing_output(self):
        expected = load_fixture().transcript
        result = assert_transcript(expected.replace("GLAD", "HAPPY"), expected)
        self.assertEqual(result["word_errors"], 1)
        with self.assertRaises(AssertionError) as caught:
            assert_transcript("unrelated response text", expected)
        self.assertNotIn("unrelated response text", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
