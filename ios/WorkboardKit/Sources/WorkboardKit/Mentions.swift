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

    /// Everyone in the workspace, as mentioned_ids gets them (data.members):
    /// each full name, then each display name, since either mentions them.
    /// Looking a member up by id finds their full name first.
    public static func members(of data: JSON) -> [(id: UInt64, name: String)] {
        let all = data["members"]?.array ?? []
        let full = all.map { (id: $0["id"]?.uint ?? 0, name: $0["name"]?.string ?? "") }
        let display: [(id: UInt64, name: String)] = all.compactMap { m in
            let d = (m["display_name"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            let n = (m["name"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !d.isEmpty, d != n else { return nil }
            return (id: m["id"]?.uint ?? 0, name: d)
        }
        return full + display
    }

    /// The name others see a member by: their display name when they set
    /// one, else their full name (the Mac app's `Member::shown`).
    public static func shownName(_ m: JSON) -> String {
        let d = (m["display_name"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return d.isEmpty ? (m["name"]?.string ?? "") : d
    }

    /// mentions_in in src/model.rs (all members, nothing picked).
    public static func ids(in text: String, data: JSON, picked: [UInt64] = []) -> [UInt64] {
        ids(in: text, members: members(of: data), picked: picked)
    }

    // MARK: groups (wb_sync::mentions::chat_mentions)

    /// The words that mention everyone in a conversation, and who is online.
    public static let everyoneWords = ["channel", "everyone"]
    public static let hereWord = "here"

    /// Whether `text` says "@channel" / "@everyone", and whether it says
    /// "@here", by the rule names follow.
    public static func groupMentions(_ text: String) -> (all: Bool, here: Bool) {
        let t = Array(text.utf8)
        var all = false, here = false
        for i in t.indices where t[i] == UInt8(ascii: "@") {
            if i > 0 {
                let before = String(decoding: t[..<i], as: UTF8.self)
                guard before.unicodeScalars.last?.properties.isWhitespace == true else { continue }
            }
            let rest = Array(t[(i + 1)...])
            if everyoneWords.contains(where: { fits(rest, Array($0.utf8)) }) { all = true }
            if fits(rest, Array(hereWord.utf8)) { here = true }
        }
        return (all, here)
    }

    static func alnum(_ b: UInt8) -> Bool {
        (b >= 0x30 && b <= 0x39) || (b >= 0x41 && b <= 0x5A) || (b >= 0x61 && b <= 0x7A)
    }

    /// Whether `rest` (what follows an "@") starts with `name` by the mention rule.
    static func fits(_ rest: [UInt8], _ name: [UInt8]) -> Bool {
        guard !name.isEmpty, rest.starts(with: name) else { return false }
        let runsOn = name.count < rest.count && alnum(rest[name.count])
        return !(alnum(name[name.count - 1]) && runsOn)
    }

    /// The teams (extra.teams) by name, for "@Design".
    public static func teams(of data: JSON) -> [(name: String, members: [UInt64])] {
        (data.at("extra.teams")?.array ?? []).compactMap { t in
            let n = (t["name"]?.string ?? "").trimmingCharacters(in: .whitespaces)
            guard !n.isEmpty else { return nil }
            return (name: n, members: (t["members"]?.array ?? []).compactMap(\.uint))
        }
    }

    /// Everyone in a conversation: a public channel is everyone's (but those
    /// still invited), a private channel or DM its members'.
    public static func conversationPeople(_ data: JSON, channel: UInt64) -> [UInt64] {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel }) else { return [] }
        if c["kind"]?.string != "Direct" && c["private"]?.bool != true {
            return (data["members"]?.array ?? []).filter { $0["invited"]?.bool != true }.compactMap { $0["id"]?.uint }
        }
        return (c["members"]?.array ?? []).compactMap(\.uint)
    }

    /// Who a chat message written by `author` in `channel` mentions: people
    /// by name, team members for a team's name (a person's name of the same
    /// length wins), everyone here for "@channel", those of them `online`
    /// for "@here"; the writer left out of the groups. The same as the Mac
    /// app's, so the server notifies the same people.
    public static func chatIDs(in text: String, data: JSON, channel: UInt64, online: Set<UInt64>, author: UInt64, picked: [UInt64] = []) -> [UInt64] {
        let people = members(of: data)
        var out = ids(in: text, members: people, picked: picked)
        func add(_ id: UInt64) { if id != author && !out.contains(id) { out.append(id) } }
        let groups = teams(of: data)
        let t = Array(text.utf8)
        for i in t.indices where t[i] == UInt8(ascii: "@") {
            if i > 0 {
                let before = String(decoding: t[..<i], as: UTF8.self)
                guard before.unicodeScalars.last?.properties.isWhitespace == true else { continue }
            }
            let rest = Array(t[(i + 1)...])
            guard let best = groups.map({ Array($0.name.utf8) }).filter({ fits(rest, $0) }).map(\.count).max() else { continue }
            let person = people.map { Array($0.name.utf8) }.filter { fits(rest, $0) }.map(\.count).max() ?? 0
            if person >= best { continue }
            for g in groups where g.name.utf8.count == best && fits(rest, Array(g.name.utf8)) {
                g.members.forEach(add)
            }
        }
        let (all, here) = groupMentions(text)
        if all || here {
            for id in conversationPeople(data, channel: channel) where all || online.contains(id) { add(id) }
        }
        return out
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

    /// The groups the @ list offers for `query` after the people: "channel",
    /// "here" and each team, whose name starts with it.
    public static func groupCandidates(_ query: String, data: JSON) -> [String] {
        let q = query.lowercased()
        return (everyoneWords.prefix(1) + [hereWord] + teams(of: data).map(\.name)).filter { q.isEmpty || $0.lowercased().hasPrefix(q) }
    }

    /// The "@" `text` ends in is the one a pick from the list just filled
    /// in (`completed`: the text right after that pick), so the list stays
    /// closed for it while the text still starts with that pick.
    public static func isCompleted(_ text: String, after completed: String?) -> Bool {
        guard let c = completed, text.hasPrefix(c), let at = text.lastIndex(of: "@"), let cAt = c.lastIndex(of: "@") else { return false }
        return text.distance(from: text.startIndex, to: at) == c.distance(from: c.startIndex, to: cAt)
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
