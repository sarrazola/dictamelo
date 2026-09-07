#!/usr/bin/env swift

import AppKit
import Foundation

private let canvasSize = NSSize(width: 660, height: 400)
private let pixelScale = 2

private func color(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat, alpha: CGFloat = 1) -> NSColor {
    NSColor(deviceRed: red / 255, green: green / 255, blue: blue / 255, alpha: alpha)
}

private func drawText(
    _ value: String,
    in rect: NSRect,
    font: NSFont,
    color: NSColor,
    alignment: NSTextAlignment = .center
) {
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = alignment

    value.draw(
        in: rect,
        withAttributes: [
            .font: font,
            .foregroundColor: color,
            .paragraphStyle: paragraph
        ]
    )
}

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write(Data("Usage: generate-dmg-background.swift /path/background.png\n".utf8))
    exit(64)
}

let outputURL = URL(fileURLWithPath: CommandLine.arguments[1])
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: Int(canvasSize.width) * pixelScale,
    pixelsHigh: Int(canvasSize.height) * pixelScale,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
)!

// A 1320x800 bitmap with a 660x400 point size gives Finder a crisp @2x
// background while keeping the installer window compact on every display.
bitmap.size = canvasSize

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
defer { NSGraphicsContext.restoreGraphicsState() }

let bounds = NSRect(origin: .zero, size: canvasSize)
NSGradient(
    starting: color(255, 252, 245),
    ending: color(241, 242, 255)
)!.draw(in: bounds, angle: -18)

color(255, 209, 89, alpha: 0.20).setFill()
NSBezierPath(ovalIn: NSRect(x: -96, y: 226, width: 280, height: 280)).fill()

color(186, 186, 255, alpha: 0.20).setFill()
NSBezierPath(ovalIn: NSRect(x: 510, y: -104, width: 280, height: 260)).fill()

let cardRect = NSRect(x: 34, y: 24, width: 592, height: 352)
let cardPath = NSBezierPath(roundedRect: cardRect, xRadius: 28, yRadius: 28)
let cardShadow = NSShadow()
cardShadow.shadowColor = color(72, 102, 138, alpha: 0.14)
cardShadow.shadowBlurRadius = 18
cardShadow.shadowOffset = NSSize(width: 0, height: -7)
cardShadow.set()
color(255, 255, 255, alpha: 0.91).setFill()
cardPath.fill()

NSShadow().set()
color(203, 210, 221, alpha: 0.72).setStroke()
cardPath.lineWidth = 1
cardPath.stroke()

drawText(
    "Install Dictámelo",
    in: NSRect(x: 80, y: 308, width: 500, height: 48),
    font: .systemFont(ofSize: 28, weight: .bold),
    color: color(36, 36, 40)
)

drawText(
    "Drag Dictámelo to Applications",
    in: NSRect(x: 80, y: 276, width: 500, height: 32),
    font: .systemFont(ofSize: 15.5, weight: .semibold),
    color: color(119, 119, 124)
)

color(255, 196, 0, alpha: 0.11).setFill()
NSBezierPath(ovalIn: NSRect(x: 108, y: 121, width: 128, height: 128)).fill()

color(91, 91, 245, alpha: 0.07).setFill()
NSBezierPath(ovalIn: NSRect(x: 424, y: 121, width: 128, height: 128)).fill()

let arrow = NSBezierPath()
arrow.move(to: NSPoint(x: 272, y: 184))
arrow.curve(
    to: NSPoint(x: 390, y: 184),
    controlPoint1: NSPoint(x: 310, y: 207),
    controlPoint2: NSPoint(x: 355, y: 207)
)
arrow.move(to: NSPoint(x: 390, y: 184))
arrow.line(to: NSPoint(x: 370, y: 203))
arrow.move(to: NSPoint(x: 390, y: 184))
arrow.line(to: NSPoint(x: 370, y: 165))
arrow.lineWidth = 5
arrow.lineCapStyle = .round
arrow.lineJoinStyle = .round
color(91, 91, 245).setStroke()
arrow.stroke()

drawText(
    "Then open Dictámelo from Applications.",
    in: NSRect(x: 80, y: 55, width: 500, height: 28),
    font: .systemFont(ofSize: 14, weight: .semibold),
    color: color(108, 108, 119)
)

drawText(
    "Your voice. Written.",
    in: NSRect(x: 80, y: 34, width: 500, height: 22),
    font: .systemFont(ofSize: 12, weight: .medium),
    color: color(139, 139, 149)
)

guard let data = bitmap.representation(using: .png, properties: [:]) else {
    FileHandle.standardError.write(Data("Could not encode the DMG background.\n".utf8))
    exit(70)
}

try FileManager.default.createDirectory(
    at: outputURL.deletingLastPathComponent(),
    withIntermediateDirectories: true
)
try data.write(to: outputURL, options: .atomic)
