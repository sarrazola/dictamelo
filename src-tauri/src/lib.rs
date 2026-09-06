//! Dictámelo: app de barra de menú para dictar por voz en cualquier aplicación.
//!
//! Toda la lógica vive en Rust; el frontend (carpeta `ui/`) solo pinta la interfaz.
//!
//! Módulos:
//! - `audio`         captura del micrófono (cpal), remuestreo a 16 kHz y WAV temporal.
//! - `transcription` interfaz `TranscriptionProvider` + implementaciones (Groq, OpenAI).
//! - `clipboard` / `paste` inserción del texto conservando el portapapeles.
//! - `platform`      TODO lo específico del sistema operativo (permisos, teclado,
//!   Native clipboard and windows on macOS and Windows.
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
mod license;
mod account;
mod cloud_config;
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
mod updates;
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
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            commands::get_account_status,
            commands::sign_up_account,
            commands::sign_in_account,
            commands::confirm_account_email,
            commands::request_password_reset,
            commands::resend_account_confirmation,
            commands::reset_account_password,
            commands::sign_in_with_google,
            commands::cancel_google_sign_in,
            commands::send_sign_in_code,
            commands::verify_sign_in_code,
            commands::sign_out_account,
            commands::get_license_status,
            commands::activate_license,
            commands::deactivate_license,
            commands::open_checkout,
            commands::check_for_updates,
            commands::install_update,
            commands::restart_app,
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
            // Start in the menu bar; opening settings enables the Dock/app switcher.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let state = state::AppState::init(&handle)?;
            // Capture before creating webviews: the UI persists onboarding_seen when
            // it presents setup, which must not race the native window's visibility.
            let first_run = !state.settings().onboarding_seen;
            app.manage(state);

            app_windows::create_windows(&handle)?;
            tray::create(&handle)?;
            hotkey::apply_from_settings(&handle);
            autostart::sync_with_settings(&handle);
            // Comprobación silenciosa de actualizaciones, sin estorbar el arranque.
            updates::check_on_startup(&handle);
            updates::maybe_selftest(&handle);
            // La licencia se comprueba aparte para no retrasar el arranque.
            let license_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = license_handle.state::<state::AppState>();
                let status = license::validate(state.secrets.clone()).await;
                if status.active {
                    log::info!("Plan Pro activo: la transcripción va por el servidor de Dictámelo");
                }
                *util::write(&state.license) = status;
            });
            let esc_handle = handle.clone();
            platform::install_cancel_key_monitor(std::sync::Arc::new(move || pipeline::cancel_recording(&esc_handle)));
            if selftest::enabled() {
                selftest::maybe_run(&handle);
            } else {
                pipeline::startup_checks(&handle, first_run);
            }
            log::info!("Dictámelo {} iniciado", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Cerrar la ventana de configuración solo la oculta: la app sigue en la barra de menú.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == app_windows::MAIN {
                    api.prevent_close();
                    app_windows::hide_settings(window.app_handle());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar Dictámelo")
        .run(|_app, event| {
            // Finder/open should bring the existing menu-bar application's settings back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                app_windows::show_settings(_app);
            }
            // Sin ventanas visibles la app debe seguir viva (vive en la barra de menú).
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
