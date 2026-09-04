//! Implementación macOS: TCC (micrófono/accesibilidad), CGEvent (⌘V), NSPasteboard y NSWindow.

mod audio_decode;
mod clipboard;
mod locale;
mod keyboard;
mod permissions;
mod sound;
mod window;

pub use audio_decode::decode_audio_to_wav;
pub use clipboard::clipboard_backend;
pub use locale::system_language;
pub use sound::play_sound;
pub use keyboard::{install_cancel_key_monitor, press_hotkey_for_test, send_paste_keystroke};
pub use permissions::{
    open_permission_settings, permissions_status, request_accessibility_permission,
    request_microphone_permission,
};
pub use window::{activate_app, configure_overlay_window, hide_window, refresh_window_shadow, show_window_without_focus};
