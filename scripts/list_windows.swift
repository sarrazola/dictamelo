// Lista las ventanas visibles (app propietaria, título, capa) sin permisos especiales.
import CoreGraphics
import Foundation

let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? "?"
    let name = w[kCGWindowName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    let bounds = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
    if layer == 0 || owner.contains("Notification") || owner.contains("Dictámelo") || owner.contains("Python") {
        print("\(owner) | \(name) | layer \(layer) | \(bounds["Width"] ?? 0)x\(bounds["Height"] ?? 0)")
    }
}
