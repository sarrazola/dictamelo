//! Dictámelo: app de barra de menú para dictar por voz en cualquier aplicación.
//!
//! Toda la lógica vive en Rust; el frontend (carpeta `ui/`) solo pinta la interfaz.
//!
//! Módulos:
//! - `audio`         captura del micrófono (cpal), remuestreo a 16 kHz y WAV temporal.
//! - `transcription` interfaz `TranscriptionProvider` + implementaciones (Groq, OpenAI).
//! - `clipboard` / `paste` inserción del texto conservando el portapapeles.
//! - `platform`      TODO lo específico del sistema operativo (permisos, teclado,
//!                   portapapeles nativo, ventanas). Aquí se añadirá `windows/` después.
//! - `pipeline`      máquina de estados: grabar → transcribir → pegar, con recuperación.
//! - `hotkey`, `tray`, `app_windows`, `commands`: integración con Tauri.

mod app_windows;
mod audio;
mod autostart;
mod cleanup;
mod file_transcription;
mod clipboard;
mod commands;
mod history;
mod i18n;
mod hotkey;
mod paste;
mod pipeline;
mod platform;
mod secrets;
mod selftest;
mod settings;
mod state;
mod status;
mod transcription;
mod tray;
mod util;

use tauri::{Manager, WindowEvent};

pub fn run() {
    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log_level)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("rustls", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("dictamelo".into()),
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin({
            let autostart = tauri_plugin_autostart::Builder::new();
            // `macos_launcher` solo existe en macOS; en Windows el plugin usa la clave Run del registro.
            #[cfg(target_os = "macos")]
            let autostart = autostart.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);
            autostart.build()
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_status,
            commands::get_providers,
            commands::get_cleaners,
            commands::get_api_key_status,
            commands::set_api_key,
            commands::delete_api_key,
            commands::get_permissions,
            commands::request_microphone_permission,
            commands::request_accessibility_permission,
            commands::open_permission_settings,
            commands::get_history,
            commands::delete_history_entry,
            commands::clear_history,
            commands::copy_history_entry,
            commands::list_input_devices,
            commands::validate_hotkey,
            commands::begin_hotkey_capture,
            commands::end_hotkey_capture,
            commands::get_app_info,
            commands::open_log_dir,
            commands::retry_last_transcription,
            commands::open_url,
            commands::ui_ready,
            commands::overlay_layout,
            commands::transcribe_files,
            commands::pick_audio_files,
            commands::get_file_jobs,
            commands::remove_file_job,
            commands::clear_file_jobs,
            commands::copy_file_transcript,
            commands::save_file_transcript,
        ])
        .setup(|app| {
            // App de barra de menú: sin ícono en el Dock ni menú de aplicación.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let state = state::AppState::init(&handle)?;
            app.manage(state);

            app_windows::create_windows(&handle)?;
            tray::create(&handle)?;
            hotkey::apply_from_settings(&handle);
            autostart::sync_with_settings(&handle);
            let esc_handle = handle.clone();
            platform::install_cancel_key_monitor(std::sync::Arc::new(move || pipeline::cancel_recording(&esc_handle)));
            if selftest::enabled() {
                selftest::maybe_run(&handle);
            } else {
                pipeline::startup_checks(&handle);
            }
            log::info!("Dictámelo {} iniciado", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Cerrar la ventana de configuración solo la oculta: la app sigue en la barra de menú.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == app_windows::MAIN {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar Dictámelo")
        .run(|_app, event| {
            // Sin ventanas visibles la app debe seguir viva (vive en la barra de menú).
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
