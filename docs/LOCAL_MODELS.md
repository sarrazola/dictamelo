# Local speech models

Version 1.0.0 enables on-device transcription on macOS and Windows alongside personal-key and hosted modes. See the version-specific verification record for actual native and emulated tests; earlier Windows releases did not contain the local engine.

## User behavior

Choose **Local models** in Models or first-run setup. Download a model, choose its audio language when required, and select **Use model**. Downloads show progress, can be cancelled, and can be removed from the app. No Dictámelo account or provider key is required for local transcription. Initial downloads require internet access; downloaded speech models run on the computer.

Local transcription has no hosted time allowance and never silently falls back to a cloud provider. Optional AI cleanup remains a separate cloud feature: sending a local transcript to a personal-key cleaner requires explicit consent in addition to enabling cleanup. Without that consent, the original local transcript is delivered. Normal application update checks and account features are separate from speech processing.

| Model | Download size (decimal MB) | Supported speech |
| --- | ---: | --- |
| Canary 180M Flash, Q8 | 218 | English, German, Spanish, French; explicit language required |
| Parakeet v3, Q8 | 740 | 25 European languages, including all six interface languages |
| Whisper Tiny, Q8 | 46 | Multilingual |
| Whisper Base, Q8 | 85 | Multilingual |
| Whisper Small, Q8 | 270 | Multilingual |
| Whisper Large v3, Q5 | 1,161 | Multilingual |

Download size is not a RAM requirement. Native inference needs additional working memory, and speed/accuracy depend on the model, language, recording and hardware. The exact language lists and immutable artifact metadata live in the catalog.

## Architecture

- `src-tauri/src/local_models/catalog.json` is the model registry: stable ID, engine family, languages, quantized artifact URL, exact byte size, SHA-256 and attribution.
- `local_models/mod.rs` manages model storage and downloads. IPC accepts catalog IDs, never arbitrary URLs or destination paths. Artifacts are streamed into temporary files, checked for exact size and SHA-256, and renamed only after verification. Cancelled, failed or interrupted transfers never become selectable models.
- `local_models/runtime.rs` loads the native engine on demand on a blocking worker, serializes inference and retains at most one loaded model. It validates files before native loading and normalizes model-specific language and window behavior. The model is released after two idle minutes; the next recording loads it again. Quit and restart stop new work, cancel active local jobs and drain native resources before process termination. Recording and queued-file jobs retain their selected route and model even if settings change before they finish.
- `transcription/local.rs` implements the same provider interface as hosted adapters. File imports are decoded locally to 16 kHz mono PCM WAV before inference; originals are preserved.
- `local_model_commands.rs` exposes catalog, download, cancel and delete operations. The UI receives `local-model-progress` events and obtains a fresh list after opening.

Model files live in the application's data directory under `models/`, outside source control and outside the application bundle. Removing or updating the application bundle does not require downloading models again. Only one model download runs at a time.

Only one local transcription runs at a time. An overlapping recording or separately submitted file batch reports that the model is busy; it does not silently switch providers. Original imported files remain available, and failed voice transcriptions retain their existing retry path.

The native engine is `transcribe-cpp` 0.2.3, built from pinned Cargo dependencies. On Mac it includes Metal support and the embedded Metal library. End users do not need Python, CMake, Homebrew or an external server. Mac developers need CMake and Xcode Command Line Tools to compile the native dependency. Windows builds use the static CPU backend and the toolchain helper in `scripts/windows-native-toolchain.ps1`: MSVC/Windows SDK, CMake and Ninja, with Clang for ARM64. The x64 build disables host-specific SIMD requirements. Packaged users do not need a Python environment or separate model server.

Canary and Parakeet use the CPU backend, which performed better for short recordings on the tested M4 Max. On macOS, Whisper uses automatic backend selection with Metal acceleration and a CPU fallback. All six models use the CPU backend on Windows. Loading and checking a model takes longer than a subsequent recording; hardware-specific measurements are recorded separately from general compatibility claims.

`src-tauri/build.rs` links the selected Apple toolchain's compiler runtime so native availability checks work with the declared macOS 12 deployment target. It discovers the archive through `xcrun`, without embedding a developer-specific path in source.

`.cargo/config.toml` disables build-host-specific CPU tuning. The Mac release script enforces `GGML_NATIVE=OFF` so its CPU fallback is not restricted to the builder's processor features. Hardware validation still needs representative supported Macs; a portable compilation flag is not a performance or physical-hardware test.

## Adding a model

1. Verify that the pinned native runtime supports the model's architecture and artifact format. A new model in an existing supported family is a catalog change; a new family may require an engine update or adapter.
2. Review the model and conversion licenses. Preserve required attribution. Obtain a GGUF artifact from its verified source, pin its full immutable repository revision, and independently verify its SHA-256 and byte size.
3. Add a new stable catalog ID and filename. Do not reuse an installed ID/filename for different model bytes; existing downloads must not change meaning after an app update.
4. Declare the exact languages, language-selection requirement and engine family. Add the description to all six dictionaries in `ui/i18n.js`. The UI builds compact rows from catalog metadata; no new row markup or picker logic is needed.
5. Run catalog/download regressions, real English and Spanish audio where supported, silence, longer-than-one-window audio, cancellation, switching/deleting models and a restart with the model already downloaded. Record native hardware and actual results.
6. If introducing a new engine family, update the catalog validator, native adapter and its capability tests. Verify bundled dependencies, licenses, signing and notarization again before distribution.

Catalog changes ship with an app update. There is no remote catalog that can silently introduce new code or alter a pinned model.

## Model and runtime attribution

- Native runtime: [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp), MIT; includes [ggml](https://github.com/ggml-org/ggml), MIT. Their license notices accompany the app.
- [NVIDIA Canary 180M Flash](https://huggingface.co/nvidia/canary-180m-flash) and [NVIDIA Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3): CC BY 4.0. The downloaded artifacts are third-party GGUF conversions/quantizations, not original NVIDIA checkpoints.
- [OpenAI Whisper](https://github.com/openai/whisper): MIT. The selected GGUF conversion repositories declare Apache 2.0. Each catalog entry includes both the original source and the exact conversion revision; preserve both attributions and the conversion license.

The app shows model source and license information. Source links and license notices identify dependencies and model authors; they do not imply endorsement.

## Verification

See [Testing](TESTING.md) and [Windows verification](WINDOWS_BUILD_REPORT.md) for actual build and execution records. Offline request-contract tests for cloud adapters are not evidence of a live paid account. Native local inference tests use downloaded models and the licensed committed speech fixture; they require explicit opt-in because models are large. Historical Windows tests from 0.5.1 do not validate the local runtime; 1.0.0 has a separate verification record.

```sh
DICTAMELO_LOCAL_TESTS=1 \
DICTAMELO_LOCAL_TEST_DIR="$PWD/dist/local-model-release/models" \
cargo test --release --manifest-path src-tauri/Cargo.toml \
  local_models_transcribe_the_licensed_english_fixture -- --ignored --nocapture

DICTAMELO_LOCAL_TESTS=1 \
DICTAMELO_LOCAL_TEST_DIR="$PWD/dist/local-model-release/models" \
cargo test --release --manifest-path src-tauri/Cargo.toml \
  local_models_handle_silence_and_bounded_window_audio -- --ignored --nocapture
```

The first command downloads any missing catalog artifacts; the second requires all six already downloaded. Run them sequentially because model storage is exclusively locked. The windowing test generates silence and repeated licensed speech in the ignored test directory. For another language, set `DICTAMELO_LOCAL_TEST_AUDIO`, `DICTAMELO_LOCAL_TEST_LANGUAGE`, `DICTAMELO_LOCAL_TEST_EXPECTED` (comma-separated words) and `DICTAMELO_LOCAL_TEST_REPORT`. Keyword assertions verify integration, not perfect transcription accuracy.
