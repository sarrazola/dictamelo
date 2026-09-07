//! Autodiagnóstico sin micrófono ni atajo: con `DICTAMELO_SELFTEST_WAV=/ruta/audio.wav` la app
//! transcribe ese archivo con la configuración actual, lo pega/copia como en el flujo normal,
//! imprime el resultado y termina (código 0 si todo fue bien). Útil para probar la integración
//! con el proveedor, el portapapeles y el historial de forma automatizada.

use crate::audio::{self, RawRecording};
use crate::pipeline;
use crate::status::Status;
use std::path::Path;
use std::time::Duration;
use tauri::AppHandle;

pub fn enabled() -> bool {
    std::env::var_os("DICTAMELO_SELFTEST_WAV").is_some()
        || std::env::var_os("DICTAMELO_SELFTEST_HOTKEY_SECS").is_some()
        || std::env::var_os("DICTAMELO_SELFTEST_FILE").is_some()
        || std::env::var_os("DICTAMELO_SELFTEST_RESTART_MARKER").is_some()
}

pub fn maybe_run(app: &AppHandle) {
    let wav = std::env::var("DICTAMELO_SELFTEST_WAV").ok();
    let hotkey_secs = std::env::var("DICTAMELO_SELFTEST_HOTKEY_SECS").ok().and_then(|s| s.parse::<f64>().ok());
    let file = std::env::var("DICTAMELO_SELFTEST_FILE").ok();
    let restart_marker = std::env::var_os("DICTAMELO_SELFTEST_RESTART_MARKER").map(std::path::PathBuf::from);
    if wav.is_none() && hotkey_secs.is_none() && file.is_none() && restart_marker.is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Deja que bandeja, ventanas y atajo terminen de registrarse.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if let Some(marker) = restart_marker {
            let result = if wav.is_some() || hotkey_secs.is_some() {
                Err("Restart selftest only supports DICTAMELO_SELFTEST_FILE".into())
            } else if let Some(path) = file.as_deref() {
                run_restart_file(&app, Path::new(path), &marker).await
            } else {
                Err("Restart selftest requires DICTAMELO_SELFTEST_FILE".into())
            };
            match result {
                Ok(RestartOutcome::Requested) => {
                    // The requested restart normally ends this process immediately.
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    log::error!("SELFTEST_RESTART_FAIL restart did not terminate the original process");
                    app.exit(1);
                }
                Ok(RestartOutcome::Completed) => app.exit(0),
                Err(error) => {
                    eprintln!("SELFTEST_RESTART_FAIL {error}");
                    log::error!("SELFTEST_RESTART_FAIL {error}");
                    app.exit(1);
                }
            }
            return;
        }
        let result = match (wav, hotkey_secs, file) {
            (Some(path), _, _) => run(&app, Path::new(&path)).await,
            (None, Some(secs), _) => run_with_hotkey(&app, secs).await,
            (None, None, Some(path)) => run_file(&app, Path::new(&path)).await,
            (None, None, None) => unreachable!(),
        };
        match &result {
            Ok(text) => {
                println!("SELFTEST_OK {text}");
                log::info!("SELFTEST_OK {text}");
            }
            Err(e) => {
                eprintln!("SELFTEST_FAIL {e}");
                log::error!("SELFTEST_FAIL {e}");
            }
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
        app.exit(if result.is_ok() { 0 } else { 1 });
    });
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestartRequest {
    schema_version: u32,
    parent_pid: u32,
    fixture: std::path::PathBuf,
    model: String,
}

enum RestartOutcome { Requested, Completed }

fn evidence_path(marker: &Path, suffix: &str) -> Result<std::path::PathBuf, String> {
    let mut name = marker.file_name().ok_or("Restart marker needs a filename")?.to_os_string();
    name.push(suffix);
    Ok(marker.with_file_name(name))
}

fn write_new_evidence(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    use std::io::Write;
    let data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("Could not create new restart evidence: {error}"))?;
    file.write_all(&data).and_then(|_| file.sync_all()).map_err(|error| error.to_string())
}

fn restart_request(marker: &Path, fixture: &Path, model: &str, pid: u32) -> Result<Option<RestartRequest>, String> {
    if !marker.is_absolute() || !marker.parent().is_some_and(Path::is_dir) {
        return Err("Restart marker must have an absolute path in an existing test directory".into());
    }
    let metadata = match std::fs::symlink_metadata(marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 {
        return Err("Restart request must be a small regular evidence file".into());
    }
    let request: RestartRequest = serde_json::from_slice(&std::fs::read(marker).map_err(|error| error.to_string())?)
        .map_err(|_| "Invalid restart request evidence")?;
    if request.schema_version != 1 || request.parent_pid == pid || request.fixture != fixture || request.model != model {
        return Err("Restart request does not match a new process with the same local fixture and model".into());
    }
    Ok(Some(request))
}

/// Opt-in packaged restart probe. Use a fresh marker under the ignored `dist/` test directory.
/// The immutable request, child claim, and completion files prevent replay or restart loops.
async fn run_restart_file(app: &AppHandle, fixture: &Path, marker: &Path) -> Result<RestartOutcome, String> {
    use tauri::Manager;
    let settings = app.state::<crate::state::AppState>().settings();
    if !settings.uses_local_transcription() || settings.should_clean_transcript() {
        return Err("Restart selftest requires a local model with cloud cleanup disabled".into());
    }
    let fixture = fixture.canonicalize().map_err(|error| error.to_string())?;
    let pid = std::process::id();
    let request = restart_request(marker, &fixture, &settings.model, pid)?;
    let completed = evidence_path(marker, ".complete.json")?;
    let claimed = evidence_path(marker, ".child.json")?;
    if completed.try_exists().map_err(|error| error.to_string())? {
        return Err("Restart completion already exists; choose a fresh marker path".into());
    }
    if request.is_some() {
        write_new_evidence(&claimed, &serde_json::json!({"childPid": pid}))?;
    } else if claimed.try_exists().map_err(|error| error.to_string())? {
        return Err("Restart evidence already exists; choose a fresh marker path".into());
    }
    let text = run_file(app, &fixture).await?;
    if let Some(request) = request {
        write_new_evidence(&completed, &serde_json::json!({
            "schemaVersion": 1, "parentPid": request.parent_pid, "childPid": pid,
            "fixture": fixture, "model": settings.model, "transcript": text,
            "restartCommand": "commands::restart_app", "status": "completed"
        }))?;
        println!("SELFTEST_RESTART_OK parent_pid={} child_pid={pid}", request.parent_pid);
        log::info!("SELFTEST_RESTART_OK parent_pid={} child_pid={pid}", request.parent_pid);
        Ok(RestartOutcome::Completed)
    } else {
        write_new_evidence(marker, &RestartRequest { schema_version: 1, parent_pid: pid, fixture, model: settings.model })?;
        println!("SELFTEST_RESTART_REQUESTED parent_pid={pid}");
        log::info!("SELFTEST_RESTART_REQUESTED parent_pid={pid}");
        crate::commands::restart_app(app.clone());
        Ok(RestartOutcome::Requested)
    }
}

/// Flujo real completo: la app pulsa su propio atajo (eventos sintéticos, requiere Accesibilidad),
/// lo mantiene `hold_secs` segundos grabando del micrófono configurado, lo suelta y deja que el
/// pipeline transcriba y pegue. Devuelve el texto transcrito.
async fn run_with_hotkey(app: &AppHandle, hold_secs: f64) -> Result<String, String> {
    use crate::state::AppState;
    use crate::util::lock;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use tauri::Manager;

    let hotkey = app.state::<AppState>().settings().hotkey;
    log::info!("Selftest: pulsando el atajo «{hotkey}» durante {hold_secs:.1}s");
    // La pulsación se mantiene en otro hilo mientras aquí observamos el estado del pipeline.
    let press_result: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
    let slot = press_result.clone();
    let hk = hotkey.clone();
    std::thread::spawn(move || {
        let result = crate::platform::press_hotkey_for_test(&hk, Duration::from_secs_f64(hold_secs)).map_err(|e| e.to_string());
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
    });
    // Opcional: pulsar Esc a mitad de la grabación para probar la cancelación.
    if let Some(ms) = std::env::var("DICTAMELO_SELFTEST_ESC_AFTER_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            log::info!("Selftest: pulsando Esc");
            if let Err(e) = crate::platform::press_hotkey_for_test("Escape", Duration::from_millis(40)) {
                log::warn!("Selftest: no se pudo pulsar Esc: {e}");
            }
        });
    }

    let deadline = Instant::now() + Duration::from_secs(90);
    let mut started = false;
    loop {
        if let Some(Err(e)) = press_result.lock().unwrap_or_else(|e| e.into_inner()).take() {
            return Err(format!("no se pudo pulsar el atajo: {e}"));
        }
        let status = pipeline::current_status(app);
        match &status {
            Status::Recording | Status::Transcribing | Status::Cleaning | Status::Pasting => started = true,
            Status::Done { message } if started => {
                let text = lock(&app.state::<AppState>().history).entries().first().map(|e| e.text.clone()).unwrap_or_default();
                log::info!("Selftest terminado: {message}");
                return Ok(format!("{text} [{message}]"));
            }
            Status::Error { message } if started => return Err(message.clone()),
            Status::Error { message } => return Err(format!("antes de grabar: {message}")),
            _ => {}
        }
        if Instant::now() > deadline {
            return Err(if started { "el pipeline no terminó a tiempo".into() } else { "el atajo nunca llegó a la app".into() });
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Modo WAV: transcribe un archivo existente con el pipeline normal (sin micrófono ni atajo).
async fn run(app: &AppHandle, path: &Path) -> Result<String, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|v| v as f32 / max)).collect::<Result<_, _>>()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
    }
    .map_err(|e| format!("WAV inválido: {e}"))?;
    let raw = RawRecording { samples, sample_rate: spec.sample_rate, channels: spec.channels };
    let prepared = audio::prepare(&raw);
    log::info!("Selftest: {:.2}s de audio desde {}", prepared.duration_secs(), path.display());

    pipeline::set_status(app, Status::Transcribing);
    match pipeline::transcribe_and_deliver(app, prepared).await {
        Some(text) => Ok(text),
        None => Err(pipeline::current_status(app).label("en")),
    }
}

/// Modo archivo: encola un audio como si lo hubieran arrastrado y espera el resultado.
async fn run_file(app: &AppHandle, path: &Path) -> Result<String, String> {
    use crate::file_transcription::{self, Stage};
    use crate::state::AppState;
    use crate::util::lock;
    use std::time::Instant;
    use tauri::Manager;

    file_transcription::enqueue(app, vec![path.to_path_buf()]);
    let deadline = Instant::now() + Duration::from_secs(900);
    loop {
        let job = lock(&app.state::<AppState>().file_jobs).first().cloned();
        match job {
            Some(job) if job.stage == Stage::Done => {
                return Ok(format!("{} [{} tramo(s), {:.0}s]", job.text, job.chunks, job.duration_secs));
            }
            Some(job) if job.stage == Stage::Failed => return Err(job.error.unwrap_or_default()),
            None => return Err("el trabajo desapareció de la cola".into()),
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("tiempo de espera agotado".into());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod restart_tests {
    use super::*;

    #[test]
    fn restart_evidence_requires_a_new_matching_process_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!("dictamelo-restart-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let marker = root.join("request.json");
        let fixture = root.join("fixture.wav");
        assert!(restart_request(&marker, &fixture, "whisper-base", 100).unwrap().is_none());
        let request = RestartRequest { schema_version: 1, parent_pid: 100, fixture: fixture.clone(), model: "whisper-base".into() };
        write_new_evidence(&marker, &request).unwrap();
        let original = std::fs::read(&marker).unwrap();
        assert!(write_new_evidence(&marker, &serde_json::json!({"overwrite": true})).is_err());
        assert_eq!(std::fs::read(&marker).unwrap(), original);
        assert!(restart_request(&marker, &fixture, "whisper-base", 100).is_err());
        assert!(restart_request(&marker, &fixture, "parakeet-v3", 200).is_err());
        assert!(restart_request(&marker, &root.join("different.wav"), "whisper-base", 200).is_err());
        assert_eq!(restart_request(&marker, &fixture, "whisper-base", 200).unwrap().unwrap().parent_pid, 100);
        let claimed = evidence_path(&marker, ".child.json").unwrap();
        write_new_evidence(&claimed, &serde_json::json!({"childPid": 200})).unwrap();
        assert!(write_new_evidence(&claimed, &serde_json::json!({"childPid": 300})).is_err());
        assert_eq!(std::fs::read(&marker).unwrap(), original);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restart_rejects_invalid_and_oversized_evidence() {
        let root = std::env::temp_dir().join(format!("dictamelo-restart-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let marker = root.join("request.json");
        std::fs::write(&marker, b"not a restart request").unwrap();
        assert!(restart_request(&marker, &root.join("fixture.wav"), "whisper-base", 200).is_err());
        std::fs::write(&marker, vec![b'x'; 16 * 1024 + 1]).unwrap();
        assert!(restart_request(&marker, &root.join("fixture.wav"), "whisper-base", 200).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
