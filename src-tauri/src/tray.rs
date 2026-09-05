//! Ícono y menú de la barra de menú; refleja el estado actual y el idioma elegido.

use crate::i18n::{t, tf};
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
    hint_item: MenuItem<Wry>,
    open_item: MenuItem<Wry>,
    retry_item: MenuItem<Wry>,
    autopaste_item: CheckMenuItem<Wry>,
    quit_item: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let settings = app.state::<AppState>().settings();
    let lang = settings.ui_lang();

    let status_item = MenuItem::with_id(app, "status", t(&lang, "status.idle"), false, None::<&str>)?;
    let hint_item = MenuItem::with_id(
        app,
        "hint",
        tf(&lang, "tray.hint", &[("k", &pretty_hotkey(&settings.hotkey))]),
        false,
        None::<&str>,
    )?;
    let open_item = MenuItem::with_id(app, "open", t(&lang, "tray.settings"), true, None::<&str>)?;
    let retry_item = MenuItem::with_id(app, "retry", t(&lang, "tray.retry"), false, None::<&str>)?;
    let autopaste_item =
        CheckMenuItem::with_id(app, "autopaste", t(&lang, "tray.autopaste"), true, settings.auto_paste, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", t(&lang, "tray.quit"), true, None::<&str>)?;

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
        .tooltip("Dictámelo")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .build(app)?;

    app.manage(TrayHandles { tray, status_item, hint_item, open_item, retry_item, autopaste_item, quit_item });
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
    let lang = app.state::<AppState>().settings().ui_lang();
    let _ = handles.tray.set_icon(Some(icon_for(status)));
    let _ = handles.tray.set_icon_as_template(matches!(status, Status::Idle | Status::Done { .. }));
    let _ = handles.tray.set_title(title_for(status, &lang));
    let _ = handles.status_item.set_text(status.label(&lang));
}

/// Vuelve a escribir el menú tras cambiar el idioma o el atajo.
pub fn relabel(app: &AppHandle) {
    let Some(handles) = app.try_state::<TrayHandles>() else { return };
    let settings = app.state::<AppState>().settings();
    let lang = settings.ui_lang();
    let _ = handles.hint_item.set_text(tf(&lang, "tray.hint", &[("k", &pretty_hotkey(&settings.hotkey))]));
    let _ = handles.open_item.set_text(t(&lang, "tray.settings"));
    let _ = handles.retry_item.set_text(t(&lang, "tray.retry"));
    let _ = handles.autopaste_item.set_text(t(&lang, "tray.autopaste"));
    let _ = handles.quit_item.set_text(t(&lang, "tray.quit"));
    let _ = handles.status_item.set_text(pipeline::current_status(app).label(&lang));
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
        Status::Transcribing | Status::Cleaning => include_bytes!("../icons/tray/transcribing.png"),
        Status::Pasting => include_bytes!("../icons/tray/pasting.png"),
        Status::Error { .. } => include_bytes!("../icons/tray/error.png"),
    };
    crate::platform::tray_icon(bytes)
}

/// Texto junto al ícono. Solo se muestra mientras hay algo en curso.
fn title_for(status: &Status, lang: &str) -> Option<String> {
    match status {
        Status::Idle => None,
        Status::Recording => Some(t(lang, "status.recording").into()),
        Status::Transcribing => Some(t(lang, "status.transcribing").into()),
        Status::Cleaning => Some(t(lang, "status.cleaning").into()),
        Status::Pasting => Some(t(lang, "status.pasting").into()),
        Status::Done { .. } => Some("✓".into()),
        Status::Error { .. } => Some(t(lang, "status.error").into()),
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
            "space" => "Space".to_string(),
            other => other.trim_start_matches("key").trim_start_matches("digit").to_uppercase(),
        })
        .collect::<String>()
        .trim_end_matches('+')
        .to_string()
}
