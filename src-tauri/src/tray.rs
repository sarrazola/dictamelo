//! Ícono y menú de la barra de menú; refleja el estado actual.

use crate::state::AppState;
use crate::status::Status;
use crate::util::write;
use crate::{app_windows, pipeline};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

pub struct TrayHandles {
    tray: tauri::tray::TrayIcon<Wry>,
    status_item: MenuItem<Wry>,
    retry_item: MenuItem<Wry>,
    autopaste_item: CheckMenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let settings = app.state::<AppState>().settings();

    let status_item = MenuItem::with_id(app, "status", "Listo", false, None::<&str>)?;
    let hint_item = MenuItem::with_id(app, "hint", format!("Mantén {} y habla", pretty_hotkey(&settings.hotkey)), false, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Configuración…", true, None::<&str>)?;
    let retry_item = MenuItem::with_id(app, "retry", "Reintentar última transcripción", false, None::<&str>)?;
    let autopaste_item = CheckMenuItem::with_id(app, "autopaste", "Pegado automático", true, settings.auto_paste, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Salir de Dictado", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &hint_item,
            &PredefinedMenuItem::separator(app)?,
            &open_item,
            &retry_item,
            &autopaste_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let tray = TrayIconBuilder::with_id("main")
        .icon(icon_for(&Status::Idle))
        .icon_as_template(true)
        .tooltip("Dictado")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .build(app)?;

    app.manage(TrayHandles { tray, status_item, retry_item, autopaste_item });
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => app_windows::show_settings(app),
        "retry" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { pipeline::retry_last(&app).await });
        }
        "autopaste" => {
            let state = app.state::<AppState>();
            let updated = {
                let mut settings = write(&state.settings);
                settings.auto_paste = !settings.auto_paste;
                settings.clone()
            };
            if let Err(e) = updated.save(&state.settings_path) {
                log::error!("No se pudo guardar la configuración: {e}");
            }
            set_autopaste_checked(app, updated.auto_paste);
            let _ = app.emit("settings-changed", &updated);
        }
        "quit" => {
            log::info!("Saliendo por petición del usuario");
            app.exit(0);
        }
        _ => {}
    }
}

pub fn update(app: &AppHandle, status: &Status) {
    let Some(handles) = app.try_state::<TrayHandles>() else { return };
    let _ = handles.tray.set_icon(Some(icon_for(status)));
    let _ = handles.tray.set_icon_as_template(matches!(status, Status::Idle | Status::Done { .. }));
    let _ = handles.tray.set_title(title_for(status));
    let _ = handles.status_item.set_text(status.label());
}

pub fn set_retry_enabled(app: &AppHandle, enabled: bool) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.retry_item.set_enabled(enabled);
    }
}

pub fn set_autopaste_checked(app: &AppHandle, checked: bool) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.autopaste_item.set_checked(checked);
    }
}

fn icon_for(status: &Status) -> Image<'static> {
    let bytes: &'static [u8] = match status {
        Status::Idle | Status::Done { .. } => include_bytes!("../icons/tray/idle.png"),
        Status::Recording => include_bytes!("../icons/tray/recording.png"),
        Status::Transcribing => include_bytes!("../icons/tray/transcribing.png"),
        Status::Pasting => include_bytes!("../icons/tray/pasting.png"),
        Status::Error { .. } => include_bytes!("../icons/tray/error.png"),
    };
    Image::from_bytes(bytes).expect("los íconos PNG embebidos son válidos")
}

fn title_for(status: &Status) -> Option<&'static str> {
    match status {
        Status::Idle => None,
        Status::Recording => Some("Grabando"),
        Status::Transcribing => Some("Transcribiendo…"),
        Status::Pasting => Some("Pegando…"),
        Status::Done { .. } => Some("✓"),
        Status::Error { .. } => Some("Error"),
    }
}

/// "Alt+Shift+Space" → "⌥⇧Espacio" (macOS) para el menú.
pub fn pretty_hotkey(hotkey: &str) -> String {
    hotkey
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "super" | "cmd" | "command" | "meta" => if cfg!(target_os = "macos") { "⌘" } else { "Win+" }.to_string(),
            "alt" | "option" => if cfg!(target_os = "macos") { "⌥" } else { "Alt+" }.to_string(),
            "control" | "ctrl" => if cfg!(target_os = "macos") { "⌃" } else { "Ctrl+" }.to_string(),
            "shift" => if cfg!(target_os = "macos") { "⇧" } else { "Shift+" }.to_string(),
            "space" => "Espacio".to_string(),
            other => other.trim_start_matches("key").trim_start_matches("digit").to_uppercase(),
        })
        .collect::<String>()
        .trim_end_matches('+')
        .to_string()
}
