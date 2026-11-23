import CoreText
import XCTest
@testable import WorkboardKit

/// Italics lean in every script (Slant.swift): Apple has no italic Korean
/// font, so *기울임* stayed upright on the iPhone.
final class SlantTests: XCTestCase {
    /// The font that draws each run of `s` set in `font`, and its matrix.
    func runFonts(_ s: String, _ font: CTFont) -> [(String, CGAffineTransform)] {
        let a = NSAttributedString(string: s, attributes: [NSAttributedString.Key(kCTFontAttributeName as String): font])
        let line = CTLineCreateWithAttributedString(a)
        return (CTLineGetGlyphRuns(line) as! [CTRun]).map { run in
            let f = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] as! CTFont
            return (CTFontCopyPostScriptName(f) as String, CTFontGetMatrix(f))
        }
    }

    func testHangulLeansToo() {
        let upright = CTFontCreateUIFontForLanguage(.system, 17, nil)!
        let before = runFonts("ab 기울임", upright)
        XCTAssertGreaterThan(before.count, 1, "Hangul comes from a fallback font")
        XCTAssertTrue(before.allSatisfy { $0.1.c == 0 })
        // the trait the iPhone used to ask for: no italic Hangul exists
        let traited = CTFontCreateCopyWithSymbolicTraits(upright, 0, nil, .traitItalic, .traitItalic)!
        XCTAssertTrue(runFonts("기울임", traited).allSatisfy { $0.1.c == 0 && !$0.0.contains("Italic") }, "\(runFonts("기울임", traited))")

        let leaning = Slant.font(upright)
        XCTAssertEqual(CTFontGetSize(leaning), 17)
        let after = runFonts("ab 기울임", leaning)
        XCTAssertEqual(after.count, before.count)
        XCTAssertTrue(after.allSatisfy { $0.1 == Slant.matrix }, "\(after)")
    }

    func testItalicRunsGetALeaningFont() {
        let src = ChatFormat.inline("plain *기울임* **굵게** ***둘 다*** `code`")
        var asked: [Bool] = []
        let out = Slant.italics(src) { bold in
            asked.append(bold)
            return CTFontCreateUIFontForLanguage(bold ? .emphasizedSystem : .system, 17, nil)!
        }
        XCTAssertEqual(String(out.characters), String(src.characters))
        XCTAssertEqual(asked, [false, true], "one upright font for *기울임*, one bold for ***둘 다***")
        func run(_ s: String) -> AttributedString.Runs.Run? {
            out.runs.first { String(out[$0.range].characters) == s }
        }
        XCTAssertNotNil(run("기울임")?.swiftUI.font)
        XCTAssertFalse(run("기울임")?.inlinePresentationIntent?.contains(.emphasized) ?? false)
        XCTAssertNotNil(run("둘 다")?.swiftUI.font)
        XCTAssertEqual(run("둘 다")?.inlinePresentationIntent?.contains(.stronglyEmphasized), false, "bold is in the font")
        XCTAssertNil(run("굵게")?.swiftUI.font, "bold alone stays as it was")
        XCTAssertEqual(run("굵게")?.inlinePresentationIntent, .stronglyEmphasized)
        XCTAssertEqual(run("code")?.inlinePresentationIntent, .code)
        XCTAssertEqual(Slant.italics(ChatFormat.inline("nothing here")) { _ in fatalError() }, ChatFormat.inline("nothing here"))
    }
}
