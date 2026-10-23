"""A partial release must never trigger a false Windows upgrade."""
import importlib.util
import json
import pathlib
import plistlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "stage-macos-release.py"
spec = importlib.util.spec_from_file_location("mac_stage", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class MacReleaseTests(unittest.TestCase):
    def test_legacy_manifest_is_byte_identical_and_new_manifest_is_mac_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            bundle = root / "src-tauri/target/aarch64-apple-darwin/release/bundle"
            (bundle / "macos/Dictámelo.app/Contents").mkdir(parents=True)
            (bundle / "dmg").mkdir()
            (root / "src-tauri/tauri.conf.json").write_text('{"version":"1.0.1"}')
            (bundle / "macos/Dictámelo.app/Contents/Info.plist").write_bytes(
                plistlib.dumps({"CFBundleShortVersionString": "1.0.1"}))
            for name in ["macos/Dictámelo.app.tar.gz", "macos/Dictámelo.app.tar.gz.sig", "dmg/Dictamelo_1.0.1_aarch64.dmg"]:
                (bundle / name).write_bytes(b"fixture")
            (root / "docs/releases").mkdir(parents=True)
            (root / "docs/releases/1.0.1.md").write_text("Mac-only fixture release")
            legacy = root / "previous.json"
            original = json.dumps({"version": "1.0.0", "platforms": {
                name: {"url": f"https://example.com/1.0.0/{name}", "signature": "unchanged"}
                for name in ["darwin-aarch64", "windows-x86_64", "windows-aarch64"]
            }}, indent=4).encode() + b"\n"
            legacy.write_bytes(original)
            stage = module.stage_release(root, "1.0.1", legacy)
            self.assertEqual((stage / "latest.json").read_bytes(), original)
            manifest = json.loads((stage / "latest-macos.json").read_text())
            self.assertEqual(manifest["version"], "1.0.1")
            self.assertEqual(set(manifest["platforms"]), {"darwin-aarch64"})
            self.assertFalse(list(stage.glob("*.exe")))
            self.assertEqual(len((stage / "SHA256SUMS.txt").read_text().splitlines()), 5)
            with self.assertRaises(FileExistsError):
                module.stage_release(root, "1.0.1", legacy)
