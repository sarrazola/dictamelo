# Windows build report

## 0.5.1 release

Application source: `ee71115a4c4ddf36208fc6a73d1dd097824114b6`. Both official-cloud installers came from [Actions run 34074580032](https://github.com/sarrazola/dictamelo/actions/runs/34074580032), whose two jobs passed. The final EXE bytes were signed with the existing Tauri updater key on macOS and downloaded from the v0.5.1 draft into the Windows VM. Nothing was rebuilt in the VM. Tauri updater signatures are not Microsoft Authenticode signatures.

### Installer and installed-payload identity

| | ARM64 | x86_64 |
| --- | --- | --- |
| Published asset name | `Dictamelo_0.5.1_aarch64-setup.exe` | `Dictamelo_0.5.1_x86_64-setup.exe` |
| Installer bytes | 3,343,867 | 3,718,298 |
| Installer SHA-256 | `291cdb510c0ed4f2dd1c10657c94917afafb8ac94cf6346ddb9969f6831d175c` | `49043f13d365e2c44151931a069dea945e63a65b1387513a649850bffedc5c96` |
| Installed executable bytes | 11,385,344 | 12,990,976 |
| Installed executable SHA-256 | `5377ff536abc40f2f57c119810af48d1910b68044f9599a3c1ea17d1d678fab9` | `94fabb8f559553b49532d3c786a3a1ead8fe558431b97992ed7c07d527a1b45a` |
| Installed PE machine | `0xaa64` | `0x8664` |
| Installed ProductVersion | 0.5.1 | 0.5.1 |
| Runtime environment | Native ARM64, Windows 11 VM | ARM64 Windows emulation; `xtajit64se.dll` loaded |

Both installer hashes match the reviewed CI artifacts. Both installed executable hashes match CI's `packagedPayloadSha256`. The improved CI gate extracted the NSIS payload, verified its PE and version, and proved its only difference from the restored compiler output was the documented `UNK` to `NSS` marker change. ARM64 has zero pre-existing `NSS` markers; x64 has one, preserved unchanged. The installer bootstrap's own x86 PE header is not the application's architecture.

### Installed smoke tests, September 6, 2026

Guest: Windows 11 ARM64 in VMware Fusion, 8,187 MB RAM and four logical processors. Tests used the existing personal Groq credential through the installed application's self-test hooks and separate native paste target; these are real binary/provider checks, not UI mocks. They do not establish physical Intel/AMD or real-microphone behavior.

| Check | Result |
| --- | --- |
| ARM64 upgrade over 0.5.0 | Same install path; exact 0.5.1 payload above. Startup logged `first_run=false`, recognized `Control+Shift+KeyQ`, and showed no onboarding wizard. Settings and history remained byte-identical. |
| Stored credential preservation | Both credential entries remained present. The fixture transcribed immediately without entering a key. No credential values were printed. |
| Licensed speech fixture, both architectures | `tests/fixtures/english-speech.wav` produced “Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.”, matching the corpus words. |
| WMA conversion, ARM64 | Media Foundation converted the existing WMA fixture to 16 kHz mono PCM, 121,679 samples; transcription succeeded and no temporary audio remained. |
| AI cleanup, ARM64 | Same synthetic audio and application: cleanup off retained “Um, so, send the email to Andres on Thursday, no wait, on Friday, and, uh, tell him that the meeting is at 3.” Cleanup on returned “Send the email to Andres on Friday and tell him that the meeting is at 3.” Original settings were restored afterward. |
| Paste into a separate window, both architectures | After explicitly verifying target focus, the transcript reached the target, which logged `Ctrl` and `V`; the previous clipboard was restored. |
| Settings-window behavior, ARM64 | Window stayed open across Alt+Tab and back. Explicit close hid the window while leaving the same process alive. |
| Final restoration | ARM64 0.5.1 installed and running with the exact hash above. Original `settings.json` (`14c5974a…3655`) and `history.json` (`e64af3a8…f0f0`) restored byte-for-byte; both credentials retained. Original 0.5.0 installers preserved for the updater test. |

**Harness observations.** An initial paste attempt was invalid because host-driven Windows Search took focus. The final runs explicitly checked focus and passed on both architectures. Unstable taskbar/tray coordinates also opened unrelated applications during exploration; no settings or account setup was submitted in those applications. An initial cleanup test wrote a UTF-8 BOM into temporary test settings through PowerShell; the app rejected that JSON, logged the warning and used defaults. Rewriting the test settings without a BOM enabled the intended cleanup test. These discarded attempts are not application failures or passing test evidence.

**Not repeated or not covered.** Fresh-profile Skip/restart, physical microphone speech, physical Intel/AMD hardware, Windows cloud-account/free-quota/Pro flows, and a click-by-click Files/export UI run were not revalidated for 0.5.1. Previous 0.5.0 results remain labeled below. Real published-updater execution was verified on native ARM64 and emulated x64 as recorded next; the first x64 attempt required manual Retry and is preserved separately from the successful clean UI repeat.

### Published updater: installed ARM64 0.5.0 to 0.5.1

After v0.5.1 became the public stable Latest release, the preserved ARM64 0.5.0 installer was used only to establish the starting version. Its installer hash matched `f0b50145…cd51`; the installed old executable was verified as 0.5.0, PE `0xaa64`, 11,385,344 bytes, SHA-256 `b47092f1c9cb7cffb6f3c2a4d0e46872092984bfa6a94e59745cdad408d401e2`.

That installed executable was launched with its existing `DICTAMELO_SELFTEST_UPDATE=1` hook. The hook invokes the production `updates.rs` check, download, signature verification and installation functions. The 0.5.1 installer was **not** run manually for this result; this is an actual installed-updater execution, not a visual click test.

The application log recorded the following sequence (timestamps as printed in the log):

```text
20:51:02  Dictámelo 0.5.0 iniciado
20:51:05  updates: Instalando la versión 0.5.1
20:51:06  updates: Descarga completa; aplicando
20:51:10  Dictámelo 0.5.1 iniciado
```

The public endpoint returned version 0.5.1 with all three platforms and the correct `windows-aarch64` EXE URL. Tauri verified the downloaded package's signature before applying it. NSIS ran in passive mode, completed and relaunched the application. The installed result was 0.5.1, PE `0xaa64`, 11,385,344 bytes, SHA-256 `5377ff536abc40f2f57c119810af48d1910b68044f9599a3c1ea17d1d678fab9`, exactly matching the reviewed public payload.

`settings.json` (`14c5974a…3655`) and `history.json` (`e64af3a8…f0f0`) remained byte-identical **through the update**, without restoring them to obtain that result. Both stored credential entries remained intact. A subsequent licensed-fixture transcription succeeded using the stored key; only the history entry added by that post-update test was removed by restoring the original history backup.

**Self-test environment caveat.** The process relaunched by NSIS inherited `DICTAMELO_SELFTEST_UPDATE` from the test process. It therefore ran the diagnostic again, found no newer version and exited. This was a test-launch artifact, not an updater failure; the application was then relaunched without any diagnostic environment variables. Final state: one normal ARM64 0.5.1 instance running, exact payload hash, `Control+Shift+KeyQ` registered, `first_run=false`, original settings/history and both credentials intact. Backups and the 0.5.0 installers were retained.

### Published updater: installed x64 0.5.0 to 0.5.1 under ARM emulation

The preserved official x64 0.5.0 installer matched SHA-256 `dba093828f4adf58ee8046c1270632d6cee9cf91d37b4d30fb651d690cc2ee29`. Before testing, the installed starting executable was verified as 0.5.0, PE `0x8664`, 12,990,976 bytes, SHA-256 `d2b0a0f19fd2e5f1ed4d70fd89419d29565f06282a30f42cd02c69245d5842f7`.

**First attempt: completed with manual Retry.** The installed application's `DICTAMELO_SELFTEST_UPDATE=1` hook downloaded and verified the correct public x64 package, then started NSIS with `/P /UPDATE /R /ARGS`. NSIS PID 9752 remained alive at an `Error opening file for writing: …\Dictámelo\dictamelo.exe` dialog with Abort, Retry and Ignore buttons. This was an installer waiting for input, not an observed process failure or a successful unattended update. At diagnosis there were no `dictamelo.exe` processes and the destination could be opened for writing, but the lock owner at the time of the first write was not captured. The cause remains unproven.

The application log contains one startup in the original test window, at `20:57:39`, with the primary shortcut registered. The harness explicitly stopped any processes after installing 0.5.0 and waited before starting the self-test. A separate ten-sample census over 20 seconds on the clean repeat found zero auto-launched instances after the 0.5.0 installer ran with `/S`. These observations do not support attributing the first dialog to a second application instance, and they do not prove another cause.

After confirming that no application process remained and the file was writable, **Retry was clicked once on the same waiting installer** at 16:09:20.877 guest local time. NSIS exited in under three seconds and relaunched 0.5.1 (application-log timestamp `21:09:23`). The resulting x64 payload matched `94fabb8f559553b49532d3c786a3a1ead8fe558431b97992ed7c07d527a1b45a`, PE `0x8664`, 12,990,976 bytes, FileVersion and ProductVersion 0.5.1. Settings and history remained byte-identical. The original dialog text, button IDs, process information and launch arguments were retained in the VM scratch report `x64-update-dialog-evidence.txt`. This result is explicitly **manually assisted**, although download, signature verification and NSIS execution came from the real updater.

**Clean repeat: passed from the normal application's About page.** The official 0.5.0 x64 starting payload was restored and one normal instance was allowed to finish starting, without a self-test environment variable. Through the existing tray instance, Settings → About visibly showed 0.5.0 and an available 0.5.1 update. The **Install** button was clicked once; the interface showed downloading and closed within ten seconds. No Retry, Abort, Ignore or manually launched 0.5.1 installer was used on this repeat.

Post-update verification found the exact x64 0.5.1 payload above, a single new application process (PID 12168, started at 16:17:59 guest local time), the old PID 11476 gone, no running installers, no pending installer dialogs and no residual updater payloads in the checked local application-data location. The NSIS exit code was not captured because the app launched the installer and it had already exited when inspected; the exact replaced executable and installer-driven relaunch establish completion. `settings.json` (517 bytes) and `history.json` (6,003 bytes, 18 entries) stayed byte-identical to their preflight backups, and both stored credential entries remained present. No fixture or credential-value read was repeated for this final updater-only check.

**Final environment restoration.** After recording the x64 result, the already verified ARM64 0.5.1 installer was run solely to leave the VM in its normal architecture; this manual restoration is not updater evidence. The final executable is PE `0xaa64`, 11,385,344 bytes, version 0.5.1, SHA-256 `5377ff536abc40f2f57c119810af48d1910b68044f9599a3c1ea17d1d678fab9`. One normal instance (PID 9900) was running with the primary shortcut registered, no `DICTAMELO_SELFTEST_UPDATE` or `DICTAMELO_SELFTEST_WAV` environment values at process/user/machine scope, original settings/history unchanged and both credentials intact. No source or published artifact was modified during these tests.

## 0.5.0 release

Application source: `097551f9582fce8c17d6f4a539192d89b80236d8`. Both official-cloud installers came from [Actions run 34008710129](https://github.com/sarrazola/dictamelo/actions/runs/34008710129). Each matrix job passed 63 native x64 Rust tests (three explicit opt-in tests ignored), sixteen Python checks and eleven UI contract/behavior checks. ARM64 was cross-compiled on x64.

| Installer | Bytes | SHA-256 | Verified payload PE |
| --- | ---: | --- | --- |
| `Dictamelo_0.5.0_x86_64-setup.exe` | 3,717,291 | `dba093828f4adf58ee8046c1270632d6cee9cf91d37b4d30fb651d690cc2ee29` | `0x8664` |
| `Dictamelo_0.5.0_aarch64-setup.exe` | 3,343,997 | `f0b501459f0031619f8471db5fe592c11855f8d79b5e64228192a4648abfcd51` | `0xaa64` |

Actions artifact IDs are `9981967895` (x64) and `9981973817` (ARM64). ZIP SHA-256 and byte counts matched the GitHub API. Build metadata matched the exact source, version, target and official-cloud flag. The local EXE header is the x86 NSIS bootstrap, not its payload architecture. Both detached updater signatures were created with the existing signing key and independently verified against the application's public key. There is no Authenticode signature claim.

The local ARM VM was shut down normally before resizing to 8,192 MB RAM and four virtual processors. It was subsequently unlocked and booted; installed runtime results on September 6 are recorded below.

### Installed runtime verification (Windows 11 ARM64 VM)

Guest after the resize, read from the running system: `Win32_ComputerSystem.TotalPhysicalMemory` 8,187 MB, `NumberOfLogicalProcessors` 4, `OSArchitecture` ARM 64-bit. Repository at `C:\Users\andre\Downloads\dictamelo`, fast-forwarded to `3cde602` with a clean working tree; `git merge-base --is-ancestor 097551f… HEAD` confirms the application source, and `git diff --stat 097551f..HEAD -- src-tauri/src ui/` is empty, so only documentation moved.

Installers used: the signed assets downloaded from the v0.5.0 draft with `gh release download v0.5.0 --pattern "*setup.exe*"`. Their SHA-256 equals the table above, equals the CI artifacts from run 34008710129 downloaded separately, and equals the draft's own `SHA256SUMS.txt`.

#### Payload hashes

The installer's own PE header is the x86 NSIS bootstrap for both architectures, so the payload was extracted with 7-Zip and compared directly. `build-verification.json` records the **compiled** executable, which the bundler patches for packaging and then restores; the shipped payload therefore differs until the bundle-type marker is normalized. Each payload was read into memory, checked for the marker, normalized `__TAURI_BUNDLE_TYPE_VAR_NSS` → `…_UNK`, and hashed. No installed or extracted file was modified.

| | ARM64 | x86_64 |
| --- | --- | --- |
| Packaged payload bytes | 11,385,344 | 12,990,976 |
| Packaged payload SHA-256 | `b47092f1c9cb7cffb6f3c2a4d0e46872092984bfa6a94e59745cdad408d401e2` | `d2b0a0f19fd2e5f1ed4d70fd89419d29565f06282a30f42cd02c69245d5842f7` |
| Installed executable equals packaged payload | yes | yes |
| PE machine | `0xaa64` | `0x8664` |
| ProductVersion | 0.5.0 | 0.5.0 |
| `__TAURI_BUNDLE_TYPE_VAR_*` occurrences | 1 (`NSS`) | 2 (both `NSS`) |
| Normalized SHA-256 vs `payloadSha256` | matches `f304984b…f207` | matches `6159f262…82dc` after reversing the second marker |

**x86_64 marker count.** The x64 payload carries two adjacent `NSS` copies, at offsets `0x009a55da` and `0x009a55f5`, with zero `UNK` remaining. Reversing only the **second** reproduces the CI `payloadSha256` exactly; reversing the first, or both, does not. The ARM64 payload has a single occurrence at `0x0089013a` and needs no such choice. This is not a defect in the artifact: the compiled x64 binary genuinely contains one pre-existing `NSS` string plus the one `UNK` marker that Tauri patches, so a rule of "exactly one `NSS` in the packaged payload" is too strict for x64. The verification below, run with the corrected helper, settles it.

#### Passed checks

| Check | Evidence |
| --- | --- |
| ARM64 upgrade over the existing 0.2.0 | Installed 0.2.0 (`891ed0cd…`, PE `0xaa64`, 11,051,008 B) upgraded in place by the 0.5.0 ARM64 installer run with `/S`. Same path, ProductVersion 0.5.0, PE `0xaa64`. Startup logged `first_run=false`; `settings.json` SHA-256 was identical before and after launch, so no wizard and no rewrite on upgrade. |
| Stored credentials survive the upgrade | Models page shows the key as stored in Credential Manager with only its last characters; the licensed fixture transcribed immediately after the upgrade. |
| Clean first launch and Skip | With `settings.json` moved aside, startup logged `first_run=true` and the three-plan wizard appeared ("STEP 1 OF 3"), showing 30 minutes weekly for the free cloud plan. **Skip** closed it, persisted `onboardingSeen: true`, and the next launch logged `first_run=false` with no wizard. |
| No permanent onboarding entry | Sidebar after Skip is General, Plan, Models, Files, History, Advanced, About only. |
| Groq Large v3 recommendation | The model dropdown lists "Whisper Large v3 · Recomendado"; a fresh profile defaults to `whisper-large-v3`. Opening the dropdown did not change the saved selection. |
| Settings window across Alt+Tab | Window stayed `visible` while focus moved to another application and back, twice, with the process alive throughout. |
| Explicit close to tray | Clicking the window's close button left the window `hidden` with the same PID alive; reopening from the tray menu restored it. |
| Licensed audio upload with AI cleanup | `tests/fixtures/english-speech.wav` (SHA-256 `799f78ed…`, LibriSpeech CC BY 4.0) imported through the Files page produced "Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.", matching the corpus transcript word for word after case and punctuation normalization. |
| AI cleanup demonstrably applied to uploads | Same synthetic audio, same build, cleanup on: "Send the email to Andres on Friday and tell him that the meeting is at 3." Cleanup off: "Um, so, send the email to Andres on Thursday, no wait, on Friday, and, uh, tell him that the meeting is at 3." |
| Copy and paste of a file transcript | The **Copiar** button placed the transcript on the clipboard; pasting into a separate window reproduced it exactly, with the target window logging the `Ctrl`/`V` keystrokes. |
| Media conversion | `short.wma` imported: `Media Foundation: … → PCM 16000 Hz, 1 canal(es), remuestreado por el lector`, 121,679 mono samples (7.6 s), transcribed, and the temporary audio directory left empty. |
| Shortcut and cancellation | Holding the configured `Control+Shift+KeyQ` started recording (`Estado: Grabando…`); pressing Esc mid-recording logged `Grabación cancelada con Esc` and returned to Ready without transcribing. |
| Credentials after a full restart | After quitting and relaunching, the shortcut re-registered and the licensed fixture transcribed again using the stored credential. |
| Update check without downgrade | The updater endpoint currently serves 0.1.2 (platforms `darwin-aarch64`, `windows-aarch64`). On ARM64 0.5.0 the startup check produced no update-available event and no error across a 30-second window, so no downgrade was offered. |
| x64 installation under ARM emulation | The draft x64 installer installed a PE `0x8664`, 0.5.0 payload byte-identical to the extracted one; the process loads `xtajit64se.dll`. The licensed fixture transcribed (88 characters in 0.9 s), pasted into a separate window, and the previous clipboard was restored. |

#### Remaining observations

- **Former manifest omitted x64.** During the initial emulated x64 test, the build logged `None of the fallback platforms ["windows-x86_64-nsis", "windows-x86_64"] were found in the response platforms object` against the old 0.1.2 manifest, with no downgrade offered. The final official 0.5.0 Latest manifest now includes `windows-x86_64` and both other platforms, verified through public HTTPS. This resolves the missing metadata; it is not a new x64 installed-updater execution test.
- **The manual update check lives in About, not the tray.** The tray item is `#[cfg(target_os = "macos")]`, but the About page's **Buscar ahora** button is cross-platform and works on Windows; see the measured result below.
- **No single-instance guard.** Launching a second copy while one runs leaves the second unable to register the shortcut; it reports the conflict and falls back to `Alt+Shift+Space`, which is graceful, but two instances can run at once.
- **A second credential entry appears.** 0.5.0 writes `groq.com.dictamelo.desktop.runtime.v1` and keeps the legacy `groq.com.dictamelo.desktop` as a silent migration source, as `secrets.rs` documents. Nothing was deleted.

#### Limitations

- No physical Intel or AMD hardware was involved. Every x64 result here comes from the ARM64 emulation layer, which differs from native execution in timing and CPU feature detection.
- No physical microphone was exercised. The VM's capture device returns silence or low-level noise; the shortcut test proves the device opens, delivers samples and cancels correctly, not audio quality or that speech transcribes from a real microphone.
- All transcription used the existing personal Groq credential in "Free · your API keys" mode. No account sign-in, free weekly quota or Pro licence path was exercised, so the 30 minutes per week and 180 hours per 30 days figures were read from the interface, not consumed.
- The in-app updater was not observed applying an update, only declining to offer one.
- Configuration was backed up before the first-launch test and restored afterwards; `settings.json` SHA-256 `14c5974a…3655` is identical to its pre-test value, and the machine was left with ARM64 0.5.0 installed.

#### Follow-up measurements

**Payload verification with `scripts/windows_payload.py` from `e467b03`.** The helper was read out of that commit into a temporary file outside the working tree and driven against the payloads extracted from the signed draft installers. The compiler output was reconstructed in memory by reversing one `NSS` marker back to `UNK`; the candidate offset was not chosen by hand but accepted only when its **full** SHA-256 equalled the recorded CI `payloadSha256`. Nothing on disk was rewritten.

| | ARM64 | x86_64 |
| --- | --- | --- |
| Packaged payload SHA-256 | `b47092f1c9cb7cffb6f3c2a4d0e46872092984bfa6a94e59745cdad408d401e2` | `d2b0a0f19fd2e5f1ed4d70fd89419d29565f06282a30f42cd02c69245d5842f7` |
| `NSS` offsets in the packaged payload | `0x89013a` | `0x9a55da`, `0x9a55f5` |
| Offset reversed to `UNK` | `0x89013a` (8,978,746) | `0x9a55f5` (10,114,549) |
| Reconstructed compiler SHA-256 | `f304984b7133d1c751337c3027f92fc3ca09c065f063f962671e450e0685f207` | `6159f262ac7e5b08bee88df83e8b1141c9f0da6a00d91050428a168a880682dc` |
| Full CI `payloadSha256` assertion | pass | pass |
| `verify_payloads` | pass | pass |
| `preexistingNssMarkers` reported | 0 | 1 |
| `payloadBytes` | 11,385,344 | 12,990,976 |

Both architectures pass. The helper confirms the packaged payload differs from the compiler output *only* by the documented `UNK`→`NSS` change at the compiler's own marker offset, and that the x64 payload's pre-existing `NSS` string is byte-identical on both sides. The earlier "blocked" note stood only because a single-`NSS` rule cannot describe the x64 binary; with the corrected comparison there is no open question about either artifact.

**Manual update check in About.** The installed ARM64 0.5.0 About page has an **Actualizaciones** row with a **Buscar ahora** button, so the manual check does exist on Windows even though the tray entry is macOS-only. Clicking it changed the row's caption from "Se comprueba sola al abrir…" to **"Estás en la última versión"**, wrote nothing to the log, and left the installed ProductVersion at 0.5.0. The updater endpoint still advertises 0.1.2, so this also confirms the button reports up to date rather than offering a downgrade.

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
