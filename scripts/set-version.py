#!/usr/bin/env python3
"""Synchronize application manifests before testing/building a new release."""
import json, pathlib, re, subprocess, sys
version = sys.argv[1] if len(sys.argv) == 2 else ''
if not re.fullmatch(r'\d+\.\d+\.\d+', version):
    raise SystemExit('Usage: python3 scripts/set-version.py X.Y.Z')
root = pathlib.Path(__file__).resolve().parents[1]
for name in ('package.json', 'src-tauri/tauri.conf.json'):
    p = root / name
    value = json.loads(p.read_text()); value['version'] = version
    p.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
p = root / 'src-tauri/Cargo.toml'
p.write_text(re.sub(r'^version = "[^"]+"', f'version = "{version}"', p.read_text(), count=1, flags=re.M))
subprocess.run(['npm', 'install', '--package-lock-only', '--ignore-scripts', '--no-audit', '--no-fund'], cwd=root, check=True)
subprocess.run(['cargo', 'check', '--manifest-path', 'src-tauri/Cargo.toml', '--quiet'], cwd=root, check=True)
print(f'Synchronized manifests and lockfiles to {version}. Update CHANGELOG, release notes, and README before committing.')
