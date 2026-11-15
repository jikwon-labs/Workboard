// Draws the iOS app icon from the same geometry as Resources/AppIcon.svg and
// scripts/make_icon.swift ("W into a check" on black), and writes
// ios/Workboard/Assets.xcassets/AppIcon.appiconset/AppIcon-1024.png.
// Run from ios/: swift scripts/make_ios_icon.swift
//
// iOS rounds the corners itself, so there is no rounded square, edge or
// shadow here: the dark ground fills the whole square and the Mac icon's
// square (x/y 100...924) is scaled up to the full 1024 px.
import AppKit
import CoreGraphics

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: a)
}

let px = 1024
let cs = CGColorSpace(name: CGColorSpace.sRGB)!
// no alpha channel: App Store icons must be opaque
let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0, space: cs, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
ctx.translateBy(x: 0, y: CGFloat(px))
ctx.scaleBy(x: 1, y: -1)

let ground = CGGradient(colorsSpace: cs, colors: [rgb(0x26252B), rgb(0x0C0C0E)] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(ground, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: 1024), options: [])

// master square 100...924 -> 0...1024
let k = CGFloat(1024) / 824
ctx.scaleBy(x: k, y: k)
ctx.translateBy(x: -100, y: -100)

let pts = [CGPoint(x: 250, y: 450), CGPoint(x: 352, y: 680), CGPoint(x: 470, y: 486), CGPoint(x: 588, y: 680), CGPoint(x: 790, y: 330)]
let line = CGMutablePath()
line.addLines(between: pts)
let stroke = line.copy(strokingWithWidth: 70, lineCap: .round, lineJoin: .round, miterLimit: 10)
let mark = CGGradient(colorsSpace: cs, colors: [rgb(0xFFFFFF), rgb(0xDEDCD6)] as CFArray, locations: [0, 1])!
ctx.saveGState()
ctx.addPath(stroke)
ctx.clip()
let box = stroke.boundingBox
ctx.drawLinearGradient(mark, start: CGPoint(x: 0, y: box.minY), end: CGPoint(x: 0, y: box.maxY), options: [])
ctx.restoreGState()
ctx.setFillColor(rgb(0x8B7BFF))
ctx.fillEllipse(in: CGRect(x: 790 - 35, y: 330 - 35, width: 70, height: 70))

let rep = NSBitmapImageRep(cgImage: ctx.makeImage()!)
let out = URL(fileURLWithPath: "Workboard/Assets.xcassets/AppIcon.appiconset/AppIcon-1024.png")
try rep.representation(using: .png, properties: [:])!.write(to: out)
print("wrote \(out.path)")
