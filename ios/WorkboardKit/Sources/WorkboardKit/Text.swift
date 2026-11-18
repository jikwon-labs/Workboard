// Trimming typed text.

import Foundation

extension StringProtocol {
    /// Without spaces and newlines at either end.
    public var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }

    /// Empty, or nothing but spaces and newlines.
    public var isBlank: Bool { trimmed.isEmpty }
}
