# Windows build report

## 0.4.0 release candidate

Both official cloud installers were built from `374a77cf3329bfaa210eaa3f3977331c0a248a53`, after that source was pushed to `main`. [Windows run 34001827519](https://github.com/sarrazola/dictamelo/actions/runs/34001827519) passed on Windows Server 2022 x64 runners. Each matrix job ran 16 Python tests, 6 UI contract tests and 61 Rust tests; 3 explicitly opt-in tests were ignored.

| Installer | Bytes | Application PE machine | GitHub artifact ID |
| --- | ---: | --- | --- |
| `Dictamelo_0.4.0_x86_64-setup.exe` | 3,716,357 | `0x8664` | `9979889040` |
| `Dictamelo_0.4.0_aarch64-setup.exe` | 3,343,832 | `0xaa64` | `9979900519` |

GitHub artifact ZIP digests were verified before extraction. Build metadata matched the exact commit, version, target and official-cloud configuration. The existing Tauri updater key signed both installers; detached signatures independently passed `verify_artifact`. Signing did not alter the installer payload bytes. The NSIS launcher is x86 for both packages; the compiled application payload determines the target architecture.

ARM64 was cross-compiled on x64. The local ARM VM displayed a black screen and was unavailable for installation or functional checks. This release therefore has native x64 test execution and cross-compiled ARM64 packaging evidence, but no new Windows installation, ARM64 execution, physical Intel/AMD microphone or Windows upgrade test. The installers do not have an Authenticode certificate. Historical VM results below are not results for 0.4.0.

## Historical report — 0.2.0

Record of how the Windows installers are built, what was verified for 0.2.0, and which parts of
that verification are weaker than they look. Written from the Windows machine; the macOS side and
the final `latest.json` are handled elsewhere.

## Machine used

| | |
| --- | --- |
| Hardware | Windows 11 ARM64 virtual machine on Apple Silicon, 2 logical CPUs, 4 GB RAM |
| Rust | 1.98.1, host `aarch64-pc-windows-msvc`, target `x86_64-pc-windows-msvc` added with `rustup target add` |
| Node | 24 LTS (ARM64) |
| Visual Studio | Build Tools 2022 17.14, C++ workload, Windows SDK 10.0.26100, `VC.Tools.ARM64`, `VC.Llvm.Clang` |
| Assembler | NASM 2.16 (`winget install NASM.NASM`), installed per-user in `%LOCALAPPDATA%\bin\NASM` |
| Other | WebView2 152, GitHub CLI 2.100, Python 3.13 |

**There is no physical Intel/AMD machine here.** Local x64 builds were cross-compiled; the draft's
x64 installer was built on a native CI runner. All x64 execution on this VM uses ARM64 emulation.
See *Emulation* for how far that evidence goes.

## Toolchain prerequisites, by target

The TLS stack decides these, and it depends on the **target**, not on the machine doing the build:

| Target | Needs | Why |
| --- | --- | --- |
| `aarch64-pc-windows-msvc` | Clang on `PATH` | `aws-lc-sys` finds `clang-cl` inside Visual Studio on its own, but `ring` invokes plain `clang`, which has to be resolvable. The Visual Studio component installs it under `VC\Tools\Llvm\ARM64\bin`. |
| `x86_64-pc-windows-msvc` | NASM on `PATH` | `aws-lc-sys` and `ring` assemble x86 assembly. Without NASM the build fails inside a build script. |
| Cross-compiling | MSVC compiler/linker for the target | On an ARM64 host, Visual Studio 2022 ships `Hostx64\x64` but **not** `Hostarm64\x64`; adding `VC.Tools.x86.x64` does not create it. The x64 toolchain therefore runs under x64 emulation on this machine. |

`scripts\build-release.ps1` locates all three and fails with an actionable message instead of
letting a build script die on a missing assembler.

## What the scripts do

`scripts\build-release.ps1 -Target <triple>`

- Defaults to the host triple; accepts `aarch64-pc-windows-msvc` or `x86_64-pc-windows-msvc`.
- Installs the Rust standard library for the target if it is missing.
- Passes `--target` to `tauri build`, so artifacts land in
  `src-tauri\target\<triple>\release\bundle\nsis` and the two architectures never overwrite each other.
- Reports when the MSVC toolchain in use runs under emulation.
- Starts `tauri build` through `ProcessStartInfo`. Windows cannot hold an empty environment
  variable — `$env:VAR = ''` deletes it — so `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` would be absent,
  Tauri would prompt for the key password on the console, and the build would hang with no
  explanation. .NET can write `VAR=` into a child environment block; PowerShell cannot.

`scripts\release-windows.ps1 <version> [-Targets ...] [-SkipBuild] [-AssetsOnly] [-DryRun]`

- Builds and stages one installer per target, matching artifacts **by version** so a leftover
  bundle from an earlier build cannot be published under the new version's name.
- Derives the updater platform key (`windows-aarch64`, `windows-x86_64`) and the asset name from
  the **target triple**, never from the host, so a cross-compiled installer cannot be published
  under the wrong architecture.
- Locates the detached `<installer>.exe.sig` explicitly and stages it next to the installer, so
  whoever assembles `latest.json` can do it from the release itself.
- Merges `latest.json` by adding only its own `windows-<arch>` entries, and aborts if any platform
  it did not build disappears from the manifest.
- `-AssetsOnly` skips the manifest entirely: the mode to use while the release is a draft or when
  the manifest is assembled on another machine.
- `-DryRun` stages and merges without uploading.

Asset naming matches `docs/RELEASING.md`: `Dictamelo_<version>_<arch>-setup.exe` and `.exe.sig`.
Tauri names its own output `_arm64-` / `_x64-`; the published name always uses the Rust
architecture (`aarch64`, `x86_64`) that the updater expects.

## 0.2.0 verification

Built from commit `28160ef` with `TAURI_SIGNING_PRIVATE_KEY` set to the same key macOS uses.

| Artifact | Size | Signature key id |
| --- | --- | --- |
| `Dictamelo_0.2.0_aarch64-setup.exe` (this machine) | 3,252,989 B | `63c26faf867696ba` — matches the public key in `tauri.conf.json` |
| `Dictamelo_0.2.0_x86_64-setup.exe` (cross-built here, **not** published) | 3,615,197 B | same key id |
| `Dictamelo_0.2.0_x86_64-setup.exe` (CI build, published in the draft) | 3,616,318 B | same key id |

### Installer versus installed executable

Both NSIS installers report **PE machine `x86`**: the NSIS bootstrap is a 32-bit executable
regardless of payload. Inspecting the installer header proves nothing about the application. Only
the installed executable does:

| Installed from | `%LOCALAPPDATA%\Dictámelo\dictamelo.exe` | ProductVersion | Runs |
| --- | --- | --- | --- |
| ARM64 installer built here | PE machine ARM64 | 0.2.0 | native |
| x64 installer built here | PE machine x86-64 | 0.2.0 | loads `xtajit64se.dll` |
| x64 installer from CI (downloaded from the draft with `gh`) | PE machine x86-64 | 0.2.0 | loads `xtajit64se.dll` |

### Upgrade from 0.1.2

The published `Dictamelo_0.1.2_aarch64-setup.exe` was installed first (ProductVersion 0.1.2, PE
ARM64, 10,934,784 B), then the 0.2.0 ARM64 installer was run over it. The installation was
upgraded in place: same path, ProductVersion 0.2.0, PE ARM64, 11,051,008 B. Settings, history and
the stored credential survived the upgrade.

This covers the **installer** half of an upgrade. It is **not** a test of the in-app updater,
which needs `latest.json` to advertise 0.2.0; the manifest is deliberately untouched while the
release is a draft.

### Functional checks

Both architectures were exercised from their **installed** copies, using the existing personal
Groq credential in Windows Credential Manager. The x64 run used the artifact downloaded from the
draft with `gh`, that is the one built by the native CI runner, not the copy cross-compiled here.

| | ARM64 (built here) | x86_64 (CI artifact) |
| --- | --- | --- |
| Installed executable | PE ARM64, v0.2.0, 11,051,008 B | PE x86-64, v0.2.0, 12,600,320 B |
| `DICTAMELO_SELFTEST_WAV`, transcription | 104 characters in 1.0 s | 104 characters in 0.9 s |
| Paste into a real window | not captured | `scripts\paste_target.ps1` logged `Ctrl` down, `V` down, `text len=104` |
| Clipboard restored | yes | yes, byte-identical to the marker set before the run |
| `DICTAMELO_SELFTEST_HOTKEY_SECS=6`, microphone | Historical 0.1.2 result: 5.7 s captured; not repeated for 0.2.0 | 4.89 s captured on 0.2.0 |
| Capture device | `Microphone (High Definition Audio Device)`, 48 kHz, 2 channels, F32 | same |

On the x64 run the global hotkey (`RegisterHotKey`) received its synthetic press and release,
WASAPI opened the capture stream, the audio was resampled to 16 kHz mono, transcribed and pasted.
Both architectures log `A buffer underrun or overrun occurred` warnings while the stream starts;
they are non-fatal by design and the recording continues.

**Limit of the microphone evidence.** This VM has no real audio input: the capture device returns
silence or low-level noise, so Whisper returns a filler phrase (`Thank you.` on this run) rather
than a transcript of anything spoken. What the test proves is that the device opens, the stream
delivers samples for the whole hold, and the pipeline carries them through transcription and
paste. It does **not** prove audio quality, gain, or that a real voice transcribes correctly on
x64 hardware.

Recording duration was shorter than the 6.0 s hold: 4.89 s on the current x64 build and 5.7 s in
the historical ARM64 test. Stream start-up and the self-test's modifier steps may contribute;
these individual runs on a loaded 2-CPU VM do not establish the cause or typical latency.

## Script guards (validated at `df0b29c`)

| Check | Result |
| --- | --- |
| PowerShell syntax of both scripts, tokenizer and AST parser | No errors. Both files keep their UTF-8 BOM, which PowerShell 5.1 needs to read the accented characters correctly. |
| `release-windows.ps1 0.2.0 -SkipBuild -AssetsOnly -DryRun` | Staged both installers and both `.exe.sig` files into `dist\v0.2.0-windows`, uploaded nothing. Draft assets byte-for-byte identical before and after. |
| `release-windows.ps1 0.1.2 -AssetsOnly` against the **public** v0.1.2 | Refused in 3.2 s with *"Public release artifacts are immutable. Create a new version and upload to its draft."* No compiler ran, no local bundle was touched, and the four v0.1.2 assets were unchanged. |
| `release-windows.ps1 9.9.9 -SkipBuild -AssetsOnly` | *"Release v9.9.9 does not exist yet. Create a draft from macOS first."* |

`npm ci` needs `package-lock.json`, which is committed, so the build path is unaffected.

**One sharp edge worth knowing.** `-Targets` defaults to *both* architectures, so a plain
`-AssetsOnly` run from this machine stages the locally cross-compiled x64 installer and would
replace the CI-built `Dictamelo_0.2.0_x86_64-setup.exe` in the draft (3,615,197 B against the CI
build's 3,616,318 B — different bytes, both valid and correctly signed). While the published x64
artifact comes from the native runner, pass the target explicitly:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0 -Targets aarch64-pc-windows-msvc -SkipBuild -AssetsOnly
```

That is how the ARM64 pair was uploaded; the x86_64 assets were left untouched.

## Emulation: what the x64 evidence is worth

`IsWow64Process2` reports `processMachine = IMAGE_FILE_MACHINE_UNKNOWN` for these processes, which
is expected: WOW64 means 32-bit on 64-bit, and an x64 process on ARM64 is not that. The positive
evidence is the loaded module list — every x64 build run here loads `xtajit64se.dll`, the ARM64
x64 emulation JIT.

So, for x64:

- **Verified:** it compiles, links, bundles, signs with the right key, installs, produces an
  x86-64 executable with the right version, starts, registers the global hotkey, captures from the
  microphone through WASAPI, reaches Groq over TLS, pastes into another application with
  `SendInput`, and restores the previous clipboard.
- **Not verified:** behaviour on real Intel/AMD silicon. Emulation and native execution differ in
  timing, in CPU feature detection (the crypto crates select code paths from CPUID) and in audio
  device behaviour, and this VM's microphone produces no real audio. Media Foundation audio-file
  conversion was not exercised on x64.

The x64 installer in the draft comes from the native CI workflow, which is stronger evidence than
anything this VM can produce. Testing it on physical Intel/AMD hardware is still pending.

## Not covered

- Account sign-in and the free weekly quota: blocked on SMTP delivery. No 0.2.0 auth path was
  exercised; the functional check above used a personal Groq key.
- The in-app update from 0.1.2 to 0.2.0, for the reason given above.
- Pasting into an elevated application. Windows blocks synthetic input from a normal process to an
  elevated one (UIPI); the text stays on the clipboard, by design.
- Microsoft Authenticode. The installers carry the updater signature only, so SmartScreen will warn
  until the binaries build reputation.
- Windows 10, and Windows on 32-bit.

## Reproducing

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = '<same key as macOS>'
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Target aarch64-pc-windows-msvc

# Upload installers and signatures to a draft, leaving latest.json alone:
powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 0.2.0 -SkipBuild -AssetsOnly
```

A full x64 + ARM64 build from a clean target directory takes roughly 40 minutes on this VM and
needs about 3 GB per target, so keep an eye on disk space.
