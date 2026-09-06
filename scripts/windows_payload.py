"""Verify unsigned NSIS payload bytes against Tauri's restored compiler output.

Tauri CLI 2.11.4 patches UNK to NSS before packaging, then restores the original
executable. Only that unique UNK marker's same-offset change is accepted here;
any pre-existing NSS strings must remain byte-identical.
See docs/RELEASING.md for the pinned upstream implementation and version checks.
This helper reads files only; it never normalizes or rewrites an artifact.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct


COMPILED_MARKER = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
PACKAGED_MARKER = b"__TAURI_BUNDLE_TYPE_VAR_NSS"
MACHINES = {"0x8664", "0xaa64"}


def pe_machine(data):
    """Read a bounded PE32+ header; an NSIS x86 bootstrap is not an app payload."""
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("Payload is missing its DOS header")
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if offset < 64 or offset + 24 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        raise ValueError("Payload has an invalid or truncated PE header")
    optional_size = struct.unpack_from("<H", data, offset + 20)[0]
    if optional_size < 112 or offset + 24 + optional_size > len(data):
        raise ValueError("Payload has an invalid or truncated PE optional header")
    if struct.unpack_from("<H", data, offset + 24)[0] != 0x20B:
        raise ValueError("Payload must be a PE32+ application, not an installer bootstrap")
    return f"0x{struct.unpack_from('<H', data, offset + 4)[0]:x}"


def verify_payloads(compiled, packaged, expected_machine):
    expected_machine = expected_machine.lower()
    if expected_machine not in MACHINES:
        raise ValueError("Expected machine must be Windows x64 or ARM64")
    compiled_machine = pe_machine(compiled)
    packaged_machine = pe_machine(packaged)
    if compiled_machine != expected_machine or packaged_machine != expected_machine:
        raise ValueError("Compiler output or packaged payload has the wrong PE machine")
    if len(compiled) != len(packaged):
        raise ValueError("Compiler output and packaged payload differ in length")
    if compiled.count(COMPILED_MARKER) != 1:
        raise ValueError("Compiler output must contain exactly one UNK marker")
    offset = compiled.index(COMPILED_MARKER)
    if packaged[offset:offset + len(PACKAGED_MARKER)] != PACKAGED_MARKER:
        raise ValueError("Packaged payload is missing the NSS marker at the compiler's UNK offset")
    expected = compiled[:offset] + PACKAGED_MARKER + compiled[offset + len(COMPILED_MARKER):]
    if packaged != expected:
        raise ValueError("Packaged payload differs beyond Tauri's documented UNK-to-NSS marker")
    compiled_hash = hashlib.sha256(compiled).hexdigest()
    return {
        # Keep the old field for existing artifact consumers, but name its scope.
        "payloadSha256": compiled_hash,
        "payloadSha256Source": "restored-compiler-output",
        "payloadMachine": compiled_machine,
        "compiledPayloadSha256": compiled_hash,
        "compiledPayloadMachine": compiled_machine,
        "packagedPayloadSha256": hashlib.sha256(packaged).hexdigest(),
        "packagedPayloadMachine": packaged_machine,
        "payloadBytes": len(packaged),
        "bundleTypeTransformation": {
            "from": COMPILED_MARKER.decode("ascii"),
            "to": PACKAGED_MARKER.decode("ascii"),
            "offset": offset,
            "preexistingNssMarkers": compiled.count(PACKAGED_MARKER),
            "onlyDifferenceVerified": True,
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiled", required=True, type=Path)
    parser.add_argument("--packaged", required=True, type=Path)
    parser.add_argument("--machine", required=True, choices=sorted(MACHINES))
    args = parser.parse_args()
    try:
        metadata = verify_payloads(args.compiled.read_bytes(), args.packaged.read_bytes(), args.machine)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Windows payload verification failed: {error}\n")
    print(json.dumps(metadata))


if __name__ == "__main__":
    main()
