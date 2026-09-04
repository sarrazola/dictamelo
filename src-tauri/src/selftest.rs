//! Autodiagnóstico sin micrófono ni atajo: con `DICTADO_SELFTEST_WAV=/ruta/audio.wav` la app
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
    std::env::var_os("DICTADO_SELFTEST_WAV").is_some() || std::env::var_os("DICTADO_SELFTEST_HOTKEY_SECS").is_some()
}

pub fn maybe_run(app: &AppHandle) {
    let wav = std::env::var("DICTADO_SELFTEST_WAV").ok();
    let hotkey_secs = std::env::var("DICTADO_SELFTEST_HOTKEY_SECS").ok().and_then(|s| s.parse::<f64>().ok());
    if wav.is_none() && hotkey_secs.is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Deja que bandeja, ventanas y atajo terminen de registrarse.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let result = match (wav, hotkey_secs) {
            (Some(path), _) => run(&app, Path::new(&path)).await,
            (None, Some(secs)) => run_with_hotkey(&app, secs).await,
            (None, None) => unreachable!(),
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

    let deadline = Instant::now() + Duration::from_secs(90);
    let mut started = false;
    loop {
        if let Some(Err(e)) = press_result.lock().unwrap_or_else(|e| e.into_inner()).take() {
            return Err(format!("no se pudo pulsar el atajo: {e}"));
        }
        let status = pipeline::current_status(app);
        match &status {
            Status::Recording | Status::Transcribing | Status::Pasting => started = true,
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
        None => Err(pipeline::current_status(app).label()),
    }
}
