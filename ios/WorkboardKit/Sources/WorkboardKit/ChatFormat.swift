// Formatting in chat messages, as the Mac app reads it (src/app/chatfmt.rs):
// the Markdown the Docs use inside a line (**bold**, *italic*, ~~strike~~,
// `code`, [text](url), bare web addresses), ``` code blocks, "> " quotes
// and "- " lists. And link previews (src/linkpreview.rs): the title,
// description and site of the page a message's first web address points
// to, fetched once by the writer's app and kept on the message.

import Foundation

public enum ChatBlock: Equatable, Sendable {
    /// lines of text with inline marks
    case text(String)
    /// the lines of a ``` block, as typed
    case code(String)
    /// a "> " line's text (inline marks)
    case quote(String)
    /// a "- " line's text (inline marks)
    case bullet(String)
}

public enum ChatFormat {
    /// The blocks a message is shown as, in order; neighbouring text lines
    /// stay together.
    public static func blocks(_ src: String) -> [ChatBlock] {
        var out: [ChatBlock] = []
        var code: [String]?
        func addText(_ line: String) {
            if case .text(let t)? = out.last {
                out[out.count - 1] = .text(t + "\n" + line)
            } else {
                out.append(.text(line))
            }
        }
        for line in src.components(separatedBy: "\n") {
            let trimmed = line.drop(while: { $0 == " " || $0 == "\t" })
            if trimmed.hasPrefix("```") {
                let after = trimmed.dropFirst(3)
                if code == nil, after.hasSuffix("```"), after.count > 3 {
                    out.append(.code(String(after.dropLast(3))))
                    continue
                }
                if let lines = code {
                    out.append(.code(lines.joined(separator: "\n")))
                    code = nil
                } else {
                    code = []
                }
                continue
            }
            if code != nil {
                code!.append(line)
            } else if line.hasPrefix("> ") {
                out.append(.quote(String(line.dropFirst(2))))
            } else if line == ">" {
                out.append(.quote(""))
            } else if line.hasPrefix("- ") || line.hasPrefix("* ") {
                out.append(.bullet(String(line.dropFirst(2))))
            } else {
                addText(line)
            }
        }
        // a block never closed shows what it has
        if let lines = code { out.append(.code(lines.joined(separator: "\n"))) }
        return out
    }

    /// One line's inline marks as an AttributedString: Apple's Markdown
    /// reading, with bare web addresses made links first.
    public static func inline(_ s: String) -> AttributedString {
        let linked = autolinked(s)
        let opts = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        return (try? AttributedString(markdown: linked, options: opts)) ?? AttributedString(s)
    }

    /// Bare http(s) addresses at the start of a word wrapped in <...>
    /// (Markdown's autolinks), outside `code`; \ and _ escaped, since the
    /// Mac shows them as typed (no escapes, italic only with *).
    static func autolinked(_ s: String) -> String {
        var out = ""
        var i = s.startIndex
        var inCode = false
        while i < s.endIndex {
            let c = s[i]
            if c == "`" { inCode.toggle() }
            let wordStart = i == s.startIndex || s[s.index(before: i)].isWhitespace
            let rest = s[i...]
            if !inCode, wordStart, rest.hasPrefix("http://") || rest.hasPrefix("https://") {
                let end = rest.firstIndex(where: \.isWhitespace) ?? s.endIndex
                var url = String(s[i..<end])
                var tail = ""
                while let last = url.last, ".,);:!?".contains(last) {
                    tail = String(last) + tail
                    url.removeLast()
                }
                out += "<\(url)>" + tail
                i = end
                continue
            }
            // the Mac reads no \ escapes and no _italic_: shown as typed
            if !inCode && (c == "\\" || c == "_") {
                out.append("\\")
            }
            out.append(c)
            i = s.index(after: i)
        }
        return out
    }

    /// The first web address of a message, not counting code (for its preview).
    public static func firstURL(_ src: String) -> String? {
        for b in blocks(src) {
            let line: String
            switch b {
            case .text(let t), .quote(let t), .bullet(let t): line = t
            case .code: continue
            }
            for run in inline(line).runs {
                if let url = run.link, let s = Optional(url.absoluteString), s.hasPrefix("http://") || s.hasPrefix("https://") {
                    return s
                }
            }
        }
        return nil
    }

    /// `mark` around the selected characters `a..<b` of `text` (two marks
    /// with the cursor between them when nothing is selected; a ``` block
    /// for "`" over several lines), or the marks taken off when they are
    /// already there. Returns the new text and selection (chatfmt::wrap).
    public static func wrap(_ text: String, _ a: Int, _ b: Int, mark: String) -> (String, Int, Int) {
        var chars = Array(text)
        let (a, b) = (min(a, b, chars.count), min(max(a, b), chars.count))
        let multi = mark == "`" && chars[a..<b].contains("\n")
        let open = Array(multi ? "```\n" : mark), close = Array(multi ? "\n```" : mark)
        func run(_ s: ArraySlice<Character>, back: Bool) -> Int {
            back ? s.reversed().prefix(while: { $0 == "*" }).count : s.prefix(while: { $0 == "*" }).count
        }
        let present: Bool
        if a == b {
            present = false
        } else if mark == "*" {
            present = run(chars[..<a], back: true) % 2 == 1 && run(chars[b...], back: false) % 2 == 1
        } else if mark == "**" {
            present = run(chars[..<a], back: true) >= 2 && run(chars[b...], back: false) >= 2
        } else {
            present = a >= open.count && Array(chars[(a - open.count)..<a]) == open
                && b + close.count <= chars.count && Array(chars[b..<(b + close.count)]) == close
        }
        if present {
            chars.removeSubrange(b..<(b + close.count))
            chars.removeSubrange((a - open.count)..<a)
            return (String(chars), a - open.count, b - open.count)
        }
        chars.insert(contentsOf: close, at: b)
        chars.insert(contentsOf: open, at: a)
        return (String(chars), a + open.count, b + open.count)
    }
}

// MARK: link previews

public enum LinkPreviews {
    static let maxBytes = 512 * 1024

    /// The preview of `url` (Message::preview: {url, title, description,
    /// site}), or nil when the page can't be read or says nothing.
    public static func fetch(_ url: String) async -> JSON? {
        guard url.hasPrefix("http://") || url.hasPrefix("https://"), let u = URL(string: url) else { return nil }
        var req = URLRequest(url: u, timeoutInterval: 6)
        req.setValue("Mozilla/5.0 (compatible; Workboard link preview)", forHTTPHeaderField: "User-Agent")
        req.setValue("text/html,application/xhtml+xml", forHTTPHeaderField: "Accept")
        guard let (data, _) = try? await URLSession.shared.data(for: req) else { return nil }
        let html = String(decoding: data.prefix(maxBytes), as: UTF8.self)
        return fromHTML(url: url, html: html)
    }

    /// A page's preview from its <meta> tags (Open Graph, Twitter, description)
    /// or its <title> (linkpreview::from_html).
    public static func fromHTML(url: String, html: String) -> JSON? {
        let head: Substring = html.range(of: "</head", options: .caseInsensitive).map { html[..<$0.lowerBound] } ?? html[...]
        let metas = metaTags(head)
        func meta(_ keys: [String]) -> String? {
            for k in keys {
                if let c = metas.first(where: { $0.0.caseInsensitiveCompare(k) == .orderedSame })?.1,
                   !c.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return c }
            }
            return nil
        }
        let title = clean(meta(["og:title", "twitter:title"]) ?? titleTag(head) ?? "", 200)
        let desc = clean(meta(["og:description", "twitter:description", "description"]) ?? "", 300)
        if title.isEmpty && desc.isEmpty { return nil }
        let site = clean(meta(["og:site_name"]) ?? host(url), 80)
        return .object(["url": .string(url), "title": .string(title), "description": .string(desc), "site": .string(site)])
    }

    public static func host(_ url: String) -> String {
        let rest = url.range(of: "://").map { url[$0.upperBound...] } ?? url[...]
        var h = rest.split(whereSeparator: { "/?#".contains($0) }).first.map(String.init) ?? ""
        if let at = h.lastIndex(of: "@") { h = String(h[h.index(after: at)...]) }
        return h.hasPrefix("www.") ? String(h.dropFirst(4)) : h
    }

    static func metaTags(_ head: Substring) -> [(String, String)] {
        var out: [(String, String)] = []
        var from = head.startIndex
        while let r = head.range(of: "<meta", options: .caseInsensitive, range: from..<head.endIndex) {
            let end = head[r.upperBound...].firstIndex(of: ">") ?? head.endIndex
            let tag = head[r.lowerBound..<end]
            if let n = attr(tag, "property") ?? attr(tag, "name"), let c = attr(tag, "content") { out.append((n, c)) }
            from = end
        }
        return out
    }

    static func attr(_ tag: Substring, _ name: String) -> String? {
        var from = tag.startIndex
        while let r = tag.range(of: name, options: .caseInsensitive, range: from..<tag.endIndex) {
            from = r.upperBound
            guard r.lowerBound == tag.startIndex || tag[tag.index(before: r.lowerBound)].isWhitespace else { continue }
            var i = r.upperBound
            while i < tag.endIndex, tag[i].isWhitespace { i = tag.index(after: i) }
            guard i < tag.endIndex, tag[i] == "=" else { continue }
            i = tag.index(after: i)
            while i < tag.endIndex, tag[i].isWhitespace { i = tag.index(after: i) }
            guard i < tag.endIndex else { return "" }
            let q = tag[i]
            if q == "\"" || q == "'" {
                let s = tag.index(after: i)
                let e = tag[s...].firstIndex(of: q) ?? tag.endIndex
                return decode(String(tag[s..<e]))
            }
            let e = tag[i...].firstIndex(where: { $0.isWhitespace || $0 == "/" }) ?? tag.endIndex
            return decode(String(tag[i..<e]))
        }
        return nil
    }

    static func titleTag(_ head: Substring) -> String? {
        guard let r = head.range(of: "<title", options: .caseInsensitive),
              let open = head[r.upperBound...].firstIndex(of: ">"),
              let close = head.range(of: "</title", options: .caseInsensitive, range: open..<head.endIndex) else { return nil }
        return decode(String(head[head.index(after: open)..<close.lowerBound]))
    }

    static func decode(_ s: String) -> String {
        var out = ""
        var rest = Substring(s)
        while let amp = rest.firstIndex(of: "&") {
            out += rest[..<amp]
            let after = rest[amp...]
            if let semi = after.prefix(12).firstIndex(of: ";") {
                let name = String(after[after.index(after: amp)..<semi])
                var ch: Character?
                switch name {
                case "amp": ch = "&"
                case "lt": ch = "<"
                case "gt": ch = ">"
                case "quot": ch = "\""
                case "apos", "#39": ch = "'"
                case "nbsp": ch = " "
                default:
                    if name.hasPrefix("#x") || name.hasPrefix("#X") {
                        ch = UInt32(name.dropFirst(2), radix: 16).flatMap(Unicode.Scalar.init).map(Character.init)
                    } else if name.hasPrefix("#") {
                        ch = UInt32(name.dropFirst()).flatMap(Unicode.Scalar.init).map(Character.init)
                    }
                }
                if let ch {
                    out.append(ch)
                    rest = after[after.index(after: semi)...]
                    continue
                }
            }
            out.append("&")
            rest = after.dropFirst()
        }
        out += rest
        return out
    }

    static func clean(_ s: String, _ max: Int) -> String {
        let one = s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        return one.count > max ? String(one.prefix(max)).trimmingCharacters(in: .whitespaces) + "..." : one
    }
}

extension Edits {
    /// A fetched preview onto my message, unless its first address changed meanwhile.
    @discardableResult
    public static func setPreview(_ data: inout JSON, channel: UInt64, message: UInt64, preview: JSON, me: UInt64) -> Bool {
        changeMessage(&data, channel: channel, message: message) { m in
            guard m["author"]?.uint == me, ChatFormat.firstURL(m["text"]?.string ?? "") == preview["url"]?.string else { return }
            m["preview"] = preview
        }
    }

    /// The writer takes the preview off their message.
    @discardableResult
    public static func removePreview(_ data: inout JSON, channel: UInt64, message: UInt64, me: UInt64) -> Bool {
        changeMessage(&data, channel: channel, message: message) { m in
            guard m["author"]?.uint == me else { return }
            m["preview"] = nil
        }
    }
}
