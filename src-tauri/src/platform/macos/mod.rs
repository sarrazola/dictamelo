//! Implementación macOS: TCC (micrófono/accesibilidad), CGEvent (⌘V), NSPasteboard y NSWindow.

mod clipboard;
mod keyboard;
mod permissions;
mod window;

pub use clipboard::clipboard_backend;
pub use keyboard::{press_hotkey_for_test, send_paste_keystroke};
pub use permissions::{
    open_permission_settings, permissions_status, request_accessibility_permission,
    request_microphone_permission,
};
pub use window::{activate_app, configure_overlay_window, hide_window, show_window_without_focus};
