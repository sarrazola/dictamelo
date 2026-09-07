# Configure the pinned static speech engine and target-specific Windows compilers.
# Dot-source before cargo/tauri commands so the child process inherits this environment.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('aarch64-pc-windows-msvc', 'x86_64-pc-windows-msvc')]
    [string]$Target
)

$ErrorActionPreference = 'Stop'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw 'Visual Studio C++ Build Tools and Windows SDK are required.' }
$nativeVs = & $vswhere -latest -products * -property installationPath
if (-not $nativeVs) { throw 'Could not find a Visual Studio installation.' }

foreach ($nativeToolDir in @(
    "$nativeVs\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin",
    "$nativeVs\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja",
    "$env:ProgramFiles\CMake\bin"
)) {
    if (Test-Path $nativeToolDir) { $env:Path = "$nativeToolDir;$env:Path" }
}
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw 'CMake is required to compile the local speech engine. Install the Visual Studio C++ CMake tools component.'
}

# Replacing arbitrary prior flags is intentional for an official portable build.
# The pinned engine uses the native thread pool; do not introduce OpenMP/BLAS DLLs.
# Explicit SIMD OFF values also reset a cache previously built with host tuning.
$env:TRANSCRIBE_CMAKE_ARGS = '-DGGML_NATIVE=OFF -DTRANSCRIBE_X86_CONSERVATIVE=ON -DGGML_SSE42=OFF -DGGML_AVX=OFF -DGGML_AVX_VNNI=OFF -DGGML_AVX2=OFF -DGGML_BMI2=OFF -DGGML_FMA=OFF -DGGML_F16C=OFF -DGGML_AVX512=OFF -DGGML_AVX512_VBMI=OFF -DGGML_AVX512_VNNI=OFF -DGGML_AVX512_BF16=OFF -DGGML_BLAS=OFF -DTRANSCRIBE_USE_OPENMP=OFF'
if ($env:TRANSCRIBE_DIR) { throw 'Unset TRANSCRIBE_DIR: official builds must compile the pinned native dependency.' }
if ($env:CMAKE_ARGS) { throw 'Unset CMAKE_ARGS: official native build flags must not be overridden.' }
if ($env:CARGO_ENCODED_RUSTFLAGS) { throw 'Unset CARGO_ENCODED_RUSTFLAGS: it would override the required static CRT flags.' }
# Apply the same CRT mode to Rust and every cc/cmake dependency. Tauri's narrower
# static-VCRuntime override leaves C++'s MSVCP140.dll dynamically imported.
$env:RUSTFLAGS = '-C target-feature=+crt-static'
$env:STATIC_VCRUNTIME = 'false'

if ($Target -eq 'aarch64-pc-windows-msvc') {
    if (-not (Get-Command ninja -ErrorAction SilentlyContinue)) { throw 'Ninja is required for the ARM64 Clang build. Install the Visual Studio C++ CMake tools component.' }
    $nativeLlvmHosts = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { @('ARM64', 'x64') } else { @('x64') }
    $nativeClang = @($nativeLlvmHosts | ForEach-Object { "$nativeVs\VC\Tools\Llvm\$_\bin\clang-cl.exe" }) + @("$nativeVs\VC\Tools\Llvm\bin\clang-cl.exe")
    $nativeClang = $nativeClang | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $nativeClang) { throw 'ARM64 local models require the Visual Studio C++ Clang compiler component.' }

    # ggml's ARM NEON sources require Clang, not MSVC cl.exe. Ninja lets cmake-rs
    # use the target-specific compiler without its automatic VS toolset argument.
    $env:CMAKE_GENERATOR_aarch64_pc_windows_msvc = 'Ninja'
    $env:CC_aarch64_pc_windows_msvc = $nativeClang
    $env:CXX_aarch64_pc_windows_msvc = $nativeClang
    $env:TRANSCRIBE_CMAKE_ARGS += ' -DGGML_CPU_ARM_ARCH=armv8-a'

}
# Native tools need the selected target's SDK environment, including after another
# architecture was built in this shell. An x64 host toolchain also runs on ARM Windows.
$nativeVcvars = Join-Path $nativeVs 'VC\Auxiliary\Build\vcvarsall.bat'
$nativeVcarch = if ($Target -eq 'aarch64-pc-windows-msvc') { 'x64_arm64' } else { 'x64' }
# Pass cmd.exe its raw command string through .NET. PowerShell's native-argument
# quoting differs between 5.1 and 7 and can double the quotes around Program Files.
$nativeEnvStart = New-Object System.Diagnostics.ProcessStartInfo
$nativeEnvStart.FileName = $env:ComSpec
$nativeEnvStart.Arguments = "/d /u /s /c `"`"$nativeVcvars`" $nativeVcarch >nul && set`""
$nativeEnvStart.UseShellExecute = $false
$nativeEnvStart.RedirectStandardOutput = $true
$nativeEnvStart.RedirectStandardError = $true
$nativeEnvStart.StandardOutputEncoding = [Text.Encoding]::Unicode
$nativeEnvProcess = [Diagnostics.Process]::Start($nativeEnvStart)
$nativeEnvError = $nativeEnvProcess.StandardError.ReadToEndAsync()
$nativeEnv = $nativeEnvProcess.StandardOutput.ReadToEnd()
$nativeEnvProcess.WaitForExit()
[void]$nativeEnvError.GetAwaiter().GetResult()
$nativeEnvExitCode = $nativeEnvProcess.ExitCode
$nativeEnvProcess.Dispose()
# Captured environment and diagnostics can contain private process variables;
# never print them to the build log, even when setup fails.
if ($nativeEnvExitCode -ne 0) { throw 'Could not initialize the selected Visual Studio/Windows SDK environment.' }
foreach ($nativeLine in ($nativeEnv -split "`r?`n")) {
    if ($nativeLine -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
}
Write-Host "Native speech build: $Target, static portable CPU backend"

function Get-VerifiedWindowsImports {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Executable)
    $dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if (-not $dumpbin) { throw 'dumpbin.exe is required to verify the Windows release dependencies.' }
    $dependencyOutput = & $dumpbin.Source /nologo /dependents $Executable
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect Windows executable dependencies.' }
    $dependencies = @($dependencyOutput | ForEach-Object {
        if ($_ -match '^\s+([^\s\\/]+\.dll)\s*$') { $matches[1].ToLowerInvariant() }
    } | Sort-Object -Unique)
    if (-not $dependencies.Count) { throw 'The PE dependency inspection returned no Windows imports.' }
    # Every other direct import requires an explicit packaging review. In particular,
    # do not rely on a developer's installed VC++/OpenMP/BLAS/GPU runtime or PATH.
    $systemLibraries = @('advapi32.dll','bcrypt.dll','bcryptprimitives.dll','combase.dll','comctl32.dll','crypt32.dll','dwmapi.dll','gdi32.dll','kernel32.dll','kernelbase.dll','mfplat.dll','mfreadwrite.dll','mmdevapi.dll','msvcrt.dll','ntdll.dll','ole32.dll','oleaut32.dll','shell32.dll','shlwapi.dll','ucrtbase.dll','user32.dll','winmm.dll','ws2_32.dll')
    $external = @($dependencies | Where-Object { $_ -notin $systemLibraries -and $_ -notmatch '^(api|ext)-ms-win-[a-z0-9-]+\.dll$' })
    if ($external.Count) { throw "Unbundled Windows runtime dependencies: $($external -join ', ')" }
    return $dependencies
}
