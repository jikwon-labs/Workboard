import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import WorkboardKit

final class MediaScrubTests: XCTestCase {
    /// A 64x32 picture written by ImageIO with a place, a phone, a date and
    /// a quarter turn, as a phone's camera would.
    func photo(_ type: UTType) throws -> Data {
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 64, height: 32, bitsPerComponent: 8, bytesPerRow: 0,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
        ctx.setFillColor(red: 1, green: 0, blue: 0, alpha: 1)
        ctx.fill(CGRect(x: 0, y: 0, width: 32, height: 32))
        let img = try XCTUnwrap(ctx.makeImage())
        let props: [CFString: Any] = [
            kCGImagePropertyOrientation: 6,
            kCGImagePropertyGPSDictionary: [kCGImagePropertyGPSLatitude: 37.5665, kCGImagePropertyGPSLatitudeRef: "N",
                                            kCGImagePropertyGPSLongitude: 126.978, kCGImagePropertyGPSLongitudeRef: "E"],
            kCGImagePropertyTIFFDictionary: [kCGImagePropertyTIFFMake: "SecretPhone"],
            kCGImagePropertyExifDictionary: [kCGImagePropertyExifDateTimeOriginal: "2026:11:20 10:00:00"],
        ]
        let data = NSMutableData()
        let dest = try XCTUnwrap(CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(dest, img, props as CFDictionary)
        XCTAssertTrue(CGImageDestinationFinalize(dest))
        return data as Data
    }

    struct Seen {
        let w: Int, h: Int, turn: Int?, gps: Bool, make: Bool, date: Bool
    }

    func look(_ d: Data) throws -> Seen {
        let src = try XCTUnwrap(CGImageSourceCreateWithData(d as CFData, nil))
        let img = try XCTUnwrap(CGImageSourceCreateImageAtIndex(src, 0, nil))
        let p = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any] ?? [:]
        let tiff = p[kCGImagePropertyTIFFDictionary] as? [CFString: Any] ?? [:]
        let exif = p[kCGImagePropertyExifDictionary] as? [CFString: Any] ?? [:]
        return Seen(w: img.width, h: img.height, turn: p[kCGImagePropertyOrientation] as? Int,
                    gps: p[kCGImagePropertyGPSDictionary] != nil, make: tiff[kCGImagePropertyTIFFMake] != nil,
                    date: exif[kCGImagePropertyExifDateTimeOriginal] != nil)
    }

    func contains(_ d: Data, _ s: String) -> Bool { d.range(of: Data(s.utf8)) != nil }

    func testAPhotoLosesWhereAndWhichPhoneButKeepsItsTurn() throws {
        for type in [UTType.jpeg, .heic, .png] {
            let src = try photo(type)
            let before = try look(src)
            XCTAssertTrue(before.gps && before.make && before.date, "\(type)")
            let out = try XCTUnwrap(MediaScrub.scrub(src), "\(type)")
            let after = try look(out)
            XCTAssertFalse(after.gps || after.make || after.date, "\(type)")
            XCTAssertEqual(after.turn, 6, "\(type)")
            XCTAssertEqual([after.w, after.h], [64, 32], "\(type)")
            XCTAssertFalse(contains(out, "SecretPhone"), "\(type)")
            // a second pass finds nothing more to take
            XCTAssertEqual(MediaScrub.scrub(out), out, "\(type)")
        }
    }

    func box(_ kind: String, _ body: [UInt8]) -> [UInt8] {
        let n = body.count + 8
        return [UInt8(n >> 24 & 0xFF), UInt8(n >> 16 & 0xFF), UInt8(n >> 8 & 0xFF), UInt8(n & 0xFF)] + Array(kind.utf8) + body
    }

    func testAVideoLosesItsLocationAndNothingMoves() throws {
        let loc = Array("+37.5665+126.9780/".utf8)
        let meta = box("meta", box("keys", Array("com.apple.quicktime.location.ISO6709".utf8)) + box("ilst", loc))
        let trak = box("trak", box("tkhd", [UInt8](repeating: 7, count: 20)) + box("udta", Array("SECRET-TRACK".utf8)))
        let moov = box("moov", box("mvhd", [UInt8](repeating: 5, count: 20)) + trak + box("udta", box("\u{A9}xyz", loc)) + meta)
        let src = Data(box("ftyp", Array("qt  \0\0\0\0qt  ".utf8)) + box("mdat", Array("FRAMES".utf8)) + moov)
        let out = try XCTUnwrap(MediaScrub.scrub(src))
        XCTAssertEqual(out.count, src.count)
        XCTAssertFalse(contains(out, "+37.5665") || contains(out, "SECRET-TRACK") || contains(out, "location"))
        XCTAssertTrue(contains(out, "FRAMES"))
        XCTAssertTrue(out.range(of: Data([UInt8](repeating: 7, count: 20))) != nil)

        // and in place on disk, as an upload does
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".mov")
        defer { try? FileManager.default.removeItem(at: url) }
        try src.write(to: url)
        XCTAssertTrue(try MediaScrub.scrubFile(at: url))
        XCTAssertEqual(try Data(contentsOf: url), out)
        XCTAssertFalse(try MediaScrub.scrubFile(at: url), "nothing left to take")
    }

    func testWebPLosesItsExifAndXmpChunks() throws {
        func chunk(_ k: String, _ d: [UInt8]) -> [UInt8] {
            let n = d.count
            return Array(k.utf8) + [UInt8(n & 0xFF), UInt8(n >> 8 & 0xFF), 0, 0] + d + (n & 1 == 1 ? [0] : [])
        }
        let body = Array("WEBP".utf8) + chunk("VP8X", [0x0C, 0, 0, 0, 3, 0, 0, 3, 0, 0]) + chunk("VP8L", [1, 2, 3])
            + chunk("EXIF", Array("GPS +37.5665".utf8)) + chunk("XMP ", Array("<xmp>SecretPhone</xmp>".utf8))
        let n = body.count
        let src = Data(Array("RIFF".utf8) + [UInt8(n & 0xFF), UInt8(n >> 8 & 0xFF), 0, 0] + body)
        let out = try XCTUnwrap(MediaScrub.scrub(src))
        XCTAssertFalse(contains(out, "+37.5665") || contains(out, "SecretPhone"))
        XCTAssertEqual(out[20] & 0x0C, 0)
        XCTAssertEqual(Int(out[4]) | Int(out[5]) << 8, out.count - 8)
        XCTAssertTrue(contains(out, "VP8L"))
    }

    func testOtherFilesAndBrokenOnesGoAsTheyAre() {
        XCTAssertNil(MediaScrub.scrub(Data("GIF89a....".utf8)))
        XCTAssertNil(MediaScrub.scrub(Data("%PDF-1.7".utf8)))
        XCTAssertNil(MediaScrub.scrub(Data([0xFF, 0xD8, 0xFF, 0xE1, 0x40])))
        XCTAssertNil(MediaScrub.scrub(Data(MediaScrub.pngSig + [0, 0, 0, 0x0D] + Array("IHDR".utf8))))
        XCTAssertNil(MediaScrub.scrub(Data(box("ftyp", Array("isom".utf8)) + [0, 0, 0x10, 0] + Array("moov".utf8))))
    }
}
