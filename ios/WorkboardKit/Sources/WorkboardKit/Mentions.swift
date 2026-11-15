// Who a text @mentions: a byte-for-byte port of mentioned_ids in
// src/model.rs, so a message written or edited on the iPhone carries the
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

    /// mentions_in in src/model.rs.
    public static func ids(in text: String, data: JSON, picked: [UInt64] = []) -> [UInt64] {
        ids(in: text, members: members(of: data), picked: picked)
    }

    /// Sets `mentions` on a message or comment, left out when empty (as the
    /// Mac app writes it: skip_serializing_if = "Vec::is_empty").
    static func put(_ ids: [UInt64], on j: inout JSON) {
        j["mentions"] = ids.isEmpty ? JSON?.none : .array(ids.map(JSON.id))
    }
}
