// Ventana destino para las pruebas de extremo a extremo, con un menú Editar real (Paste = ⌘V),
// necesario para que una pulsación ⌘V se interprete como pegar. Guarda lo pegado y termina.
// Uso: paste_target <archivo_salida> [segundos_max]
import AppKit

let args = CommandLine.arguments
let outPath = args.count > 1 ? args[1] : "/tmp/dictado_pasted.txt"
let timeout = args.count > 2 ? Double(args[2]) ?? 60 : 60

let app = NSApplication.shared
app.setActivationPolicy(.regular)

// Menú principal con Editar → Pegar (⌘V), Copiar, Cortar, Seleccionar todo.
let mainMenu = NSMenu()
let editItem = NSMenuItem()
mainMenu.addItem(editItem)
let editMenu = NSMenu(title: "Edit")
editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
editItem.submenu = editMenu
app.mainMenu = mainMenu

let window = NSWindow(contentRect: NSRect(x: 260, y: 300, width: 680, height: 220),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "Dictámelo — destino de pegado (prueba automática)"
let scroll = NSScrollView(frame: window.contentView!.bounds)
scroll.autoresizingMask = [.width, .height]
let textView = NSTextView(frame: scroll.bounds)
textView.autoresizingMask = [.width, .height]
textView.font = NSFont.systemFont(ofSize: 16)
textView.isRichText = false
textView.isEditable = true
textView.isSelectable = true
scroll.documentView = textView
window.contentView?.addSubview(scroll)
window.level = .floating
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(textView)
app.activate(ignoringOtherApps: true)

let start = Date()
// Durante los primeros segundos, reafirma el foco cada 250 ms para que ninguna otra app
// (p. ej. una VM que captura el teclado) se quede con el primer plano antes del pegado.
var holdUntil = start.addingTimeInterval(6)
Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { t in
    if Date() > holdUntil { t.invalidate(); return }
    app.activate(ignoringOtherApps: true)
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(textView)
}
var lastLength = 0
var lastChange: Date? = nil
func finish() {
    try? textView.string.write(toFile: outPath, atomically: true, encoding: .utf8)
    app.terminate(nil)
}
Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { _ in
    let content = textView.string
    if content.count != lastLength { lastLength = content.count; lastChange = Date() }
    let now = Date()
    if let c = lastChange, now.timeIntervalSince(c) > 2.5 { finish() }
    if now.timeIntervalSince(start) > timeout { finish() }
}
app.run()
