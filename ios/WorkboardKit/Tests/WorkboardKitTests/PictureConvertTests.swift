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
}
