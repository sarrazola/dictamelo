#!/usr/bin/env python3
"""Assemble all platforms, inspect PE architecture, and generate an updater manifest."""
import datetime, hashlib, json, pathlib, plistlib, re, shutil, struct, sys
version = sys.argv[1] if len(sys.argv) == 2 else ''
if not re.fullmatch(r'\d+\.\d+\.\d+', version): raise SystemExit('Usage: scripts/stage-release.py X.Y.Z')
root = pathlib.Path(__file__).resolve().parents[1]
conf = json.loads((root/'src-tauri/tauri.conf.json').read_text())
assert conf['version'] == version, 'Version mismatch'
stage = root/'dist'/f'v{version}'; stage.mkdir(parents=True, exist_ok=True)
mac = root/'src-tauri/target/aarch64-apple-darwin/release/bundle'
app = mac/'macos/Dictámelo.app'
with (app/'Contents/Info.plist').open('rb') as f:
    assert plistlib.load(f)['CFBundleShortVersionString'] == version, 'Stale macOS app'
files = [
    (mac/f'dmg/Dictamelo_{version}_aarch64.dmg', f'Dictamelo_{version}_aarch64.dmg'),
    (mac/'macos/Dictámelo.app.tar.gz', f'Dictamelo_{version}_aarch64.app.tar.gz'),
    (mac/'macos/Dictámelo.app.tar.gz.sig', f'Dictamelo_{version}_aarch64.app.tar.gz.sig'),
]
for arch, machine in [('x86_64', 0x8664), ('aarch64', 0xaa64)]:
    name = f'Dictamelo_{version}_{arch}-setup.exe'
    path = root/'dist/windows'/name
    b = path.read_bytes(); assert b[:2] == b'MZ', f'Invalid installer: {name}'
    pe = struct.unpack_from('<I', b, 0x3c)[0]
    # NSIS bootstrap can be x86 even for an ARM64/x64 payload. Payload architecture
    # must also be verified by Windows build reports; accept the known NSIS launcher.
    assert b[pe:pe+4] == b'PE\0\0', f'Invalid PE: {name}'
    assert struct.unpack_from('<H', b, pe+4)[0] in (0x14c, machine), f'Wrong PE machine: {name}'
    files.extend([(path,name),(path.with_name(name+'.sig'),name+'.sig')])
for source,name in files:
    assert source.is_file() and source.stat().st_size > 0, f'Missing artifact: {source}'
    shutil.copy2(source, stage/name)
platforms = {}
for platform,name in [('darwin-aarch64',f'Dictamelo_{version}_aarch64.app.tar.gz'),('windows-x86_64',f'Dictamelo_{version}_x86_64-setup.exe'),('windows-aarch64',f'Dictamelo_{version}_aarch64-setup.exe')]:
    platforms[platform] = {'signature': (stage/(name+'.sig')).read_text().strip(), 'url': f'https://github.com/sarrazola/dictamelo/releases/download/v{version}/{name}'}
manifest = {'version':version,'notes':(root/f'docs/releases/{version}.md').read_text(),'pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'platforms':platforms}
(stage/'latest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
(stage/'SHA256SUMS.txt').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in sorted(stage.iterdir()) if p.is_file() and p.name != 'SHA256SUMS.txt'))
print(stage)
