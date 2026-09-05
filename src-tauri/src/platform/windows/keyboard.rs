//! Teclado: Ctrl+V sintético con `SendInput`, monitor global de Esc (sondeo del estado de la
//! tecla, sin hooks) y pulsación del atajo para el autodiagnóstico.

use super::super::PlatformError;
use std::sync::{Arc, OnceLock};
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_LMENU, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

/// Evento de teclado con código virtual **y** de escaneo: los controles de edición de Windows
/// (y muchas apps) ignoran las pulsaciones sintéticas que llegan sin código de escaneo.
fn key_event(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    // SAFETY: consulta pura de la distribución de teclado activa.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as u16;
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if matches!(vk, VK_LWIN | VK_RWIN | VK_RMENU | VK_RCONTROL) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn send(events: &[INPUT]) -> Result<(), PlatformError> {
    // SAFETY: `events` es un slice de estructuras INPUT completamente inicializadas.
    let sent = unsafe { SendInput(events, std::mem::size_of::<INPUT>() as i32) } as usize;
    if sent == events.len() {
        Ok(())
    } else {
        Err(PlatformError::Other(format!(
            "SendInput solo envió {sent} de {} eventos: {}",
            events.len(),
            std::io::Error::last_os_error()
        )))
    }
}

/// Estado asíncrono de una tecla: (pulsada ahora, pulsada en algún momento desde la última consulta).
fn key_state(vk: VIRTUAL_KEY) -> (bool, bool) {
    // SAFETY: consulta de solo lectura del estado del teclado.
    let state = unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16;
    (state & 0x8000 != 0, state & 0x0001 != 0)
}

fn is_pressed(vk: VIRTUAL_KEY) -> bool {
    key_state(vk).0
}

/// Título de la ventana en primer plano (solo para el registro de diagnóstico).
fn foreground_title() -> String {
    // SAFETY: lectura del título con un búfer válido.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return "(ninguna)".into();
        }
        let mut buf = [0u16; 128];
        let len = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
        String::from_utf16_lossy(&buf[..len])
    }
}

/// Modificadores que el sistema ve pulsados (solo para el registro de diagnóstico).
fn pressed_modifiers() -> String {
    let names = [
        (VK_SHIFT, "Shift"),
        (VK_CONTROL, "Ctrl"),
        (VK_MENU, "Alt"),
        (VK_LWIN, "Win"),
        (VK_RWIN, "Win(der)"),
    ];
    let pressed: Vec<&str> = names.iter().filter(|(vk, _)| is_pressed(*vk)).map(|(_, n)| *n).collect();
    if pressed.is_empty() {
        "ninguno".into()
    } else {
        pressed.join("+")
    }
}

/// Envía Ctrl+V a la ventana con el foco (la app donde está el cursor).
pub fn send_paste_keystroke() -> Result<(), PlatformError> {
    log::debug!(
        "Ctrl+V hacia la ventana en primer plano «{}» (modificadores pulsados: {})",
        foreground_title(),
        pressed_modifiers()
    );
    // Primero se sueltan Shift y Alt, siempre: el estado de teclado del hilo destino puede no
    // coincidir con el global, y con un Alt «colgado» el Ctrl se entrega como tecla de sistema y
    // la ventana entra en el modo de menú en vez de pegar. Win se suelta solo si el sistema lo ve
    // pulsado (p. ej. el atajo sigue apretado al alcanzar la duración máxima de grabación).
    let mut release: Vec<INPUT> =
        [VK_LSHIFT, VK_RSHIFT, VK_SHIFT, VK_LMENU, VK_RMENU, VK_MENU].into_iter().map(|vk| key_event(vk, true)).collect();
    for vk in [VK_LWIN, VK_RWIN] {
        if is_pressed(vk) {
            release.push(key_event(vk, true));
        }
    }
    send(&release)?;
    sleep(Duration::from_millis(20));
    send(&[key_event(VK_CONTROL, false), key_event(VK_V, false)])?;
    sleep(Duration::from_millis(15));
    send(&[key_event(VK_V, true), key_event(VK_CONTROL, true)])
}

static ON_ESCAPE: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Observa la tecla Esc en todo el sistema (sin consumirla) y avisa cada vez que se pulsa.
/// Se hace sondeando `GetAsyncKeyState` cada 25 ms en un hilo propio: no hace falta un hook
/// global de teclado (Windows lo retira en silencio si el hilo tarda en atenderlo, y los antivirus
/// lo miran con recelo) y las pulsaciones muy breves se detectan por el bit «pulsada desde la
/// última consulta». Se instala una sola vez; el pipeline decide si hay una grabación que cancelar.
pub fn install_cancel_key_monitor(on_escape: Arc<dyn Fn() + Send + Sync>) {
    if ON_ESCAPE.set(on_escape).is_err() {
        return; // ya instalado
    }
    let spawned = std::thread::Builder::new().name("dictamelo-esc".into()).spawn(|| {
        let mut was_down = false;
        loop {
            let (down, pressed_since_last) = key_state(VK_ESCAPE);
            if (down && !was_down) || (!down && pressed_since_last) {
                if let Some(on_escape) = ON_ESCAPE.get() {
                    on_escape();
                }
            }
            was_down = down;
            sleep(Duration::from_millis(25));
        }
    });
    match spawned {
        Ok(_) => log::debug!("Monitor de Esc instalado"),
        Err(e) => log::warn!("No se pudo crear el hilo del monitor de Esc: {e}"),
    }
}

/// Solo para el autodiagnóstico: mantiene pulsado el atajo `hotkey` (formato del plugin, p. ej.
/// "Alt+Shift+Space") durante `hold` usando eventos sintéticos, como lo haría una persona.
pub fn press_hotkey_for_test(hotkey: &str, hold: Duration) -> Result<(), PlatformError> {
    let mut modifiers = Vec::new();
    let mut key = None;
    for part in hotkey.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "shift" => modifiers.push(VK_SHIFT),
            "alt" | "option" => modifiers.push(VK_MENU),
            "control" | "ctrl" => modifiers.push(VK_CONTROL),
            "super" | "cmd" | "command" | "meta" => modifiers.push(VK_LWIN),
            other => {
                key = Some(virtual_key(other).ok_or_else(|| {
                    PlatformError::Other(format!("tecla no soportada en el autodiagnóstico: {other}"))
                })?)
            }
        }
    }
    let key = key.ok_or_else(|| PlatformError::Other("el atajo no tiene tecla principal".into()))?;
    for vk in &modifiers {
        send(&[key_event(*vk, false)])?;
        sleep(Duration::from_millis(20));
    }
    send(&[key_event(key, false)])?;
    sleep(hold);
    send(&[key_event(key, true)])?;
    sleep(Duration::from_millis(20));
    for vk in modifiers.iter().rev() {
        send(&[key_event(*vk, true)])?;
        sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Código de tecla virtual para los nombres de tecla (en minúsculas) que acepta el plugin de atajos.
fn virtual_key(name: &str) -> Option<VIRTUAL_KEY> {
    let code: u16 = match name {
        "space" => 0x20,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backquote" => 0xC0,
        "minus" => 0xBD,
        "equal" => 0xBB,
        "bracketleft" => 0xDB,
        "bracketright" => 0xDD,
        "backslash" => 0xDC,
        "semicolon" => 0xBA,
        "quote" => 0xDE,
        "comma" => 0xBC,
        "period" => 0xBE,
        "slash" => 0xBF,
        _ => {
            if let Some(letter) = name.strip_prefix("key").map(str::as_bytes).filter(|b| b.len() == 1 && b[0].is_ascii_alphabetic()) {
                u16::from(letter[0].to_ascii_uppercase()) // 'A'..'Z' = 0x41..0x5A
            } else if let Some(digit) = name.strip_prefix("digit").map(str::as_bytes).filter(|b| b.len() == 1 && b[0].is_ascii_digit()) {
                u16::from(digit[0]) // '0'..'9' = 0x30..0x39
            } else if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()).filter(|n| (1..=24).contains(n)) {
                0x6F + n // VK_F1 = 0x70
            } else {
                return None;
            }
        }
    };
    Some(VIRTUAL_KEY(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_plugin_key_names_to_virtual_keys() {
        assert_eq!(virtual_key("space"), Some(VIRTUAL_KEY(0x20)));
        assert_eq!(virtual_key("keyd"), Some(VIRTUAL_KEY(0x44)));
        assert_eq!(virtual_key("digit7"), Some(VIRTUAL_KEY(0x37)));
        assert_eq!(virtual_key("f13"), Some(VIRTUAL_KEY(0x7C)));
        assert_eq!(virtual_key("f25"), None);
        assert_eq!(virtual_key("noexiste"), None);
    }

    #[test]
    fn key_events_carry_scan_codes() {
        // SAFETY: solo se lee el campo `ki` de una unión inicializada como teclado.
        let event = key_event(VK_V, false);
        let ki = unsafe { event.Anonymous.ki };
        assert_eq!(ki.wVk, VK_V);
        assert_ne!(ki.wScan, 0);
        let up = unsafe { key_event(VK_RMENU, true).Anonymous.ki };
        assert!(up.dwFlags.contains(KEYEVENTF_KEYUP) && up.dwFlags.contains(KEYEVENTF_EXTENDEDKEY));
    }
}
