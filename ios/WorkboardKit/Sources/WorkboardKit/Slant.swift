// Italics that lean in every script. Apple has no italic Korean, Japanese
// or Chinese font, so SwiftUI's .italic() and UIKit's traitItalic leave
// those letters upright: on the iPhone *기울임* looked like plain text,
// while the Mac app (egui slants the glyphs) shows it leaning. A font with
// a slant in its matrix leans every letter instead, and Core Text hands
// the matrix on to the fallback font that draws the Hangul.

import CoreText
import Foundation
import SwiftUI

public enum Slant {
    /// About 12 degrees, close to the system font's own italic.
    public static let matrix = CGAffineTransform(a: 1, b: 0, c: 0.21, d: 1, tx: 0, ty: 0)

    /// `font` leaning right, every script. `font` should be upright (an
    /// italic face would lean twice).
    public static func font(_ font: CTFont) -> CTFont {
        var m = matrix
        return CTFontCreateWithFontDescriptor(CTFontCopyFontDescriptor(font), CTFontGetSize(font), &m)
    }

    /// `text` (Markdown read by AttributedString) with its *italic* runs in
    /// a leaning font. `base` gives the upright font the text is shown in,
    /// bold for runs that are also **bold**.
    public static func italics(_ text: AttributedString, base: (_ bold: Bool) -> CTFont) -> AttributedString {
        var out = text
        for run in text.runs {
            guard let intent = run.inlinePresentationIntent, intent.contains(.emphasized) else { continue }
            out[run.range].swiftUI.font = Font(font(base(intent.contains(.stronglyEmphasized))))
            // the font has both now; left in, SwiftUI would look for an italic face again
            out[run.range].inlinePresentationIntent = intent.subtracting([.emphasized, .stronglyEmphasized])
        }
        return out
    }
}
