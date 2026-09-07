#!/usr/bin/env python3
"""Package an existing app with Dictámelo's Finder installation layout.

This only creates the container. The release wrapper signs, notarizes and
staples it afterward. Existing output files are never replaced here.
"""

import argparse
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unicodedata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path, help="Existing Dictámelo.app bundle")
    parser.add_argument("output", type=Path, help="New .dmg output path")
    args = parser.parse_args()
    app = args.app.resolve()
    output = args.output.resolve()
    if app.suffix != ".app" or not (app / "Contents/Info.plist").is_file():
        parser.error("The source must be an existing application bundle.")
    with (app / "Contents/Info.plist").open("rb") as handle:
        info = plistlib.load(handle)
    if info.get("CFBundleIdentifier") != "com.dictamelo.desktop":
        parser.error("The source must be the Dictámelo application.")
    if output.suffix != ".dmg":
        parser.error("The output filename must end in .dmg.")
    if output.exists():
        parser.error("The output already exists; choose a new path.")
    try:
        from dmgbuild import build_dmg
    except ImportError:
        parser.error("Install scripts/requirements-dmg.txt in a build virtualenv first.")

    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)

    def verify_payload(mount_point, _options):
        # FinderInfo attributes on a signed bundle invalidate strict codesign
        # verification. Never set hide_extensions on the app or re-sign it here.
        subprocess.run(
            ["codesign", "--verify", "--deep", "--strict", str(Path(mount_point) / "Dictámelo.app")],
            check=True,
        )

    output.parent.mkdir(parents=True, exist_ok=True)
    scripts = Path(__file__).resolve().parent
    # Render vector artwork with the macOS system font at Retina resolution.
    # dmgbuild writes Finder metadata directly, without controlling Finder or
    # requiring Automation permission on the build machine.
    with tempfile.TemporaryDirectory(prefix="dictamelo-dmg-") as directory:
        background = Path(directory) / "installer-background.png"
        subprocess.run(
            ["xcrun", "swift", str(scripts / "generate-dmg-background.swift"), str(background)],
            check=True,
        )
        build_dmg(
            str(output),
            "Dictámelo",
            settings={
                "format": "UDZO",
                "compression_level": 9,
                "filesystem": "HFS+",
                "files": [(str(app), "Dictámelo.app")],
                "symlinks": {"Applications": "/Applications"},
                "background": str(background),
                "icon": str(scripts.parent / "src-tauri/icons/icon.icns"),
                "create_hook": verify_payload,
                # Allow 32 extra points for Finder's title bar so the entire
                # 660 x 400 background remains visible on current macOS.
                "window_rect": ((140, 120), (660, 432)),
                "default_view": "icon-view",
                "show_toolbar": False,
                "show_sidebar": False,
                "show_status_bar": False,
                "show_tab_view": False,
                "show_pathbar": False,
                "arrange_by": None,
                "icon_size": 112,
                "text_size": 13,
                # HFS+ stores the accented bundle name in decomposed form.
                # Finder otherwise ignores its saved position and auto-places it.
                "icon_locations": {
                    unicodedata.normalize("NFD", "Dictámelo.app"): (172, 215),
                    "Applications": (488, 215),
                },
            },
        )
    print(f"Created DMG: {output}")
    print("Sign, notarize and staple this DMG before distribution.")


if __name__ == "__main__":
    if sys.platform != "darwin":
        sys.exit("DMG packaging requires macOS.")
    main()
