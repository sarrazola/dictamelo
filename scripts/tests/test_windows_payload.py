"""Offline evidence that packaging checks reject every change except UNK-to-NSS."""
import hashlib
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from windows_payload import COMPILED_MARKER, PACKAGED_MARKER, verify_payloads


def payload(machine=0x8664, marker=COMPILED_MARKER):
    # Minimal bounded PE32+ headers plus synthetic data; no executable instructions.
    data = bytearray(512)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 64)
    data[64:68] = b"PE\0\0"
    struct.pack_into("<H", data, 68, machine)
    struct.pack_into("<H", data, 84, 240)
    struct.pack_into("<H", data, 88, 0x20B)
    data[400:400 + len(marker)] = marker
    return bytes(data)


class WindowsPayloadTests(unittest.TestCase):
    def test_exact_transformation_for_both_targets_records_distinct_hashes(self):
        for machine in (0x8664, 0xAA64):
            with self.subTest(machine=machine):
                compiled = payload(machine)
                packaged = payload(machine, PACKAGED_MARKER)
                result = verify_payloads(compiled, packaged, f"0x{machine:x}")
                self.assertEqual(result["compiledPayloadSha256"], hashlib.sha256(compiled).hexdigest())
                self.assertEqual(result["packagedPayloadSha256"], hashlib.sha256(packaged).hexdigest())
                self.assertNotEqual(result["compiledPayloadSha256"], result["packagedPayloadSha256"])
                self.assertEqual(result["payloadSha256"], result["compiledPayloadSha256"])
                self.assertEqual(result["payloadSha256Source"], "restored-compiler-output")
                self.assertEqual(result["bundleTypeTransformation"]["offset"], 400)
                self.assertTrue(result["bundleTypeTransformation"]["onlyDifferenceVerified"])

    def test_preexisting_nss_marker_remains_unchanged(self):
        compiled = bytearray(payload())
        compiled[350:350 + len(PACKAGED_MARKER)] = PACKAGED_MARKER
        packaged = bytearray(compiled)
        packaged[400:400 + len(COMPILED_MARKER)] = PACKAGED_MARKER
        result = verify_payloads(bytes(compiled), bytes(packaged), "0x8664")
        self.assertEqual(result["bundleTypeTransformation"]["preexistingNssMarkers"], 1)
        self.assertEqual(result["bundleTypeTransformation"]["offset"], 400)
        packaged[350] ^= 1
        with self.assertRaisesRegex(ValueError, "differs beyond"):
            verify_payloads(bytes(compiled), bytes(packaged), "0x8664")

    def test_wrong_machine_in_either_binary_is_rejected(self):
        for compiled_machine, packaged_machine in ((0xAA64, 0x8664), (0x8664, 0xAA64)):
            with self.subTest(compiled=compiled_machine, packaged=packaged_machine):
                with self.assertRaisesRegex(ValueError, "wrong PE machine"):
                    verify_payloads(payload(compiled_machine), payload(packaged_machine, PACKAGED_MARKER), "0x8664")

    def test_unrelated_changed_byte_is_rejected(self):
        packaged = bytearray(payload(marker=PACKAGED_MARKER))
        packaged[350] ^= 1
        with self.assertRaisesRegex(ValueError, "differs beyond"):
            verify_payloads(payload(), bytes(packaged), "0x8664")

    def test_duplicate_marker_in_either_binary_is_rejected(self):
        for change_compiled in (True, False):
            compiled, packaged = bytearray(payload()), bytearray(payload(marker=PACKAGED_MARKER))
            destination = compiled if change_compiled else packaged
            marker = COMPILED_MARKER if change_compiled else PACKAGED_MARKER
            destination[450:450 + len(marker)] = marker
            with self.subTest(compiled=change_compiled):
                message = "exactly one" if change_compiled else "differs beyond"
                with self.assertRaisesRegex(ValueError, message):
                    verify_payloads(bytes(compiled), bytes(packaged), "0x8664")

    def test_missing_or_unexpected_marker_is_rejected(self):
        for compiled_marker, packaged_marker in (
            (b"_" * len(COMPILED_MARKER), PACKAGED_MARKER),
            (COMPILED_MARKER, COMPILED_MARKER),
            (PACKAGED_MARKER, PACKAGED_MARKER),
        ):
            with self.subTest(compiled=compiled_marker, packaged=packaged_marker):
                with self.assertRaisesRegex(ValueError, "exactly one|missing the NSS marker"):
                    verify_payloads(payload(marker=compiled_marker), payload(marker=packaged_marker), "0x8664")

    def test_marker_at_different_offset_is_rejected(self):
        packaged = bytearray(payload(marker=PACKAGED_MARKER))
        packaged[400:400 + len(PACKAGED_MARKER)] = bytes(len(PACKAGED_MARKER))
        packaged[450:450 + len(PACKAGED_MARKER)] = PACKAGED_MARKER
        with self.assertRaisesRegex(ValueError, "missing the NSS marker"):
            verify_payloads(payload(), bytes(packaged), "0x8664")

    def test_added_or_truncated_bytes_are_rejected(self):
        for packaged in (payload(marker=PACKAGED_MARKER) + b"x", payload(marker=PACKAGED_MARKER)[:-1]):
            with self.assertRaisesRegex(ValueError, "differ in length"):
                verify_payloads(payload(), packaged, "0x8664")

    def test_invalid_pe_headers_are_rejected(self):
        for offset, replacement in ((0, b"ZZ"), (0x3C, b"\xff" * 4), (64, b"NONE"), (84, b"\xff\xff"), (88, b"\x0b\x01")):
            compiled = bytearray(payload())
            compiled[offset:offset + len(replacement)] = replacement
            with self.subTest(offset=offset):
                with self.assertRaises(ValueError):
                    verify_payloads(bytes(compiled), payload(marker=PACKAGED_MARKER), "0x8664")


if __name__ == "__main__":
    unittest.main()
