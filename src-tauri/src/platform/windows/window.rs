//! Ajustes de ventana que Tauri no expone: mostrar el indicador flotante sin activarlo (la app
//! donde está el cursor no debe perder el foco) y estilos extendidos de la ventana.

use super::super::PlatformError;
use tauri::WebviewWindow;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, IsWindowVisible, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

fn hwnd(window: &WebviewWindow) -> Result<HWND, PlatformError> {
    // Tauri devuelve el HWND con otra versión del crate `windows`; el valor es el mismo puntero.
    let handle = window.hwnd().map_err(|e| PlatformError::Other(e.to_string()))?;
    if handle.0.is_null() {
        return Err(PlatformError::Other("HWND nulo".into()));
    }
    Ok(HWND(handle.0))
}

/// El indicador flotante: nunca se activa (WS_EX_NOACTIVATE), no aparece en Alt+Tab
/// (WS_EX_TOOLWINDOW) y queda por encima de todo.
pub fn configure_overlay_window(window: &WebviewWindow) -> Result<(), PlatformError> {
    let hwnd = hwnd(window)?;
    // SAFETY: HWND válido de una ventana de este proceso.
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = current | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
        if wanted != current {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
        }
        // `set_ignore_cursor_events(true)` (clics que atraviesan) convierte la ventana en «layered»;
        // una ventana así no se dibuja hasta que se fijan sus atributos, aunque sea con opacidad total.
        if wanted & WS_EX_LAYERED.0 as isize != 0 {
            if let Err(e) = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) {
                log::warn!("No se pudieron fijar los atributos de la ventana flotante: {}", e.message().trim());
            }
        }
    }
    Ok(())
}

/// En Windows la sombra la dibuja DWM y no hace falta recalcularla.
pub fn refresh_window_shadow(_window: &WebviewWindow) {}

/// Muestra la ventana sin activarla ni robar el foco.
pub fn show_window_without_focus(window: &WebviewWindow) -> Result<(), PlatformError> {
    let hwnd = hwnd(window)?;
    // SAFETY: HWND válido; solo cambia visibilidad y orden Z.
    unsafe {
        SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW)
            .map_err(|e| PlatformError::Other(e.message()))?;
        log::debug!("Indicador flotante mostrado (visible: {})", IsWindowVisible(hwnd).as_bool());
    }
    Ok(())
}

pub fn hide_window(window: &WebviewWindow) -> Result<(), PlatformError> {
    let hwnd = hwnd(window)?;
    // SAFETY: HWND válido.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    Ok(())
}

/// En Windows basta con `set_focus` de la ventana de configuración (ver `app_windows`).
pub fn activate_app() {}
