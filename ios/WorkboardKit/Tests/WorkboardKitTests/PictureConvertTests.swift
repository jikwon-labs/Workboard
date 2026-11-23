import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import WorkboardKit

final class PictureConvertTests: XCTestCase {
    /// A 64x32 picture as `type`, red on its left half, with a place and a
    /// quarter turn; `clear` makes one corner pixel see-through.
    func picture(_ type: UTType, clear: Bool = false) throws -> Data {
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 64, height: 32, bitsPerComponent: 8, bytesPerRow: 0,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(red: 0, green: 0, blue: 1, alpha: 1)
        ctx.fill(CGRect(x: 0, y: 0, width: 64, height: 32))
        ctx.setFillColor(red: 1, green: 0, blue: 0, alpha: 1)
        ctx.fill(CGRect(x: 0, y: 0, width: 32, height: 32))
        if clear { ctx.clear(CGRect(x: 0, y: 0, width: 1, height: 1)) }
        let img = try XCTUnwrap(ctx.makeImage())
        let props: [CFString: Any] = [
            kCGImagePropertyOrientation: 6,
            kCGImagePropertyGPSDictionary: [kCGImagePropertyGPSLatitude: 37.5665, kCGImagePropertyGPSLatitudeRef: "N"],
        ]
        let data = NSMutableData()
        let dest = try XCTUnwrap(CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(dest, img, props as CFDictionary)
        XCTAssertTrue(CGImageDestinationFinalize(dest))
        return data as Data
    }

    func props(_ d: Data) throws -> [CFString: Any] {
        let src = try XCTUnwrap(CGImageSourceCreateWithData(d as CFData, nil))
        return try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any])
    }

    func testAHeicGoesAsAnUprightJpegWithoutWhereItWasTaken() throws {
        let (out, ext) = try XCTUnwrap(PictureConvert.converted(try picture(.heic)))
        XCTAssertEqual(ext, "jpg")
        XCTAssertEqual(PictureConvert.commonExt(out), "jpg")
        let p = try props(out)
        // the quarter turn is applied to the pixels, not left as a tag
        XCTAssertEqual(p[kCGImagePropertyPixelWidth] as? Int, 32)
        XCTAssertEqual(p[kCGImagePropertyPixelHeight] as? Int, 64)
        XCTAssertNil(p[kCGImagePropertyGPSDictionary])
    }

    func testASeeThroughTiffGoesAsAPng() throws {
        XCTAssertEqual(PictureConvert.converted(try picture(.tiff, clear: true))?.ext, "png")
        XCTAssertEqual(PictureConvert.converted(try picture(.tiff))?.ext, "jpg", "solid everywhere")
    }

    func testABigOneIsShrunk() throws {
        let (out, _) = try XCTUnwrap(PictureConvert.converted(try picture(.bmp), maxSide: 16))
        let p = try props(out)
        XCTAssertEqual(p[kCGImagePropertyPixelHeight] as? Int, 16)
        XCTAssertEqual(p[kCGImagePropertyPixelWidth] as? Int, 8)
    }

    func testJpegPngGifAndWebpGoAsTheyAre() throws {
        for (type, ext) in [(UTType.jpeg, "jpg"), (.png, "png"), (.gif, "gif")] {
            let d = try picture(type)
            XCTAssertEqual(PictureConvert.commonExt(d), ext)
            XCTAssertNil(PictureConvert.converted(d), ext)
        }
        let webp = Data("RIFF\0\0\0\0WEBPVP8 ".utf8)
        XCTAssertEqual(PictureConvert.commonExt(webp), "webp")
        XCTAssertNil(PictureConvert.converted(webp))
        XCTAssertNil(PictureConvert.converted(Data("not a picture".utf8)))
    }

    /// The red and blue of a picture's top and bottom rows, at the middle.
    func topAndBottom(_ png: Data) throws -> (top: [UInt8], bottom: [UInt8]) {
        let src = try XCTUnwrap(CGImageSourceCreateWithData(png as CFData, nil))
        let img = try XCTUnwrap(CGImageSourceCreateImageAtIndex(src, 0, nil))
        let (w, h) = (img.width, img.height)
        var px = [UInt8](repeating: 0, count: w * h * 4)
        px.withUnsafeMutableBytes { buf in
            let ctx = CGContext(data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            ctx?.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
        }
        let at = { (row: Int) in [px[(row * w + w / 2) * 4], px[(row * w + w / 2) * 4 + 2]] }
        return (at(0), at(h - 1))
    }

    /// As the Mac's square_png: the middle square of the upright picture,
    /// as PNG, never made larger.
    func testAProfilePhotoIsTheUprightMiddleSquare() throws {
        // 64x32 with a quarter turn: upright it is 32x64, red above blue
        let (png, side) = try XCTUnwrap(PictureConvert.profilePhoto(try picture(.jpeg)))
        XCTAssertEqual(side, 32, "never made larger")
        XCTAssertEqual(PictureConvert.commonExt(png), "png")
        let p = try props(png)
        XCTAssertEqual(p[kCGImagePropertyPixelWidth] as? Int, 32)
        XCTAssertEqual(p[kCGImagePropertyPixelHeight] as? Int, 32)
        XCTAssertNil(p[kCGImagePropertyGPSDictionary])
        XCTAssertNil(p[kCGImagePropertyOrientation], "the turn is in the pixels")
        // the middle of 32x64: its top row in the red half, its bottom in the blue
        let (top, bottom) = try topAndBottom(png)
        XCTAssertGreaterThan(top[0], 200)
        XCTAssertLessThan(top[1], 60)
        XCTAssertLessThan(bottom[0], 60)
        XCTAssertGreaterThan(bottom[1], 200)
    }

    func testABigProfilePhotoIsShrunk() throws {
        let (png, side) = try XCTUnwrap(PictureConvert.profilePhoto(try picture(.heic), side: 16))
        XCTAssertEqual(side, 16)
        let p = try props(png)
        XCTAssertEqual(p[kCGImagePropertyPixelWidth] as? Int, 16)
        XCTAssertEqual(p[kCGImagePropertyPixelHeight] as? Int, 16)
        XCTAssertNil(PictureConvert.profilePhoto(Data("not a picture".utf8)))
    }
}
