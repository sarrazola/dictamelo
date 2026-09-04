// Diagnóstico: ¿este proceso (y por tanto los que lance desde aquí) tiene Accesibilidad y micrófono?
import ApplicationServices
import AVFoundation

let trusted = AXIsProcessTrusted()
let mic = AVCaptureDevice.authorizationStatus(for: .audio)
let micLabel: String
switch mic {
case .authorized: micLabel = "authorized"
case .denied: micLabel = "denied"
case .restricted: micLabel = "restricted"
default: micLabel = "notDetermined"
}
print("accessibility_trusted=\(trusted) microphone=\(micLabel)")
