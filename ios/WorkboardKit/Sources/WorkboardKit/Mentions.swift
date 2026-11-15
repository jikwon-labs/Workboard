// Who a text @mentions: a byte-for-byte port of mentioned_ids in
// crates/wb-sync/src/mentions.rs (the rule the Mac app and the server share), so a message written or edited on the iPhone carries the
// same `mentions` the Mac app would give it.

import Foundation

public enum Mentions {
    /// At each "@" that starts a word the longest matching name wins, so
    /// "@Alex Kim" is Alex Kim and not Alex. A Latin name running on into
    /// more letters or digits ("@Alexander") is not that name; anything else
    /// may follow it, so "@지원님" still mentions 지원. When several people
    /// share the name, the ones in `picked` are meant, else all of them.
    public static func ids(in text: String, members: [(id: UInt64, name: String)], picked: [UInt64] = []) -> [UInt64] {
        let t = Array(text.utf8)
        let people = members.map { (id: $0.id, name: Array($0.name.utf8)) }
        let at = UInt8(ascii: "@")
        func alnum(_ b: UInt8) -> Bool {
            (b >= 0x30 && b <= 0x39) || (b >= 0x41 && b <= 0x5A) || (b >= 0x61 && b <= 0x7A)
        }
        var out: [UInt64] = []
        for i in t.indices where t[i] == at {
            if i > 0 {
                // text[..i] must end with whitespace (char::is_whitespace)
                let before = String(decoding: t[..<i], as: UTF8.self)
                guard before.unicodeScalars.last?.properties.isWhitespace == true else { continue }
            }
            let rest = t[(i + 1)...]
            func fits(_ name: [UInt8]) -> Bool {
                guard !name.isEmpty, rest.starts(with: name) else { return false }
                let next = rest.startIndex + name.count
                let runsOn = next < rest.endIndex && alnum(rest[next])
                return !(alnum(name[name.count - 1]) && runsOn)
            }
            guard let best = people.filter({ fits($0.name) }).map(\.name.count).max() else { continue }
            let named = people.filter { $0.name.count == best && fits($0.name) }.map(\.id)
            let chosen = named.filter { picked.contains($0) }
            for id in chosen.isEmpty ? named : chosen where !out.contains(id) {
                out.append(id)
            }
        }
        return out
    }

    /// Everyone in the workspace, as mentioned_ids gets them (data.members).
    public static func members(of data: JSON) -> [(id: UInt64, name: String)] {
        (data["members"]?.array ?? []).map { (id: $0["id"]?.uint ?? 0, name: $0["name"]?.string ?? "") }
    }

    /// mentions_in in src/model.rs (all members, nothing picked).
    public static func ids(in text: String, data: JSON, picked: [UInt64] = []) -> [UInt64] {
        ids(in: text, members: members(of: data), picked: picked)
    }

    // MARK: the @ list in the message box

    /// What is typed after an "@" that starts a word at the end of `text`
    /// (the part of a name typed so far, maybe empty), or nil when the text
    /// doesn't end in one. Names may hold spaces, so the query may too.
    public static func query(in text: String) -> String? {
        guard let at = text.lastIndex(of: "@") else { return nil }
        if at > text.startIndex {
            guard text[text.index(before: at)].isWhitespace else { return nil }
        }
        let q = text[text.index(after: at)...]
        guard q.count <= 40, !q.contains(where: \.isNewline), !q.hasPrefix(" ") else { return nil }
        return String(q)
    }

    /// The people the @ list offers for `query`: everyone but me and those
    /// still invited (as the Mac app's list), whose name starts with it.
    public static func candidates(_ query: String, members: [MemberView], me: UInt64?) -> [MemberView] {
        let q = query.lowercased()
        return members.filter { m in
            m.id != me && !m.invited && !m.name.isEmpty && (q.isEmpty || m.name.lowercased().hasPrefix(q))
        }
    }

    /// `text` with the "@query" it ends in replaced by "@<name> ".
    public static func complete(_ text: String, with name: String) -> String {
        guard query(in: text) != nil, let at = text.lastIndex(of: "@") else { return text + "@" + name + " " }
        return String(text[..<at]) + "@" + name + " "
    }

    /// Sets `mentions` on a message or comment, left out when empty (as the
    /// Mac app writes it: skip_serializing_if = "Vec::is_empty").
    static func put(_ ids: [UInt64], on j: inout JSON) {
        j["mentions"] = ids.isEmpty ? JSON?.none : .array(ids.map(JSON.id))
    }
}
