// The Inbox, worked out from the workspace the way the Mac app does
// (inbox_items in src/app/nav.rs): comments on tasks I'm assigned to or
// made, @mentions of me in task comments and chat, tasks assigned to me, and
// chat replies in threads I started or wrote in. My own actions are left
// out. Rows go to Primary, Other, Later or Cleared; Later and Cleared are
// my own (`extra.inbox_later`, `extra.inbox_cleared`, never shared with the
// team), kept in step with my other devices (Mine.swift) and keyed like the
// Mac's rows so the keys mean the same thing.

import Foundation

public struct InboxItem: Identifiable, Hashable, Sendable {
    public enum Source: Hashable, Sendable {
        case task(UInt64)
        case chat(channel: UInt64, message: UInt64, thread: UInt64?)
    }

    public enum Tab: Int, CaseIterable, Sendable {
        case primary = 0, other, later, cleared
        public var title: String { ["Primary", "Other", "Later", "Cleared"][rawValue] }
    }

    public let key: String
    public let ts: Int64
    public let author: UInt64
    public let title: String
    public let text: String
    public let source: Source
    public let comment: Bool
    public let mention: Bool
    public let assigned: Bool
    public let reply: Bool
    public let tab: Tab
    public var id: String { key }
}

public enum Inbox {
    /// my_task in src/app/nav.rs: one I'm assigned to, or one I made (new or
    /// as a copy).
    static func isMine(_ t: JSON, me: UInt64) -> Bool {
        if (t["assignees"]?.array ?? []).contains(where: { $0.uint == me }) { return true }
        return (t["activity"]?.array ?? []).contains { a in
            a["author"]?.uint == me && a["comment"]?.bool != true
                && ((a["text"]?.string ?? "") == "created this task" || (a["text"]?.string ?? "").hasPrefix("duplicated this task from "))
        }
    }

    /// Every Inbox row, newest first.
    public static func items(_ data: JSON, me: UInt64) -> [InboxItem] {
        let members = Mentions.members(of: data)
        let myName = members.first { $0.id == me }?.name ?? ""
        let cleared = Set((data.at("extra.inbox_cleared")?.array ?? []).compactMap(\.string))
        let later = Set((data.at("extra.inbox_later")?.array ?? []).compactMap(\.string))
        func tab(_ key: String, primary: Bool) -> InboxItem.Tab {
            cleared.contains(key) ? .cleared : later.contains(key) ? .later : primary ? .primary : .other
        }
        // imported history starts out cleared for everyone, unless saved for later
        func importedTab(_ key: String) -> InboxItem.Tab { later.contains(key) ? .later : .cleared }
        // my notification settings (kept in step with the Mac) decide what is
        // Primary; one the Mac hasn't sent is on, as its defaults are
        let notify = data.at("extra.notify")
        func on(_ k: String) -> Bool { notify?[k]?.bool ?? true }
        let (commentsOn, mentionsOn, assignedOn) = (on("comments"), on("mentions"), on("assigned"))
        func mentions(_ text: String, _ ids: [UInt64]) -> Bool {
            ids.isEmpty ? Mentions.ids(in: text, members: members).contains(me) : ids.contains(me)
        }
        var out: [InboxItem] = []
        for t in data["tasks"]?.array ?? [] {
            guard let tid = t["id"]?.uint else { continue }
            let mine = isMine(t, me: me)
            for a in t["activity"]?.array ?? [] {
                let author = a["author"]?.uint ?? 0
                guard author != me else { continue }
                let comment = a["comment"]?.bool ?? false
                let text = a["text"]?.string ?? ""
                let mention = comment && mentions(text, (a["mentions"]?.array ?? []).compactMap(\.uint))
                let assigned: Bool
                if comment { assigned = false } else if let who = a["target"]?.uint { assigned = who == me && text.hasPrefix("assigned to ") } else { assigned = !myName.isEmpty && text == "assigned to \(myName)" }
                let primary = (comment && mine && commentsOn) || (mention && mentionsOn) || (assigned && assignedOn)
                let ts = a["ts"]?.int ?? 0
                let key = "\(tid):\(ts):\(author)"
                out.append(InboxItem(key: key, ts: ts, author: author, title: t["name"]?.string ?? "", text: text, source: .task(tid),
                                     comment: comment, mention: mention, assigned: assigned, reply: false, tab: tab(key, primary: primary)))
            }
        }
        for c in data["channels"]?.array ?? [] {
            guard let cid = c["id"]?.uint else { continue }
            let direct = c["kind"]?.string == "Direct"
            let members_ = (c["members"]?.array ?? []).compactMap(\.uint)
            if (direct || c["private"]?.bool == true) && !members_.contains(me) { continue }
            let title = direct ? (members.first { $0.id == (members_.first { $0 != me } ?? me) }?.name ?? "") : "#\(c["name"]?.string ?? "")"
            for m in c["messages"]?.array ?? [] {
                guard let mid = m["id"]?.uint else { continue }
                let author = m["author"]?.uint ?? 0
                let imported = MessageView.isImported(m)
                // (the channel's copy of a thread reply: the reply is in the Inbox)
                if author != me, m["thread_of"]?.uint == nil, mentions(m["text"]?.string ?? "", (m["mentions"]?.array ?? []).compactMap(\.uint)) {
                    let key = "chat:\(cid):\(mid)"
                    out.append(InboxItem(key: key, ts: m["ts"]?.int ?? 0, author: author, title: title, text: m["text"]?.string ?? "",
                                         source: .chat(channel: cid, message: mid, thread: nil), comment: true, mention: true, assigned: false, reply: false,
                                         tab: imported ? importedTab(key) : tab(key, primary: mentionsOn)))
                }
                let replies = m["replies"]?.array ?? []
                for (i, r) in replies.enumerated() {
                    let ra = r["author"]?.uint ?? 0
                    guard ra != me, let rid = r["id"]?.uint else { continue }
                    let mention = mentions(r["text"]?.string ?? "", (r["mentions"]?.array ?? []).compactMap(\.uint))
                    let reply = author == me || replies[..<i].contains { $0["author"]?.uint == me }
                    guard mention || reply else { continue }
                    let key = "chat:\(cid):\(rid)"
                    out.append(InboxItem(key: key, ts: r["ts"]?.int ?? 0, author: ra, title: title, text: r["text"]?.string ?? "",
                                         source: .chat(channel: cid, message: rid, thread: mid), comment: true, mention: mention, assigned: false, reply: reply,
                                         tab: MessageView.isImported(r) ? importedTab(key) : tab(key, primary: (mention && mentionsOn) || reply)))
                }
            }
        }
        return out.sorted { $0.ts > $1.ts }
    }

    /// The row's text under its title (inbox_line).
    public static func line(_ item: InboxItem, author: String) -> String {
        switch item.source {
        case .chat(_, _, let thread) where thread != nil && !item.mention: return "\(author) replied: \(item.text)"
        case .chat: return "\(author) mentioned you: \(item.text)"
        case .task where item.comment: return "\(author) commented: \(item.text)"
        case .task: return "\(author) \(item.text)"
        }
    }

    /// Moves rows to Later or Cleared, or back to where they belong (nil).
    /// My own: changed with `SyncClient.editLocal`, which sends it to my
    /// other devices (Mine.swift).
    public static func move(_ data: inout JSON, keys: [String], to tab: InboxItem.Tab?) {
        var cleared = (data.at("extra.inbox_cleared")?.array ?? []).compactMap(\.string)
        var later = (data.at("extra.inbox_later")?.array ?? []).compactMap(\.string)
        cleared.removeAll { keys.contains($0) }
        later.removeAll { keys.contains($0) }
        if tab == .cleared { cleared += keys }
        if tab == .later { later += keys }
        data.set("extra.inbox_cleared", .array(cleared.map(JSON.string)))
        data.set("extra.inbox_later", .array(later.map(JSON.string)))
    }
}
