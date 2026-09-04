// Prueba E2E: simula mantener presionado un atajo durante N segundos usando CGEvent.
// Uso: swift scripts/press_hotkey.swift <segundos> [keycode] [alt] [shift] [ctrl] [cmd]
// Por defecto: Alt+Shift+Space (keycode 49). Requiere permiso de Accesibilidad.
import CoreGraphics
import Foundation

let args = CommandLine.arguments
let seconds = args.count > 1 ? Double(args[1]) ?? 3.0 : 3.0
let keycode = CGKeyCode(args.count > 2 ? UInt16(args[2]) ?? 49 : 49)
let useAlt = args.count > 3 ? args[3] == "1" : true
let useShift = args.count > 4 ? args[4] == "1" : true
let useCtrl = args.count > 5 ? args[5] == "1" : false
let useCmd = args.count > 6 ? args[6] == "1" : false

var flags: CGEventFlags = []
if useAlt { flags.insert(.maskAlternate) }
if useShift { flags.insert(.maskShift) }
if useCtrl { flags.insert(.maskControl) }
if useCmd { flags.insert(.maskCommand) }

let source = CGEventSource(stateID: .hidSystemState)
func post(_ code: CGKeyCode, down: Bool, flags: CGEventFlags) {
    guard let ev = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down) else { return }
    ev.flags = flags
    ev.post(tap: .cghidEventTap)
}
// Modificadores: Option=58, Shift=56, Control=59, Command=55
var modKeys: [CGKeyCode] = []
if useAlt { modKeys.append(58) }
if useShift { modKeys.append(56) }
if useCtrl { modKeys.append(59) }
if useCmd { modKeys.append(55) }

var accumulated: CGEventFlags = []
for m in modKeys {
    switch m {
    case 58: accumulated.insert(.maskAlternate)
    case 56: accumulated.insert(.maskShift)
    case 59: accumulated.insert(.maskControl)
    case 55: accumulated.insert(.maskCommand)
    default: break
    }
    post(m, down: true, flags: accumulated)
    usleep(20_000)
}
post(keycode, down: true, flags: flags)
print("hotkey down; holding \(seconds)s")
fflush(stdout)
Thread.sleep(forTimeInterval: seconds)
post(keycode, down: false, flags: flags)
usleep(20_000)
for m in modKeys.reversed() {
    switch m {
    case 58: accumulated.remove(.maskAlternate)
    case 56: accumulated.remove(.maskShift)
    case 59: accumulated.remove(.maskControl)
    case 55: accumulated.remove(.maskCommand)
    default: break
    }
    post(m, down: false, flags: accumulated)
    usleep(20_000)
}
print("hotkey up")
