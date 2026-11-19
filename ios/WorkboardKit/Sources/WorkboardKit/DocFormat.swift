// Doc formatting for the iPhone editor: the formatting bar's actions, list
// continuation on Return and the styled runs drawn while typing. Doc text
// stays Markdown, the same as the Mac (src/app/docs.rs: apply_block,
// apply_link, continue_list, inline_spans). Positions are UTF-16 offsets,
// as UITextView counts them.

import Foundation

public enum DocFormat {
    /// The formatting bar's buttons, in bar order.
    public enum Action: String, CaseIterable, Sendable {
        case heading1, heading2, heading3, bold, italic, strike, code, link
        case bullet, numbered, checklist, quote, codeBlock, divider

        /// The Markdown put at the start of the line, or around the selection.
        var prefix: String {
            switch self {
            case .heading1: return "# "
            case .heading2: return "## "
            case .heading3: return "### "
            case .bullet: return "- "
            case .numbered: return "1. "
            case .checklist: return "- [ ] "
            case .quote: return "> "
            case .codeBlock: return "```\n"
            case .divider: return "---\n"
            default: return ""
            }
        }

        var wrap: String {
            switch self {
            case .bold: return "**"
            case .italic: return "*"
            case .strike: return "~~"
            case .code: return "`"
            case .codeBlock: return "\n```"
            default: return ""
            }
        }
    }

    /// Line prefixes a block button replaces (longest match wins).
    static let linePrefixes = ["### ", "## ", "# ", "- [ ] ", "- [x] ", "- [X] ", "- ", "1. ", "> "]

    /// Applies `action` to `text` with the selection `selection`; returns the
    /// new text and the new selection.
    public static func apply(_ action: Action, to text: String, selection: NSRange) -> (String, NSRange) {
        let s = NSMutableString(string: text)
        let a = min(max(selection.location, 0), s.length)
        let b = min(a + max(selection.length, 0), s.length)
        switch action {
        case .bold, .italic, .strike, .code:
            let w = action.wrap as NSString
            s.insert(w as String, at: b)
            s.insert(w as String, at: a)
            // the wrapped words stay selected; an empty pair puts the caret inside
            return (s as String, NSRange(location: a + w.length, length: b - a))
        case .link:
            if a == b {
                s.insert("[](https://)", at: a)
                return (s as String, NSRange(location: a + 1, length: 0))
            }
            s.insert("](https://)", at: b)
            s.insert("[", at: a)
            // the caret goes where the address is typed
            return (s as String, NSRange(location: b + 1 + 2 + 8, length: 0))
        default:
            let ls = lineStart(s, a)
            let line = s.substring(from: ls)
            let prefix = action.prefix
            let existing = linePrefixes.first { line.hasPrefix($0) } ?? ""
            let strip = (existing as NSString).length
            s.replaceCharacters(in: NSRange(location: ls, length: strip), with: "")
            // the same button again takes the line back to plain text
            let toggleOff = existing == prefix && !prefix.hasSuffix("\n")
            let added = toggleOff ? 0 : (prefix as NSString).length
            if !toggleOff { s.insert(prefix, at: ls) }
            let c = max(a + added - strip, ls)
            if !action.wrap.isEmpty { s.insert(action.wrap, at: c) }
            let len = max(0, min(b - a, s.length - c))
            return (s as String, NSRange(location: c, length: action.wrap.isEmpty ? len : 0))
        }
    }

    /// Where the line holding position `at` starts.
    static func lineStart(_ s: NSString, _ at: Int) -> Int {
        let r = s.range(of: "\n", options: .backwards, range: NSRange(location: 0, length: at))
        return r.location == NSNotFound ? 0 : r.location + 1
    }

    /// Return typed at `cursor` (a newline is not yet in `text`): a list
    /// item's marker carries onto the new line (numbers count up) and an
    /// empty item ends the list. Returns the new text and caret.
    public static func newline(in text: String, at cursor: Int) -> (String, Int) {
        let s = NSMutableString(string: text)
        let c = min(max(cursor, 0), s.length)
        let ls = lineStart(s, c)
        let line = s.substring(with: NSRange(location: ls, length: c - ls))
        let indent = String(line.prefix { $0 == " " })
        let rest = String(line.dropFirst(indent.count))
        var marker: String?
        var pre = 0
        if rest.hasPrefix("- [ ] ") || rest.lowercased().hasPrefix("- [x] ") {
            marker = "- [ ] "; pre = 6
        } else if rest.hasPrefix("- ") || rest.hasPrefix("* ") {
            marker = String(rest.prefix(2)); pre = 2
        } else if line.hasPrefix("> ") {
            marker = "> "; pre = 2
        } else {
            let digits = rest.prefix { $0.isASCII && $0.isNumber }
            if !digits.isEmpty, rest.dropFirst(digits.count).hasPrefix(". "), let n = Int(digits) {
                marker = "\(n + 1). "; pre = digits.count + 2
            }
        }
        guard let m = marker else {
            s.insert("\n", at: c)
            return (s as String, c + 1)
        }
        let body = line.hasPrefix("> ") ? String(line.dropFirst(2)) : String(rest.dropFirst(pre))
        if body.isBlank {
            // Return on an empty item: drop its marker and stay on that line
            s.replaceCharacters(in: NSRange(location: ls, length: c - ls), with: "")
            return (s as String, ls)
        }
        let ins = "\n" + (m == "> " ? "" : indent) + m
        s.insert(ins, at: c)
        return (s as String, c + (ins as NSString).length)
    }

    /// How a run of the text is drawn while editing.
    public enum Style: Equatable, Sendable {
        case heading(Int), bold, italic, strike, code, link, quote, done, codeBlock
        /// Markdown punctuation, drawn faint
        case marker
    }

    public struct Run: Equatable, Sendable {
        public let style: Style
        public let range: NSRange
        public init(_ style: Style, _ range: NSRange) { self.style = style; self.range = range }
    }

    /// The styled runs of `text`: whole-line styles first (headings, quotes,
    /// ticked items, code blocks), then inline ones and markers, so later
    /// runs can be drawn over earlier ones.
    public static func runs(_ text: String) -> [Run] {
        let ns = text as NSString
        var out: [Run] = []
        var inCode = false
        var start = 0
        for line in text.components(separatedBy: "\n") {
            let len = (line as NSString).length
            let whole = NSRange(location: start, length: len)
            let t = line.trimmingCharacters(in: .whitespaces)
            defer { start += len + 1 }
            if t.hasPrefix("```") {
                inCode.toggle()
                out.append(Run(.marker, whole))
                continue
            }
            if inCode {
                out.append(Run(.codeBlock, whole))
                continue
            }
            if t == "---" {
                out.append(Run(.marker, whole))
                continue
            }
            var pre = 0
            if line.hasPrefix("### ") { out.append(Run(.heading(3), whole)); pre = 4 }
            else if line.hasPrefix("## ") { out.append(Run(.heading(2), whole)); pre = 3 }
            else if line.hasPrefix("# ") { out.append(Run(.heading(1), whole)); pre = 2 }
            else if line.hasPrefix("> ") { out.append(Run(.quote, whole)); pre = 2 }
            else {
                let ind = line.prefix { $0 == " " }.count
                let rest = line.dropFirst(ind)
                if rest.hasPrefix("- [ ] ") { pre = ind + 6 }
                else if rest.lowercased().hasPrefix("- [x] ") { out.append(Run(.done, whole)); pre = ind + 6 }
                else if rest.hasPrefix("- ") || rest.hasPrefix("* ") { pre = ind + 2 }
                else {
                    let digits = rest.prefix { $0.isASCII && $0.isNumber }.count
                    if digits > 0, rest.dropFirst(digits).hasPrefix(". ") { pre = ind + digits + 2 }
                }
            }
            if pre > 0 { out.append(Run(.marker, NSRange(location: start, length: pre))) }
            inline(ns, from: start + pre, to: start + len, into: &out)
        }
        return out
    }

    /// **bold**, *italic*, ~~strike~~, `code` and [text](url) links in one line.
    static func inline(_ ns: NSString, from: Int, to end: Int, into out: inout [Run]) {
        func has(_ mk: String, at i: Int) -> Bool {
            let n = (mk as NSString).length
            return i + n <= end && ns.substring(with: NSRange(location: i, length: n)) == mk
        }
        func find(_ mk: String, from i: Int) -> Int? {
            guard i < end else { return nil }
            let r = ns.range(of: mk, range: NSRange(location: i, length: end - i))
            return r.location == NSNotFound ? nil : r.location
        }
        var i = from
        while i < end {
            var matched = false
            for (mk, style) in [("`", Style.code), ("**", .bold), ("~~", .strike), ("*", .italic)] where has(mk, at: i) {
                let n = (mk as NSString).length
                if let j = find(mk, from: i + n), j > i + n {
                    out.append(Run(.marker, NSRange(location: i, length: n)))
                    out.append(Run(style, NSRange(location: i + n, length: j - i - n)))
                    out.append(Run(.marker, NSRange(location: j, length: n)))
                    i = j + n
                    matched = true
                }
                break
            }
            if !matched, has("[", at: i), let close = find("](", from: i + 1), close > i + 1,
               let paren = find(")", from: close + 2), paren > close + 2 {
                let label = ns.substring(with: NSRange(location: i + 1, length: close - i - 1))
                let url = ns.substring(with: NSRange(location: close + 2, length: paren - close - 2))
                if !label.contains("[") && !label.contains("]") && !url.contains(" ") {
                    out.append(Run(.marker, NSRange(location: i, length: 1)))
                    out.append(Run(.link, NSRange(location: i + 1, length: close - i - 1)))
                    out.append(Run(.marker, NSRange(location: close, length: paren - close + 1)))
                    i = paren + 1
                    matched = true
                }
            }
            if !matched { i += 1 }
        }
    }
}
