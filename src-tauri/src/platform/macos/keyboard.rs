//! Envío de ⌘V mediante CGEvent (requiere permiso de Accesibilidad).

use super::super::PlatformError;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::thread::sleep;
use std::time::Duration;

const KEY_V: u16 = 9;
const KEY_COMMAND: u16 = 55;
const KEY_SHIFT: u16 = 56;
const KEY_OPTION: u16 = 58;
const KEY_CONTROL: u16 = 59;

pub fn send_paste_keystroke() -> Result<(), PlatformError> {
    if super::permissions::accessibility_state() != super::super::PermissionState::Granted {
        return Err(PlatformError::AccessibilityDenied);
    }
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PlatformError::Other("no se pudo crear el origen de eventos de teclado".into()))?;

    let post = |keycode: u16, down: bool, flags: CGEventFlags| -> Result<(), PlatformError> {
        let event = CGEvent::new_keyboard_event(source.clone(), keycode, down)
            .map_err(|_| PlatformError::Other("no se pudo crear el evento de teclado".into()))?;
        event.set_flags(flags);
        event.post(CGEventTapLocation::HID);
        Ok(())
    };

    post(KEY_COMMAND, true, CGEventFlags::CGEventFlagCommand)?;
    post(KEY_V, true, CGEventFlags::CGEventFlagCommand)?;
    sleep(Duration::from_millis(15));
    post(KEY_V, false, CGEventFlags::CGEventFlagCommand)?;
    post(KEY_COMMAND, false, CGEventFlags::CGEventFlagNull)?;
    Ok(())
}

/// Solo para el autodiagnóstico: mantiene pulsado el atajo `hotkey` (formato del plugin, p. ej.
/// "Alt+Shift+Space") durante `hold` usando eventos sintéticos, como lo haría una persona.
pub fn press_hotkey_for_test(hotkey: &str, hold: Duration) -> Result<(), PlatformError> {
    if super::permissions::accessibility_state() != super::super::PermissionState::Granted {
        return Err(PlatformError::AccessibilityDenied);
    }
    let mut modifiers: Vec<(u16, CGEventFlags)> = Vec::new();
    let mut key: Option<u16> = None;
    for part in hotkey.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "shift" => modifiers.push((KEY_SHIFT, CGEventFlags::CGEventFlagShift)),
            "alt" | "option" => modifiers.push((KEY_OPTION, CGEventFlags::CGEventFlagAlternate)),
            "control" | "ctrl" => modifiers.push((KEY_CONTROL, CGEventFlags::CGEventFlagControl)),
            "super" | "cmd" | "command" | "meta" => modifiers.push((KEY_COMMAND, CGEventFlags::CGEventFlagCommand)),
            other => key = Some(keycode_for(other).ok_or_else(|| PlatformError::Other(format!("tecla no soportada en el autodiagnóstico: {other}")))?),
        }
    }
    let key = key.ok_or_else(|| PlatformError::Other("el atajo no tiene tecla principal".into()))?;
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PlatformError::Other("no se pudo crear el origen de eventos de teclado".into()))?;
    let post = |keycode: u16, down: bool, flags: CGEventFlags| -> Result<(), PlatformError> {
        let event = CGEvent::new_keyboard_event(source.clone(), keycode, down)
            .map_err(|_| PlatformError::Other("no se pudo crear el evento de teclado".into()))?;
        event.set_flags(flags);
        event.post(CGEventTapLocation::HID);
        Ok(())
    };
    let mut flags = CGEventFlags::CGEventFlagNull;
    for (code, flag) in &modifiers {
        flags |= *flag;
        post(*code, true, flags)?;
        sleep(Duration::from_millis(20));
    }
    post(key, true, flags)?;
    sleep(hold);
    post(key, false, flags)?;
    sleep(Duration::from_millis(20));
    for (code, flag) in modifiers.iter().rev() {
        flags.remove(*flag);
        post(*code, false, flags)?;
        sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Códigos de tecla virtuales (distribución ANSI) para las teclas que acepta el plugin de atajos.
fn keycode_for(name: &str) -> Option<u16> {
    let code = match name {
        "space" => 49, "enter" | "return" => 36, "tab" => 48, "escape" | "esc" => 53, "backquote" => 50,
        "minus" => 27, "equal" => 24, "bracketleft" => 33, "bracketright" => 30, "backslash" => 42,
        "semicolon" => 41, "quote" => 39, "comma" => 43, "period" => 47, "slash" => 44,
        "keya" => 0, "keyb" => 11, "keyc" => 8, "keyd" => 2, "keye" => 14, "keyf" => 3, "keyg" => 5, "keyh" => 4,
        "keyi" => 34, "keyj" => 38, "keyk" => 40, "keyl" => 37, "keym" => 46, "keyn" => 45, "keyo" => 31, "keyp" => 35,
        "keyq" => 12, "keyr" => 15, "keys" => 1, "keyt" => 17, "keyu" => 32, "keyv" => 9, "keyw" => 13, "keyx" => 7,
        "keyy" => 16, "keyz" => 6,
        "digit0" => 29, "digit1" => 18, "digit2" => 19, "digit3" => 20, "digit4" => 21, "digit5" => 23, "digit6" => 22,
        "digit7" => 26, "digit8" => 28, "digit9" => 25,
        "f1" => 122, "f2" => 120, "f3" => 99, "f4" => 118, "f5" => 96, "f6" => 97, "f7" => 98, "f8" => 100, "f9" => 101,
        "f10" => 109, "f11" => 103, "f12" => 111, "f13" => 105, "f14" => 107, "f15" => 113, "f16" => 106, "f17" => 64,
        "f18" => 79, "f19" => 80,
        _ => return None,
    };
    Some(code)
}
