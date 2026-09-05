# Windows build report — 0.2.0

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

**There is no physical Intel/AMD machine here.** Everything labelled x64 below was produced by
cross-compiling and exercised under the ARM64 x64 emulation layer. See *Emulation* for how far
that evidence goes.

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

### Functional check after upgrade

`DICTAMELO_SELFTEST_WAV` on the installed 0.2.0 ARM64 build: audio transcribed through Groq in
1.0 s (104 characters), pasted with `Ctrl+V`, and the previous clipboard contents restored intact.
The dictation pipeline still works end to end on Windows in 0.2.0.

## Emulation: what the x64 evidence is worth

`IsWow64Process2` reports `processMachine = IMAGE_FILE_MACHINE_UNKNOWN` for these processes, which
is expected: WOW64 means 32-bit on 64-bit, and an x64 process on ARM64 is not that. The positive
evidence is the loaded module list — every x64 build run here loads `xtajit64se.dll`, the ARM64
x64 emulation JIT.

So, for x64:

- **Verified:** it compiles, links, bundles, signs with the right key, installs, produces an
  x86-64 executable with the right version, and starts.
- **Not verified:** behaviour on real Intel/AMD silicon. Emulation and native execution differ in
  timing, in CPU feature detection (the crypto crates select code paths from CPUID) and in audio
  device behaviour. Microphone capture, `SendInput` pasting and Media Foundation conversion were
  **not** exercised on the x64 build at all here — only startup was.

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
