//! Standalone Fn/Globe is a modifier event, not a registerable Carbon shortcut.
//! The tap owns its run loop on one thread and consumes only the selected Fn key.

use crate::{pipeline, recording_control::Event, util::lock};
use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_graphics::event::{CallbackResult, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;

const FN_KEYCODE: i64 = 63;

#[derive(Default)]
struct Shared {
    enabled: AtomicBool,
    stopped: AtomicBool,
}

#[derive(Default)]
pub struct FnHotkey {
    shared: Arc<Shared>,
    installed: Mutex<bool>,
}

impl FnHotkey {
    pub fn enable(&self, app: &AppHandle) -> Result<(), ()> {
        let mut installed = lock(&self.installed);
        if !*installed {
            let shared = self.shared.clone();
            let app = app.clone();
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            std::thread::Builder::new().name("dictamelo-fn".into()).spawn(move || {
                let down = Arc::new(AtomicBool::new(false));
                let reset = Arc::new(AtomicBool::new(false));
                let (callback_down, callback_reset, callback_shared) = (down.clone(), reset.clone(), shared.clone());
                let tap = CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![CGEventType::FlagsChanged],
                    move |_, kind, event| {
                        if matches!(kind, CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput) {
                            callback_down.store(false, Ordering::SeqCst);
                            callback_reset.store(true, Ordering::SeqCst);
                            // A lost release must never leave the microphone running.
                            if callback_shared.enabled.load(Ordering::SeqCst) {
                                pipeline::cancel_recording(&app);
                                pipeline::enqueue_control(&app, Event::Reset);
                            }
                            return CallbackResult::Keep;
                        }
                        if !callback_shared.enabled.load(Ordering::SeqCst) {
                            callback_down.store(false, Ordering::SeqCst);
                            return CallbackResult::Keep;
                        }
                        if !matches!(kind, CGEventType::FlagsChanged) { return CallbackResult::Keep; }
                        let was_down = callback_down.load(Ordering::SeqCst);
                        if let Some(pressed) = fn_edge(event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE), event.get_flags(), was_down) {
                            callback_down.store(pressed, Ordering::SeqCst);
                            if pressed { pipeline::hotkey_pressed(&app); } else { pipeline::hotkey_released(&app); }
                            return CallbackResult::Drop;
                        }
                        CallbackResult::Keep
                    },
                );
                let Ok(tap) = tap else { let _ = ready_tx.send(false); return; };
                let Ok(source) = tap.mach_port().create_runloop_source(0) else { let _ = ready_tx.send(false); return; };
                CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
                tap.enable();
                let _ = ready_tx.send(true);
                while !shared.stopped.load(Ordering::SeqCst) {
                    CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::from_millis(250), false);
                    if reset.swap(false, Ordering::SeqCst) { tap.enable(); }
                }
                CFRunLoop::get_current().remove_source(&source, unsafe { kCFRunLoopCommonModes });
            }).map_err(|_| ())?;
            if !ready_rx.recv().unwrap_or(false) { return Err(()); }
            *installed = true;
        }
        self.shared.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn disable(&self) { self.shared.enabled.store(false, Ordering::SeqCst); }
}

impl Drop for FnHotkey {
    fn drop(&mut self) { self.shared.stopped.store(true, Ordering::SeqCst); }
}

fn fn_edge(keycode: i64, flags: CGEventFlags, was_down: bool) -> Option<bool> {
    // Arrow/function keys also carry SecondaryFn: the physical key code matters.
    if keycode != FN_KEYCODE { return None; }
    let down = flags.contains(CGEventFlags::CGEventFlagSecondaryFn);
    if down == was_down { return None; }
    let other_modifiers = CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagAlternate | CGEventFlags::CGEventFlagShift;
    if down && flags.intersects(other_modifiers) { return None; }
    Some(down)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_physical_fn_produces_edges_and_release_survives_other_modifiers() {
        let flags = CGEventFlags::CGEventFlagSecondaryFn;
        assert_eq!(fn_edge(63, flags, false), Some(true));
        assert_eq!(fn_edge(63, flags, true), None);
        assert_eq!(fn_edge(63, CGEventFlags::CGEventFlagNull, true), Some(false));
        for key in [123, 124, 125, 126, 122, 120] { assert_eq!(fn_edge(key, flags, false), None); }
        assert_eq!(fn_edge(63, flags | CGEventFlags::CGEventFlagCommand, false), None);
        assert_eq!(fn_edge(63, CGEventFlags::CGEventFlagCommand, true), Some(false));
    }
}
