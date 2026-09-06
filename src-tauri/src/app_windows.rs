//! Ventanas de la app: configuración (`main`) e indicador flotante (`overlay`).

use crate::platform;
use crate::state::AppState;
use crate::status::Status;
use crate::pipeline;
use std::time::Duration;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const OVERLAY: &str = "overlay";

/// Tamaño inicial; después la interfaz pide el ancho exacto de su contenido (`layout_overlay`).
const OVERLAY_WIDTH: f64 = 220.0;
const OVERLAY_HEIGHT: f64 = 48.0;
const OVERLAY_BOTTOM_MARGIN: f64 = 72.0;
/// Si la interfaz no responde con su tamaño (p. ej. webview aún cargando), se muestra igual.
const OVERLAY_FALLBACK: Duration = Duration::from_millis(300);

pub fn create_windows(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Dictámelo")
        .inner_size(1080.0, 760.0)
        .min_inner_size(960.0, 680.0)
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
    #[cfg(target_os = "macos")]
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // An explicitly opened settings window behaves like a normal Mac app:
        // keep it in the Dock and application switcher until the user closes it.
        #[cfg(target_os = "macos")]
        {
            let _ = handle.set_activation_policy(tauri::ActivationPolicy::Regular);
            if let Err(error) = platform::configure_settings_window(&window) {
                log::warn!("Could not configure the settings window: {error}");
            }
        }
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        platform::activate_app();
    });
}

/// Closing settings returns to the menu bar without quitting dictation.
/// Losing focus or minimizing must never call this function.
pub fn hide_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else { return };
    #[cfg(target_os = "macos")]
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(error) = window.hide() {
            log::warn!("Could not hide the settings window: {error}");
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
    });
}

/// ¿Debería verse el indicador ahora mismo?
fn overlay_wanted(app: &AppHandle) -> bool {
    app.state::<AppState>().settings().show_overlay && !matches!(pipeline::current_status(app), Status::Idle)
}

pub fn update_overlay(app: &AppHandle, status: &Status) {
    if !overlay_wanted(app) || matches!(status, Status::Idle) {
        hide_overlay(app);
        return;
    }
    // La interfaz mide su texto y llama a `layout_overlay`, que muestra la ventana ya con el
    // tamaño correcto (sin saltos). Por si no llega, se muestra igualmente pasado un momento.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(OVERLAY_FALLBACK).await;
        if overlay_wanted(&app) {
            show_overlay_window(&app);
        }
    });
}

/// Llamado por la interfaz del indicador con el tamaño lógico de su contenido.
pub fn layout_overlay(app: &AppHandle, width: f64, height: f64) {
    let Some(window) = app.get_webview_window(OVERLAY) else { return };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = window.set_size(LogicalSize::new(width.clamp(120.0, 560.0), height.clamp(36.0, 96.0)));
        position_overlay(&handle, &window);
        platform::refresh_window_shadow(&window);
        if overlay_wanted(&handle) && !window.is_visible().unwrap_or(false) {
            if let Err(e) = platform::show_window_without_focus(&window) {
                log::warn!("No se pudo mostrar el indicador: {e}");
            }
        }
    });
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
        if window.is_visible().unwrap_or(false) {
            return;
        }
        position_overlay(&handle, &window);
        if let Err(e) = platform::show_window_without_focus(&window) {
            log::warn!("No se pudo mostrar el indicador: {e}");
        }
        platform::refresh_window_shadow(&window);
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
    let (width, height) = match window.outer_size() {
        Ok(size) if size.width > 0 => (size.width as f64, size.height as f64),
        _ => (OVERLAY_WIDTH * scale, OVERLAY_HEIGHT * scale),
    };
    let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64 - height - OVERLAY_BOTTOM_MARGIN * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}
