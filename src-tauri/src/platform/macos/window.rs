//! Ajustes de ventana que Tauri no expone: mostrar sin robar el foco, nivel y comportamiento
//! del indicador flotante. Todas estas funciones deben ejecutarse en el hilo principal.

use super::super::PlatformError;
use objc2_app_kit::{NSApplication, NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};
use objc2_foundation::MainThreadMarker;
use tauri::WebviewWindow;

fn ns_window(window: &WebviewWindow) -> Result<&NSWindow, PlatformError> {
    let ptr = window.ns_window().map_err(|e| PlatformError::Other(e.to_string()))?;
    if ptr.is_null() {
        return Err(PlatformError::Other("NSWindow nulo".into()));
    }
    // SAFETY: Tauri garantiza que el puntero es un NSWindow vivo mientras exista la ventana,
    // y estas funciones solo se invocan desde el hilo principal.
    Ok(unsafe { &*(ptr as *const NSWindow) })
}

/// El indicador flotante: por encima de todo, en todos los escritorios y sin capturar el ratón.
pub fn configure_overlay_window(window: &WebviewWindow) -> Result<(), PlatformError> {
    let ns = ns_window(window)?;
    ns.setLevel(NSStatusWindowLevel);
    ns.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    ns.setIgnoresMouseEvents(true);
    ns.setHidesOnDeactivate(false);
    ns.setHasShadow(false);
    Ok(())
}

/// Muestra la ventana sin convertirla en ventana clave ni activar la app
/// (así la app donde está el cursor no pierde el foco).
pub fn show_window_without_focus(window: &WebviewWindow) -> Result<(), PlatformError> {
    ns_window(window)?.orderFrontRegardless();
    Ok(())
}

pub fn hide_window(window: &WebviewWindow) -> Result<(), PlatformError> {
    ns_window(window)?.orderOut(None);
    Ok(())
}

/// Trae la app al frente (para la ventana de configuración de una app de barra de menú).
pub fn activate_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}
