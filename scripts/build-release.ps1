# Builds the signed Windows NSIS installer for one explicit Rust target.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target x86_64-pc-windows-msvc
#
# The target is always passed to `tauri build`, so the output lands in
# src-tauri\target\<triple>\release\bundle\nsis and an ARM64 machine can produce x64 installers
# (and the other way round) without the two builds ever overwriting each other.
#
# If TAURI_SIGNING_PRIVATE_KEY is set, the bundle is signed for the updater and Tauri writes the
# detached signature next to the installer as <installer>.exe.sig.
#
# This is the Windows counterpart of build-release.sh (macOS).
[CmdletBinding()]
param(
    # Rust target triple. Defaults to the host triple reported by rustc.
    [ValidateSet('aarch64-pc-windows-msvc', 'x86_64-pc-windows-msvc')]
    [string]$Target,

    # Extra arguments forwarded verbatim to `tauri build`.
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs = @()
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# Check UI source contracts without browser dependencies or account access.
node --check ui/main.js
if ($LASTEXITCODE -ne 0) { throw 'UI syntax check failed' }
node --check ui/i18n.js
if ($LASTEXITCODE -ne 0) { throw 'Translation syntax check failed' }
npm run test:ui
if ($LASTEXITCODE -ne 0) { throw 'UI translation/control contract tests failed' }

# Validate the committed speech-upload fixture before toolchain setup or signing.
# This is offline unless the maintainer explicitly enables the live regression.
$fixturePython = Get-Command py -ErrorAction SilentlyContinue
$fixturePythonArgs = @('-3')
if (-not $fixturePython) {
    $fixturePython = Get-Command python -ErrorAction SilentlyContinue
    $fixturePythonArgs = @()
}
if (-not $fixturePython) { throw 'Python 3 is required for the offline speech fixture regression.' }
& $fixturePython.Source @fixturePythonArgs scripts/check-audio-fixture.py
if ($LASTEXITCODE -ne 0) { throw 'Offline speech fixture regression failed' }
if ($env:DICTAMELO_LIVE_REGRESSION -eq '1') {
    if (-not $env:DICTAMELO_TEST_PROJECT_REF) { throw 'Set DICTAMELO_TEST_PROJECT_REF for the explicit live regression target.' }
    & $fixturePython.Source @fixturePythonArgs scripts/test-free-cleanup-live.py --live --project-ref $env:DICTAMELO_TEST_PROJECT_REF
    if ($LASTEXITCODE -ne 0) { throw 'Live free-cloud regression failed' }
}

# ---------------------------------------------------------------------------
# Target
# ---------------------------------------------------------------------------

function Get-HostTriple {
    $line = (rustc -vV | Select-String '^host:').ToString()
    if (-not $line) { throw 'Could not read the host triple from `rustc -vV`' }
    return $line.Split(':')[1].Trim()
}

if (-not $Target) { $Target = Get-HostTriple }
# Everything downstream (asset names, updater platform keys) derives from the TARGET, never from
# the machine we happen to be building on.
$targetArch = switch ($Target) {
    'aarch64-pc-windows-msvc' { 'aarch64' }
    'x86_64-pc-windows-msvc' { 'x86_64' }
    default { throw "Unsupported target: $Target" }
}
$hostTriple = Get-HostTriple
$crossBuild = $Target -ne $hostTriple
Write-Host "==> Target: $Target (arch $targetArch)$(if ($crossBuild) { " — cross-compiling from $hostTriple" })"

if ((rustup target list --installed) -notcontains $Target) {
    Write-Host "==> Installing the Rust standard library for $Target"
    rustup target add $Target
    if ($LASTEXITCODE -ne 0) { throw "rustup target add $Target failed" }
}

# ---------------------------------------------------------------------------
# Native toolchain needed by the TLS crates
# ---------------------------------------------------------------------------

function Add-ToPath([string]$Directory, [string]$Why) {
    if ($Directory -and (Test-Path $Directory) -and ($env:Path -notlike "*$Directory*")) {
        $env:Path = "$Directory;$env:Path"
        Write-Host "    $Why`: $Directory"
    }
}

function Get-VisualStudioPath {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) { return (& $vswhere -latest -products * -property installationPath) }
    return $null
}

# `ring` and `aws-lc-sys` (the crypto behind rustls) need a native assembler/compiler that depends
# on the TARGET, not on the host:
#   * aarch64 → Clang. aws-lc-sys finds clang-cl inside Visual Studio by itself, but ring invokes
#     plain `clang`, so its directory has to be on PATH.
#   * x86_64  → NASM, which assembles the x86 assembly both crates ship.
$vs = Get-VisualStudioPath
if ($targetArch -eq 'aarch64' -and -not (Get-Command clang -ErrorAction SilentlyContinue)) {
    if ($vs) {
        $llvmHost = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'ARM64' } else { 'x64' }
        Add-ToPath (Join-Path $vs "VC\Tools\Llvm\$llvmHost\bin") 'Using Clang from Visual Studio'
    }
    if (-not (Get-Command clang -ErrorAction SilentlyContinue)) {
        throw "Building for $Target needs Clang. Install the ""C++ Clang Compiler for Windows"" component of Visual Studio Build Tools."
    }
}
if ($targetArch -eq 'x86_64' -and -not (Get-Command nasm -ErrorAction SilentlyContinue)) {
    foreach ($candidate in "$env:ProgramFiles\NASM", "${env:ProgramFiles(x86)}\NASM", "$env:LOCALAPPDATA\bin\NASM") {
        Add-ToPath $candidate 'Using NASM'
    }
    if (-not (Get-Command nasm -ErrorAction SilentlyContinue)) {
        throw "Building for $Target needs NASM (aws-lc-sys and ring assemble x86 assembly). Install it with: winget install NASM.NASM"
    }
}

# Cross-compiling also needs the MSVC compiler and linker for the target. Without them the build
# dies deep inside a build script with a confusing message, so it is checked up front.
if ($crossBuild -and $vs) {
    $msvcRoot = Join-Path $vs 'VC\Tools\MSVC'
    $msvc = Get-ChildItem $msvcRoot -Directory -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1
    $msvcTarget = if ($targetArch -eq 'x86_64') { 'x64' } else { 'arm64' }
    $hostDirs = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { @('Hostarm64', 'Hostx64') } else { @('Hostx64', 'Hostx86') }
    $found = $false
    foreach ($hostDir in $hostDirs) {
        $bin = Join-Path $msvc.FullName "bin\$hostDir\$msvcTarget"
        if (Test-Path (Join-Path $bin 'cl.exe')) {
            # A Hostx64 toolchain on an ARM64 machine runs under x64 emulation: slower, but it
            # produces exactly the same binaries.
            $emulated = ($hostDir -eq 'Hostx64' -and $env:PROCESSOR_ARCHITECTURE -eq 'ARM64')
            Add-ToPath $bin "Using MSVC $hostDir -> $msvcTarget$(if ($emulated) { ' (runs under x64 emulation)' })"
            $found = $true
            break
        }
    }
    if (-not $found) {
        throw "No MSVC toolchain found for $Target. Install the ""MSVC v143 - VS 2022 C++ x64/x86 build tools"" component of Visual Studio Build Tools."
    }
}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

if (-not (Test-Path 'node_modules\@tauri-apps\cli')) {
    Write-Host '==> Installing @tauri-apps/cli'
    npm ci --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'npm ci failed' }
}

Write-Host '==> Running native regression tests for the requested target'
# A target that cannot execute on this machine needs a compatible test runner;
# compiling it successfully is not sufficient release verification.
cargo test --locked --manifest-path src-tauri/Cargo.toml --target $Target
if ($LASTEXITCODE -ne 0) { throw "Rust regression tests failed for $Target" }

Write-Host '==> Building'
# Windows cannot hold an empty environment variable: `$env:VAR = ''` deletes it instead of leaving
# it blank. The updater key has no password, so Tauri would find no
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD, prompt for one on the console and hang the build with no
# explanation. .NET can write `VAR=` into a child process environment block, which PowerShell cannot.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $env:ComSpec
$psi.Arguments = "/c npx tauri build --target $Target $($ExtraArgs -join ' ')"
$psi.WorkingDirectory = (Get-Location).Path
$psi.UseShellExecute = $false   # inherits this console, so build output shows up as usual
if ($env:TAURI_SIGNING_PRIVATE_KEY -and -not $psi.EnvironmentVariables.ContainsKey('TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
    $psi.EnvironmentVariables['TAURI_SIGNING_PRIVATE_KEY_PASSWORD'] = ''
}
$build = [System.Diagnostics.Process]::Start($psi)
$build.WaitForExit()
if ($build.ExitCode -ne 0) { throw "tauri build failed (exit code $($build.ExitCode))" }

# ---------------------------------------------------------------------------
# Results
# ---------------------------------------------------------------------------

$bundleDir = "src-tauri\target\$Target\release\bundle\nsis"
Write-Host ''
Write-Host "Artifacts in $bundleDir"
Get-ChildItem "$bundleDir\*" -Include '*.exe', '*.sig' -ErrorAction SilentlyContinue | ForEach-Object {
    "    {0}  ({1:N0} bytes)" -f $_.Name, $_.Length
}
$appExe = "src-tauri\target\$Target\release\dictamelo.exe"
if (Test-Path $appExe) { Write-Host "    $appExe" }
