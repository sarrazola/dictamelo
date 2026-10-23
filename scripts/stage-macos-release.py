#!/usr/bin/env python3
"""Stage a Mac-only release without advertising a new version to Windows."""
import datetime
import hashlib
import json
import pathlib
import plistlib
import re
import shutil
import sys


def stage_release(root, version, legacy_path):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Expected X.Y.Z")
    conf = json.loads((root / "src-tauri/tauri.conf.json").read_text())
    assert conf["version"] == version, "Version mismatch"
    legacy_bytes = legacy_path.read_bytes()
    legacy = json.loads(legacy_bytes)
    assert set(legacy["platforms"]) >= {"darwin-aarch64", "windows-x86_64", "windows-aarch64"}
    assert tuple(map(int, legacy["version"].split("."))) < tuple(map(int, version.split(".")))
    bundle = root / "src-tauri/target/aarch64-apple-darwin/release/bundle"
    with (bundle / "macos/Dictámelo.app/Contents/Info.plist").open("rb") as source:
        assert plistlib.load(source)["CFBundleShortVersionString"] == version, "Stale app"
    files = [
        (bundle / f"dmg/Dictamelo_{version}_aarch64.dmg", f"Dictamelo_{version}_aarch64.dmg"),
        (bundle / "macos/Dictámelo.app.tar.gz", f"Dictamelo_{version}_aarch64.app.tar.gz"),
        (bundle / "macos/Dictámelo.app.tar.gz.sig", f"Dictamelo_{version}_aarch64.app.tar.gz.sig"),
    ]
    for source, _ in files:
        assert source.is_file() and source.stat().st_size > 0, f"Missing artifact: {source}"
    notes = (root / f"docs/releases/{version}.md").read_text()
    stage = root / "dist" / f"v{version}"
    stage.mkdir(parents=True, exist_ok=False)
    for source, name in files:
        shutil.copy2(source, stage / name)
    # Legacy binaries share one global version. Raising it with old Windows
    # URLs would offer Windows an endless reinstall of the same old installer.
    (stage / "latest.json").write_bytes(legacy_bytes)
    name = f"Dictamelo_{version}_aarch64.app.tar.gz"
    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z"),
        "platforms": {"darwin-aarch64": {
            "signature": (stage / (name + ".sig")).read_text().strip(),
            "url": f"https://github.com/sarrazola/dictamelo/releases/download/v{version}/{name}",
        }},
    }
    (stage / "latest-macos.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    (stage / "SHA256SUMS.txt").write_text("".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
        for path in sorted(stage.iterdir()) if path.is_file()
    ))
    return stage


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("Usage: scripts/stage-macos-release.py X.Y.Z /path/to/previous/latest.json")
    print(stage_release(pathlib.Path(__file__).resolve().parents[1], sys.argv[1], pathlib.Path(sys.argv[2])))
