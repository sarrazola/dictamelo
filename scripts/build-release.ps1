# Compila el instalador de Windows (NSIS, por usuario) con la configuración de
# src-tauri/tauri.windows.conf.json. Equivale a build-release.sh en macOS.
#
# Uso: powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 [argumentos extra de `tauri build`]
#
# Si TAURI_SIGNING_PRIVATE_KEY está definida, el bundle sale firmado para el actualizador.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# En Windows ARM64 las bibliotecas de criptografía (`aws-lc-sys`, `ring`) se compilan con Clang:
# la primera lo encuentra sola dentro de Visual Studio, pero `ring` invoca `clang` a secas y hay
# que tenerlo en el PATH. El componente «C++ Clang Compiler for Windows» lo instala aquí.
if (-not (Get-Command clang -ErrorAction SilentlyContinue)) {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vs = & $vswhere -latest -products * -property installationPath
        $arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'ARM64' } else { 'x64' }
        $llvm = Join-Path $vs "VC\Tools\Llvm\$arch\bin"
        if (Test-Path (Join-Path $llvm 'clang.exe')) {
            $env:Path = "$llvm;$env:Path"
            Write-Host "Usando Clang de Visual Studio: $llvm"
        }
    }
}

if (-not (Test-Path 'node_modules\@tauri-apps\cli')) {
    Write-Host 'Instalando @tauri-apps/cli…'
    npm install --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'npm install falló' }
}

# Windows no admite variables de entorno vacías: `$env:VAR = ''` la borra en vez de dejarla en
# blanco. Como la llave del actualizador no tiene contraseña, Tauri no encontraría
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD, la pediría por consola y la compilación se quedaría colgada
# sin decir por qué. .NET sí sabe escribir `VAR=` en el bloque de entorno del proceso hijo.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $env:ComSpec
$psi.Arguments = "/c npx tauri build $($args -join ' ')"
$psi.WorkingDirectory = (Get-Location).Path
$psi.UseShellExecute = $false   # hereda esta consola: la salida de la compilación se ve igual
if ($env:TAURI_SIGNING_PRIVATE_KEY -and -not $psi.EnvironmentVariables.ContainsKey('TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
    $psi.EnvironmentVariables['TAURI_SIGNING_PRIVATE_KEY_PASSWORD'] = ''
}
$build = [System.Diagnostics.Process]::Start($psi)
$build.WaitForExit()
if ($build.ExitCode -ne 0) { throw "tauri build falló (código $($build.ExitCode))" }

Write-Host ''
Write-Host 'Resultados:'
Get-ChildItem 'src-tauri\target\release\bundle\nsis\*' -Include '*.exe', '*.zip', '*.sig' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
Get-ChildItem 'src-tauri\target\release\*.exe' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
