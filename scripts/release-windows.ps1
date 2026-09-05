# Adds the Windows artifacts to a release that already exists and merges them into latest.json.
#
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0 -DryRun
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0 -Targets x86_64-pc-windows-msvc
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0 -AssetsOnly
#
# The release itself is created from macOS (scripts/release.sh). This script only adds the
# Windows side: one signed NSIS installer per target plus the matching `windows-<arch>` entries in
# latest.json, leaving every other platform in the manifest untouched.
#
# With -AssetsOnly it uploads the installers and their detached signatures and stops there,
# without reading or writing latest.json. That is the mode to use when whoever publishes the
# release merges the manifest elsewhere (for example on macOS, once all platforms are built), or
# when the release is still a draft.
#
# IMPORTANT: sign with THE SAME private key macOS uses. The app embeds a single public key and
# rejects any package signed with a different one, so a second key would leave Windows users
# unable to update. Put the key in TAURI_SIGNING_PRIVATE_KEY before running this.
#
# Requirements: gh (authenticated) and the build environment described in the README.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Version,

    # One installer is built and published per target.
    [ValidateSet('aarch64-pc-windows-msvc', 'x86_64-pc-windows-msvc')]
    [string[]]$Targets = @('aarch64-pc-windows-msvc', 'x86_64-pc-windows-msvc'),

    # Reuse the installers already in src-tauri\target\<triple>\release\bundle\nsis.
    [switch]$SkipBuild,

    # Upload the installers and their .sig files only, leaving latest.json alone.
    [switch]$AssetsOnly,

    # Do everything except uploading: useful to check the merged manifest before publishing.
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$Repo = 'sarrazola/dictamelo'
$Tag = "v$Version"
$Stage = "dist\$Tag-windows"

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must be X.Y.Z (got '$Version')" }
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'gh is not on PATH' }
if (-not $SkipBuild -and -not $env:TAURI_SIGNING_PRIVATE_KEY) {
    throw 'TAURI_SIGNING_PRIVATE_KEY is missing. It must be the same key that signs on macOS, or updates will not validate.'
}

# The updater platform key and the asset name come from the target triple, never from the machine
# running the build, so a cross-compiled installer cannot be published under the wrong architecture.
function Get-TargetArch([string]$Target) {
    switch ($Target) {
        'aarch64-pc-windows-msvc' { 'aarch64' }
        'x86_64-pc-windows-msvc' { 'x86_64' }
        default { throw "Unsupported target: $Target" }
    }
}

# Not redirecting stderr on purpose: with ErrorActionPreference=Stop, `2>$null` on a native
# executable turns any stderr line into an exception and the message below would be lost.
gh release view $Tag --repo $Repo | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Release $Tag does not exist yet. Publish from macOS first." }

# ---------------------------------------------------------------------------
# Build and stage one installer per target
# ---------------------------------------------------------------------------

Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $Stage | Out-Null

$built = foreach ($target in $Targets) {
    $arch = Get-TargetArch $target
    $bundleDir = "src-tauri\target\$target\release\bundle\nsis"

    if (-not $SkipBuild) {
        Write-Host ''
        Write-Host "==> Building and signing $target"
        # Bundles from earlier versions stay in the folder; clearing it keeps a stale installer
        # from being published under the new version's name.
        if (Test-Path $bundleDir) { Remove-Item "$bundleDir\*" -Force -Recurse }
        powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target $target
        if ($LASTEXITCODE -ne 0) { throw "Build failed for $target" }
    }

    # On Windows the updater runs the NSIS installer itself (Tauri signs the .exe as it is, it does
    # not zip it), so the same file serves both the updater and manual installs: it is published
    # once. Artifacts are matched by version so a leftover build cannot slip through.
    $installer = @(Get-ChildItem "$bundleDir\*_${Version}_*-setup.exe" -ErrorAction SilentlyContinue)
    if ($installer.Count -ne 1) { throw "Expected exactly one $Version installer in $bundleDir, found $($installer.Count)" }
    $installer = $installer[0]

    $signature = Get-Item "$($installer.FullName).sig" -ErrorAction SilentlyContinue
    if (-not $signature) { throw "Missing $($installer.Name).sig — was the build run without TAURI_SIGNING_PRIVATE_KEY?" }

    # ASCII asset name, like macOS: GitHub rewrites unusual characters on upload and the URL in
    # latest.json would stop matching the real asset name.
    $assetName = "Dictamelo_${Version}_${arch}-setup.exe"
    Copy-Item $installer.FullName (Join-Path $Stage $assetName)
    # The detached signature travels with the installer so whoever assembles latest.json can do it
    # from the release itself, without needing this machine.
    Copy-Item $signature.FullName (Join-Path $Stage "$assetName.sig")

    [pscustomobject]@{
        Target    = $target
        Arch      = $arch
        AssetName = $assetName
        Signature = (Get-Content $signature.FullName -Raw).Trim()
        Bytes     = $installer.Length
    }
}

Write-Host ''
Write-Host '==> Staged artifacts'
$built | ForEach-Object { "    {0}  {1}  ({2:N0} bytes)" -f $_.Target, $_.AssetName, $_.Bytes }

# ---------------------------------------------------------------------------
# Merge latest.json
# ---------------------------------------------------------------------------

if ($AssetsOnly) {
    Write-Host ''
    Write-Host '==> Uploading installers and signatures only (latest.json untouched)'
    if ($DryRun) {
        Write-Host "Dry run: nothing was uploaded. Staged files are in $Stage."
        return
    }
    gh release upload $Tag (Get-ChildItem $Stage | ForEach-Object { $_.FullName }) --repo $Repo --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Upload failed' }

    $assets = (gh release view $Tag --repo $Repo --json assets | ConvertFrom-Json).assets
    if ($LASTEXITCODE -ne 0) { throw 'Could not list the release assets' }
    foreach ($item in $built) {
        foreach ($name in $item.AssetName, "$($item.AssetName).sig") {
            $asset = $assets | Where-Object { $_.name -eq $name }
            if (-not $asset) { throw "The release has no asset named $name" }
            Write-Host ("    {0} ({1:N0} bytes)" -f $name, $asset.size)
        }
    }
    Write-Host ''
    Write-Host 'Done. latest.json was not modified, so nothing is offered as an update yet.'
    return
}

Write-Host ''
Write-Host '==> Merging latest.json'
# Read through gh (the API) and not through the public download URL: that one is served by a cache
# that keeps returning the previous manifest for a few minutes.
$publishedJson = gh release download $Tag --repo $Repo --pattern 'latest.json' --output - --clobber
if ($LASTEXITCODE -ne 0) { throw "Could not read latest.json from $Tag. Publish the macOS side first." }
$manifest = $publishedJson | ConvertFrom-Json
if ($manifest.version -ne $Version) { throw "The published latest.json is version $($manifest.version), not $Version" }
if (-not $manifest.platforms) { throw 'The published latest.json has no platforms object' }

$before = @($manifest.platforms.PSObject.Properties.Name)
foreach ($item in $built) {
    # -Force replaces the entry if this architecture was already published, and only that one:
    # every other platform (macOS, the other Windows arch) is carried over untouched.
    $manifest.platforms | Add-Member -NotePropertyName "windows-$($item.Arch)" -Force -NotePropertyValue ([pscustomobject]@{
            signature = $item.Signature
            url       = "https://github.com/$Repo/releases/download/$Tag/$($item.AssetName)"
        })
}
$after = @($manifest.platforms.PSObject.Properties.Name)
foreach ($platform in $before) {
    if ($after -notcontains $platform) { throw "The merge dropped platform $platform" }
}

# ConvertTo-Json escapes every non-ASCII character; the release notes are written in Spanish, so
# they are turned back into readable text. Both forms are valid JSON, this one is just readable.
$json = ($manifest | ConvertTo-Json -Depth 10)
$json = [regex]::Replace($json, '\\u([0-9a-fA-F]{4})', { param($m) [char][int]('0x' + $m.Groups[1].Value) })
$manifestPath = Join-Path $Stage 'latest.json'
[System.IO.File]::WriteAllText((Resolve-Path $Stage).Path + '\latest.json', $json + "`n", (New-Object System.Text.UTF8Encoding($false)))
Write-Host "    platforms: $($after -join ', ')"
Write-Host "    $manifestPath"

if ($DryRun) {
    Write-Host ''
    Write-Host "Dry run: nothing was uploaded. Review $Stage and run again without -DryRun."
    return
}

# ---------------------------------------------------------------------------
# Upload and verify
# ---------------------------------------------------------------------------

Write-Host ''
Write-Host "==> Uploading to $Tag"
# --clobber replaces latest.json with the version that now includes Windows.
gh release upload $Tag (Get-ChildItem $Stage | ForEach-Object { $_.FullName }) --repo $Repo --clobber
if ($LASTEXITCODE -ne 0) { throw 'Upload failed' }

Write-Host '==> Verifying what was published'
$publishedJson = gh release download $Tag --repo $Repo --pattern 'latest.json' --output - --clobber
if ($LASTEXITCODE -ne 0) { throw 'Could not read latest.json back' }
$published = $publishedJson | ConvertFrom-Json
$assets = (gh release view $Tag --repo $Repo --json assets | ConvertFrom-Json).assets
if ($LASTEXITCODE -ne 0) { throw 'Could not list the release assets' }

foreach ($item in $built) {
    $entry = $published.platforms."windows-$($item.Arch)"
    if (-not $entry) { throw "The published latest.json has no windows-$($item.Arch)" }
    if ($entry.url -notlike "*/$($item.AssetName)") { throw "windows-$($item.Arch) does not point at $($item.AssetName)" }
    # And the file has to exist under that exact name: if GitHub had renamed it, the updater would
    # hit a 404 and nobody would notice until someone tried to update.
    $asset = $assets | Where-Object { $_.name -eq $item.AssetName }
    if (-not $asset) { throw "The release has no asset named $($item.AssetName)" }
    Write-Host ("    windows-{0} -> {1} ({2:N0} bytes)" -f $item.Arch, $item.AssetName, $asset.size)
}
Write-Host "    latest.json $($published.version): $($published.platforms.PSObject.Properties.Name -join ', ')"

Write-Host ''
Write-Host 'Done. Installed Windows copies will see the update on their next check.'
Write-Host 'Verify the signatures end to end with:  cd src-tauri; $env:DICTAMELO_LIVE_TESTS=1; cargo test published_release'
