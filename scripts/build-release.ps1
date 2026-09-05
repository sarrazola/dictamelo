# Compila el instalador de Windows (NSIS, por usuario) con la configuración de
# src-tauri/tauri.windows.conf.json. Equivale a build-release.sh en macOS.
#
# Uso: powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 [argumentos extra de `tauri build`]
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

if (-not (Test-Path 'node_modules\@tauri-apps\cli')) {
    Write-Host 'Instalando @tauri-apps/cli…'
    npm install --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'npm install falló' }
}

npx tauri build @args
if ($LASTEXITCODE -ne 0) { throw 'tauri build falló' }

Write-Host ''
Write-Host 'Resultados:'
Get-ChildItem 'src-tauri\target\release\bundle\nsis\*.exe' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
Get-ChildItem 'src-tauri\target\release\*.exe' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
