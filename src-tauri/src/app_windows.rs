//! Ventanas de la app: configuración (`main`) e indicador flotante (`overlay`).

use crate::platform;
use crate::state::AppState;
use crate::status::Status;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const OVERLAY: &str = "overlay";

const OVERLAY_WIDTH: f64 = 320.0;
const OVERLAY_HEIGHT: f64 = 64.0;
const OVERLAY_BOTTOM_MARGIN: f64 = 72.0;

pub fn create_windows(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Dictado")
        .inner_size(660.0, 780.0)
        .min_inner_size(560.0, 520.0)
        .visible(false)
        .resizable(true)
        .build()?;

    let overlay = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("overlay.html".into()))
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .accept_first_mouse(false)
        .visible_on_all_workspaces(true)
        .build()?;
    let _ = overlay.set_ignore_cursor_events(true);
    let window = overlay.clone();
    app.run_on_main_thread(move || {
        if let Err(e) = platform::configure_overlay_window(&window) {
            log::warn!("No se pudo configurar el indicador flotante: {e}");
        }
    })?;
    Ok(())
}

/// Muestra la ventana de configuración y trae la app al frente.
pub fn show_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else { return };
    let _ = app.run_on_main_thread(move || {
        let _ = window.show();
        let _ = window.set_focus();
        platform::activate_app();
    });
}

pub fn update_overlay(app: &AppHandle, status: &Status) {
    let show_overlay = app.state::<AppState>().settings().show_overlay;
    if !show_overlay || matches!(status, Status::Idle) {
        hide_overlay(app);
    } else {
        show_overlay_window(app);
    }
}

pub fn hide_overlay(app: &AppHandle) {
    let Some(window) = app.get_webview_window(OVERLAY) else { return };
    let _ = app.run_on_main_thread(move || {
        let _ = platform::hide_window(&window);
    });
}

fn show_overlay_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(OVERLAY) else { return };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        position_overlay(&handle, &window);
        if let Err(e) = platform::show_window_without_focus(&window) {
            log::warn!("No se pudo mostrar el indicador: {e}");
        }
    });
}

/// Centrado abajo en la pantalla donde está el cursor.
fn position_overlay(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = OVERLAY_WIDTH * scale;
    let height = OVERLAY_HEIGHT * scale;
    let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64 - height - OVERLAY_BOTTOM_MARGIN * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}
