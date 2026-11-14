// Draws the Workboard app icon ("W into a check" on black) and writes
// Resources/AppIcon.icns. Run from the repo root: swift scripts/make_icon.swift
// The geometry is the 1024 px master from the design canvas; Resources/AppIcon.svg
// holds the same drawing as SVG.
import AppKit
import CoreGraphics

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: a)
}

func draw(px: Int) -> Data {
    let cs = CGColorSpace(name: CGColorSpace.sRGB)!
    let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0, space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    let s = CGFloat(px) / 1024
    // work in the master's top-left coordinates
    ctx.translateBy(x: 0, y: CGFloat(px))
    ctx.scaleBy(x: s, y: -s)

    let ground = CGPath(roundedRect: CGRect(x: 100, y: 100, width: 824, height: 824), cornerWidth: 185, cornerHeight: 185, transform: nil)
    // soft shadow under the rounded square (shadow offsets are in device space)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -14 * s), blur: 32 * s, color: rgb(0x000000, 0.3))
    ctx.addPath(ground)
    ctx.setFillColor(rgb(0x0C0C0E))
    ctx.fillPath()
    ctx.restoreGState()
    // near-black ground with a faint top-to-bottom sheen
    let groundGrad = CGGradient(colorsSpace: cs, colors: [rgb(0x26252B), rgb(0x0C0C0E)] as CFArray, locations: [0, 1])!
    ctx.saveGState()
    ctx.addPath(ground)
    ctx.clip()
    ctx.drawLinearGradient(groundGrad, start: CGPoint(x: 0, y: 100), end: CGPoint(x: 0, y: 924), options: [])
    ctx.restoreGState()
    // hairline edge
    ctx.addPath(CGPath(roundedRect: CGRect(x: 103, y: 103, width: 818, height: 818), cornerWidth: 182, cornerHeight: 182, transform: nil))
    ctx.setStrokeColor(rgb(0xFFFFFF, 0.09))
    ctx.setLineWidth(6)
    ctx.strokePath()

    // the W whose last stroke rises into a check, in warm white
    let pts: [CGPoint] = [CGPoint(x: 250, y: 450), CGPoint(x: 352, y: 680), CGPoint(x: 470, y: 486), CGPoint(x: 588, y: 680), CGPoint(x: 790, y: 330)]
    let line = CGMutablePath()
    line.addLines(between: pts)
    let stroke = line.copy(strokingWithWidth: 70, lineCap: .round, lineJoin: .round, miterLimit: 10)
    let markGrad = CGGradient(colorsSpace: cs, colors: [rgb(0xFFFFFF), rgb(0xDEDCD6)] as CFArray, locations: [0, 1])!
    ctx.saveGState()
    ctx.addPath(stroke)
    ctx.clip()
    let box = stroke.boundingBox
    ctx.drawLinearGradient(markGrad, start: CGPoint(x: 0, y: box.minY), end: CGPoint(x: 0, y: box.maxY), options: [])
    ctx.restoreGState()
    // the small accent dot at the tip of the check
    ctx.setFillColor(rgb(0x8B7BFF))
    ctx.fillEllipse(in: CGRect(x: 790 - 35, y: 330 - 35, width: 70, height: 70))

    let rep = NSBitmapImageRep(cgImage: ctx.makeImage()!)
    return rep.representation(using: .png, properties: [:])!
}

let fm = FileManager.default
let iconset = URL(fileURLWithPath: "build/AppIcon.iconset")
try? fm.removeItem(at: iconset)
try fm.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    try draw(px: base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
    try draw(px: base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
try fm.createDirectory(at: URL(fileURLWithPath: "Resources"), withIntermediateDirectories: true)
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
p.arguments = ["-c", "icns", iconset.path, "-o", "Resources/AppIcon.icns"]
try p.run()
p.waitUntilExit()
print(p.terminationStatus == 0 ? "wrote Resources/AppIcon.icns" : "iconutil failed")
