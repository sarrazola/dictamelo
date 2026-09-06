#!/usr/bin/env python3
"""Mandatory offline build check. Does not contact an API or access OS credentials."""
import sys
import unittest
from pathlib import Path

from audio_fixture import load_fixture


def main() -> int:
    fixture = load_fixture()
    print(
        f"Speech fixture: {fixture.duration_seconds:.3f}s, mono PCM16/16000Hz, "
        f"{fixture.word_count} reference words; SHA-256 verified",
        flush=True,
    )
    tests = unittest.defaultTestLoader.discover(
        str(Path(__file__).resolve().parent / "tests"),
        pattern="test_audio_fixture.py",
    )
    return 0 if unittest.TextTestRunner(verbosity=1).run(tests).wasSuccessful() else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"Speech fixture validation failed: {error}", file=sys.stderr)
        sys.exit(1)
