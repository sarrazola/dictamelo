# Añade los artefactos de Windows a un release que ya existe y actualiza latest.json.
#
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.1.2
#
# IMPORTANTE: hay que firmar con LA MISMA llave privada que usa macOS. La app lleva grabada una
# sola llave pública y rechaza cualquier paquete firmado con otra, así que una llave distinta
# dejaría a los usuarios de Windows sin poder actualizar. Pon la llave en la variable
# TAURI_SIGNING_PRIVATE_KEY antes de ejecutar esto.
#
# Requisitos: gh (autenticado), python y el entorno de compilación descrito en el README.
param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$Repo = 'sarrazola/dictamelo'
$Tag = "v$Version"

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "La versión debe ser X.Y.Z (recibí «$Version»)" }
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    throw "Falta TAURI_SIGNING_PRIVATE_KEY. Tiene que ser la misma llave que firma en macOS, o las actualizaciones no validarán."
}
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '' }
foreach ($tool in 'gh', 'python') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "Falta «$tool» en el PATH" }
}

# Sin redirigir stderr: con ErrorActionPreference=Stop, `2>$null` sobre un ejecutable nativo
# convierte cualquier línea de error en una excepción y se pierde el mensaje de abajo.
gh release view $Tag --repo $Repo | Out-Null
if ($LASTEXITCODE -ne 0) { throw "El release $Tag no existe todavía. Publica primero desde macOS." }

Write-Host "==> Poniendo la versión $Version en los manifiestos"
# El código va por la entrada estándar: en PowerShell un here-string suelto sería un ARGUMENTO
# más, y `python -` se quedaría esperando un programa que nunca llega.
$setVersion = @'
import json, re, sys, collections, pathlib
version = sys.argv[1]
p = pathlib.Path('src-tauri/tauri.conf.json')
conf = json.loads(p.read_text(encoding='utf-8'), object_pairs_hook=collections.OrderedDict)
conf['version'] = version
p.write_text(json.dumps(conf, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
p = pathlib.Path('src-tauri/Cargo.toml')
p.write_text(re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', p.read_text(encoding='utf-8'), count=1), encoding='utf-8')
print('    manifiestos en', version)
'@
$setVersion | python - $Version
if ($LASTEXITCODE -ne 0) { throw 'No se pudo fijar la versión' }

Write-Host '==> Compilando y firmando'
# Los bundles de versiones anteriores se quedan en la carpeta; se borran para no publicar por
# error un instalador viejo con el nombre de la versión nueva.
$bundleDir = 'src-tauri\target\release\bundle\nsis'
if (Test-Path $bundleDir) { Remove-Item "$bundleDir\*" -Force -Recurse }
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
if ($LASTEXITCODE -ne 0) { throw 'La compilación falló' }

# En Windows el actualizador usa el propio instalador NSIS (Tauri lo firma tal cual, sin
# comprimirlo), así que el mismo archivo sirve para actualizar y para instalar a mano: se publica
# una sola vez. Se filtra por versión: con la carpeta ya limpia debería haber uno solo.
$sig = @(Get-ChildItem "$bundleDir\*_${Version}_*.sig")
if ($sig.Count -ne 1) { throw "Esperaba una firma de la versión $Version y encontré $($sig.Count): ¿la llave no llegó a la compilación?" }
$sig = $sig[0]
$payload = Get-Item ($sig.FullName -replace '\.sig$', '')
if ($payload.Extension -ne '.exe') { throw "Esperaba que la firma cubriera el instalador .exe y cubre $($payload.Name)" }

# La arquitectura sale del nombre que puso Tauri, no de la máquina: así un cruzado no miente.
$arch = if ($payload.Name -match 'arm64|aarch64') { 'aarch64' } elseif ($payload.Name -match 'x64|x86_64') { 'x86_64' } else { throw "No reconozco la arquitectura de $($payload.Name)" }

# Nombre sin acentos, como en macOS: GitHub transforma los caracteres raros al subir el archivo
# y la URL de latest.json dejaría de coincidir con el nombre real del asset.
$payloadName = "Dictamelo_${Version}_${arch}-setup.exe"
$stage = "dist\$Tag-windows"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $payload.FullName (Join-Path $stage $payloadName)

Write-Host "==> Añadiendo la entrada windows-$arch a latest.json"
$buildManifest = @'
import json, pathlib, subprocess, sys, urllib.request
version, tag, repo, stage, payload_name, sig_path, arch = sys.argv[1:8]

# Se parte del latest.json que ya publicó macOS para no perder su plataforma.
published = subprocess.run(
    ["gh", "release", "view", tag, "--repo", repo, "--json", "assets", "--jq",
     '.assets[] | select(.name=="latest.json") | .url'],
    capture_output=True, text=True, check=True).stdout.strip()
manifest = {"version": version, "notes": f"Dictámelo {version}", "platforms": {}}
if published:
    with urllib.request.urlopen(f"https://github.com/{repo}/releases/download/{tag}/latest.json") as r:
        manifest = json.loads(r.read())
if manifest.get("version") != version:
    raise SystemExit(f"latest.json publica la versión {manifest.get('version')}, no {version}")

manifest.setdefault("platforms", {})[f"windows-{arch}"] = {
    "signature": pathlib.Path(sig_path).read_text(encoding="utf-8").strip(),
    "url": f"https://github.com/{repo}/releases/download/{tag}/{payload_name}",
}
out = pathlib.Path(stage) / "latest.json"
out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print("    plataformas ahora:", ", ".join(manifest["platforms"]))
'@
$buildManifest | python - $Version $Tag $Repo $stage $payloadName $sig.FullName $arch
if ($LASTEXITCODE -ne 0) { throw 'No se pudo armar latest.json' }
if (-not (Test-Path (Join-Path $stage 'latest.json'))) { throw 'latest.json no se generó' }

Write-Host "==> Subiendo al release $Tag"
# --clobber reemplaza latest.json por la versión que ya incluye Windows.
gh release upload $Tag (Get-ChildItem $stage | ForEach-Object { $_.FullName }) --repo $Repo --clobber
if ($LASTEXITCODE -ne 0) { throw 'La subida falló' }

Write-Host '==> Comprobando lo publicado'
# Se lee por la API (gh), no por la URL pública de descarga: esa la sirve una caché que durante
# unos minutos sigue devolviendo el latest.json anterior y haría fallar la comprobación en falso.
$publishedJson = gh release download $Tag --repo $Repo --pattern 'latest.json' --output - --clobber
if ($LASTEXITCODE -ne 0) { throw 'No se pudo releer latest.json del release' }
$manifest = $publishedJson | ConvertFrom-Json
$entry = $manifest.platforms."windows-$arch"
if (-not $entry) { throw "El latest.json publicado no tiene windows-$arch" }
if ($entry.url -notlike "*/$payloadName") { throw "La URL de windows-$arch no apunta a $payloadName" }

# Y que el archivo exista con ese nombre exacto: si GitHub lo hubiera renombrado, el actualizador
# se encontraría un 404 y nadie se enteraría hasta que alguien intentara actualizar.
$assets = (gh release view $Tag --repo $Repo --json assets | ConvertFrom-Json).assets
if ($LASTEXITCODE -ne 0) { throw 'No se pudo listar los archivos del release' }
$asset = $assets | Where-Object { $_.name -eq $payloadName }
if (-not $asset) { throw "El release no tiene el archivo $payloadName (hay: $(($assets.name) -join ', '))" }
Write-Host "    latest.json $($manifest.version): $($manifest.platforms.PSObject.Properties.Name -join ', ')"
Write-Host "    windows-$arch -> $payloadName ($('{0:N0}' -f $asset.size) bytes)"

Write-Host ''
Write-Host "Listo. Los Windows instalados verán la actualización en su próxima comprobación."
Write-Host "Prueba la firma de extremo a extremo con:  cd src-tauri; `$env:DICTAMELO_LIVE_TESTS=1; cargo test published_release"
