//! Sonidos de aviso con los sonidos del sistema (sin assets propios).

use super::super::SoundKind;
use objc2::rc::Retained;
use objc2_app_kit::NSSound;
use objc2_foundation::NSString;
use std::cell::RefCell;
use std::collections::HashMap;
use tauri::AppHandle;

thread_local! {
    // Solo se toca desde el hilo principal (ver `play_sound`): así los NSSound viven lo suficiente.
    static SOUNDS: RefCell<HashMap<&'static str, Retained<NSSound>>> = RefCell::new(HashMap::new());
}

/// Reproduce un sonido corto del sistema, bajito, sin bloquear.
pub fn play_sound(app: &AppHandle, kind: SoundKind) {
    let (name, volume) = match kind {
        SoundKind::Start => ("Pop", 0.35f32),
        SoundKind::Stop => ("Tink", 0.3),
        SoundKind::Error => ("Basso", 0.3),
    };
    let _ = app.run_on_main_thread(move || {
        SOUNDS.with(|cache| {
            let mut cache = cache.borrow_mut();
            if !cache.contains_key(name) {
                match NSSound::soundNamed(&NSString::from_str(name)) {
                    Some(sound) => {
                        cache.insert(name, sound);
                    }
                    None => {
                        log::warn!("No existe el sonido del sistema «{name}»");
                        return;
                    }
                }
            }
            if let Some(sound) = cache.get(name) {
                sound.setVolume(volume);
                sound.play();
            }
        });
    });
}
