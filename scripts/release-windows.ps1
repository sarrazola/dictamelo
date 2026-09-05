# Añade los artefactos de Windows a un release que ya existe y actualiza latest.json.
#
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.1.2
#
# IMPORTANTE: hay que firmar con LA MISMA llave privada que usa macOS. La app lleva grabada una
# sola llave pública y rechaza cualquier paquete firmado con otra, así que una llave distinta
# dejaría a los usuarios de Windows sin poder actualizar. Pon la llave en la variable
# TAURI_SIGNING_PRIVATE_KEY antes de ejecutar esto.
param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$Repo = 'sarrazola/dictamelo'
$Tag = "v$Version"

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    throw "Falta TAURI_SIGNING_PRIVATE_KEY. Tiene que ser la misma llave que firma en macOS, o las actualizaciones no validarán."
}
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '' }

gh release view $Tag --repo $Repo | Out-Null
if ($LASTEXITCODE -ne 0) { throw "El release $Tag no existe todavía. Publica primero desde macOS." }

Write-Host "==> Poniendo la versión $Version en los manifiestos"
python - $Version @'
import json, re, sys, collections, pathlib
version = sys.argv[1]
p = pathlib.Path('src-tauri/tauri.conf.json')
conf = json.loads(p.read_text(encoding='utf-8'), object_pairs_hook=collections.OrderedDict)
conf['version'] = version
p.write_text(json.dumps(conf, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
p = pathlib.Path('src-tauri/Cargo.toml')
p.write_text(re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', p.read_text(encoding='utf-8'), count=1), encoding='utf-8')
'@

Write-Host '==> Compilando y firmando'
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
if ($LASTEXITCODE -ne 0) { throw 'La compilación falló' }

# El instalador es para quien instala a mano; el .nsis.zip y su .sig son los del actualizador.
$sig = Get-ChildItem 'src-tauri\target\release\bundle\nsis\*.sig' | Select-Object -First 1
if (-not $sig) { throw 'No se generó la firma: ¿la llave no llegó a la compilación?' }
$payload = Get-Item ($sig.FullName -replace '\.sig$', '')
$installer = Get-ChildItem 'src-tauri\target\release\bundle\nsis\*-setup.exe' | Select-Object -First 1

$stage = "dist\$Tag-windows"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $payload.FullName $stage
if ($installer) { Copy-Item $installer.FullName $stage }

Write-Host '==> Añadiendo la entrada de Windows a latest.json'
python - $Version $Tag $Repo $stage $payload.Name $sig.FullName @'
import json, pathlib, subprocess, sys, platform
version, tag, repo, stage, payload_name, sig_path = sys.argv[1:7]

# Se parte del latest.json que ya publicó macOS para no perder su plataforma.
current = subprocess.run(
    ["gh", "release", "view", tag, "--repo", repo, "--json", "assets", "--jq",
     '.assets[] | select(.name=="latest.json") | .url'],
    capture_output=True, text=True, check=True).stdout.strip()
manifest = {"version": version, "notes": f"Dictámelo {version}", "platforms": {}}
if current:
    import urllib.request
    url = f"https://github.com/{repo}/releases/download/{tag}/latest.json"
    with urllib.request.urlopen(url) as r:
        manifest = json.loads(r.read())

arch = "aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64"
manifest.setdefault("platforms", {})[f"windows-{arch}"] = {
    "signature": pathlib.Path(sig_path).read_text(encoding="utf-8").strip(),
    "url": f"https://github.com/{repo}/releases/download/{tag}/{payload_name}",
}
out = pathlib.Path(stage) / "latest.json"
out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print("    plataformas ahora:", ", ".join(manifest["platforms"]))
'@

Write-Host "==> Subiendo al release $Tag"
# --clobber reemplaza latest.json por la versión que ya incluye Windows.
gh release upload $Tag (Get-ChildItem $stage | ForEach-Object { $_.FullName }) --repo $Repo --clobber
if ($LASTEXITCODE -ne 0) { throw 'La subida falló' }

Write-Host ''
Write-Host "Listo. Los Windows instalados verán la actualización en su próxima comprobación."
