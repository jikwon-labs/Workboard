import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Pictures not every device opens (HEIC, TIFF, BMP: Windows can't show a
/// HEIC) go as a JPEG, or a PNG when see-through, upright and no larger than
/// `maxSide`; a JPEG, PNG, GIF or WebP goes as it is. Ported from
/// media::converted_for_sending in src/media.rs; the two must agree.
public enum PictureConvert {
    /// Longer side of a converted picture (media::CONVERTED_MAX).
    public static let maxSide = 4096
    /// Endings of a picked file that get converted (media::CONVERTED_EXTS).
    public static let convertedExts: Set<String> = ["heic", "heif", "tif", "tiff", "bmp"]

    /// The ending a JPEG, PNG, GIF or WebP goes under, from its bytes; nil
    /// for any other picture.
    public static func commonExt(_ data: Data) -> String? {
        let b = [UInt8](data.prefix(12))
        if b.starts(with: [0xFF, 0xD8, 0xFF]) { return "jpg" }
        if b.starts(with: MediaScrub.pngSig) { return "png" }
        if b.starts(with: Array("GIF8".utf8)) { return "gif" }
        if b.count >= 12, b[0..<4].elementsEqual(Array("RIFF".utf8)), b[8..<12].elementsEqual(Array("WEBP".utf8)) {
            return "webp"
        }
        return nil
    }

    /// The picture as JPEG or PNG bytes and that ending, upright and no
    /// larger than `maxSide`. Nil for a JPEG, PNG, GIF or WebP (it goes as
    /// it is) and for bytes that aren't a picture.
    public static func converted(_ data: Data, maxSide: Int = maxSide) -> (data: Data, ext: String)? {
        guard commonExt(data) == nil,
              let src = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let opts: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxSide,
        ]
        guard let img = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
        if seeThrough(img) {
            return encode(img, .png, quality: nil).map { ($0, "png") }
        }
        return encode(img, .jpeg, quality: 0.85).map { ($0, "jpg") }
    }

    /// Whether any pixel is (partly) see-through: a picture with an alpha
    /// channel is often fully solid all the same.
    static func seeThrough(_ img: CGImage) -> Bool {
        switch img.alphaInfo {
        case .none, .noneSkipFirst, .noneSkipLast: return false
        default: break
        }
        let (w, h) = (img.width, img.height)
        var px = [UInt8](repeating: 0, count: w * h * 4)
        let drawn = px.withUnsafeMutableBytes { buf -> Bool in
            guard let ctx = CGContext(
                data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
            return true
        }
        return drawn && stride(from: 3, to: px.count, by: 4).contains { px[$0] < 255 }
    }

    /// The picture written as `type`; nothing of the original's metadata
    /// (where it was taken, the phone) is carried over, its colour profile is.
    static func encode(_ img: CGImage, _ type: UTType, quality: Double?) -> Data? {
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, type.identifier as CFString, 1, nil) else { return nil }
        let props = quality.map { [kCGImageDestinationLossyCompressionQuality: $0] as CFDictionary }
        CGImageDestinationAddImage(dest, img, props)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return out as Data
    }
}
