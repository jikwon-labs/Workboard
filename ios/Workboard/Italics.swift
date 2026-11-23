// Italics in chat messages and docs that lean in Korean too (WorkboardKit
// Slant.swift says why .italic() can't).

import SwiftUI
import UIKit
import WorkboardKit

extension Slant {
    /// The system font of `style` at this Dynamic Type size, bold when asked.
    static func base(_ style: UIFont.TextStyle, bold: Bool, size: DynamicTypeSize) -> UIFont {
        let traits = UITraitCollection(preferredContentSizeCategory: UIContentSizeCategory(size))
        let f = UIFont.preferredFont(forTextStyle: style, compatibleWith: traits)
        guard bold, let d = f.fontDescriptor.withSymbolicTraits(f.fontDescriptor.symbolicTraits.union(.traitBold)) else { return f }
        return UIFont(descriptor: d, size: f.pointSize)
    }

    /// `text`'s *italic* runs leaning, in the font of `style` (bold for the
    /// whole line when `bold`, as headings are).
    static func italics(_ text: AttributedString, style: UIFont.TextStyle, bold: Bool = false, size: DynamicTypeSize) -> AttributedString {
        italics(text) { base(style, bold: bold || $0, size: size) as CTFont }
    }

    /// `font` leaning right (for the doc editor's text view).
    static func font(_ font: UIFont) -> UIFont {
        let leaning: CTFont = Slant.font(font as CTFont)
        return leaning as UIFont
    }
}
