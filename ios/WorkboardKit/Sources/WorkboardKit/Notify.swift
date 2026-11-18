// What to notify about, as the Mac app decides it (NotifySettings in
// src/model.rs, notify_candidates in src/app/notify.rs): direct messages,
// channels set to everything, huddles, reactions to my messages, and the
// Inbox's mentions, thread replies, assignments, comments and changes on my
// tasks, plus answers to my forms and people joining (for owners and
// admins). The choices are each person's own and stay on the device, like
// the Mac's.

import Foundation

public struct NotifyPrefs: Codable, Equatable, Sendable {
    public var direct = true
    public var mentions = true
    /// Replies in threads I started or wrote in.
    public var replies = true
    public var huddles = true
    /// Reactions to my messages.
    public var reactions = false
    public var assigned = true
    /// Comments on tasks I'm on or made.
    public var comments = true
    /// Someone else changes the status, due date or priority of my tasks,
    /// or takes me off one.
    public var taskChanges = true
    /// Once a day at `dueTime`: my tasks due today and overdue.
    public var due = true
    public var dueTime = "09:00"
    public var meetings = true
    public var meetingMinutes = 10
    /// My goals due today.
    public var goals = true
    public var forms = true
    /// Someone joins the workspace (owners and admins).
    public var team = true
    public var sound = true
    /// The number on the app icon: Primary Inbox rows plus unread direct messages.
    public var badge = true
    /// Channel messages: "all", "mentions" or "nothing".
    public var channelDefault = "mentions"
    /// Channels set apart from `channelDefault`.
    public var channelModes: [UInt64: String] = [:]
    /// No notifications until then (seconds since 1970).
    public var mutedUntil: Int64 = 0

    public init() {}

    /// Every kind on and every channel loud: what exists at all (so a later
    /// change of mind doesn't bring back old news).
    public static var everything: NotifyPrefs {
        var p = NotifyPrefs()
        p.reactions = true
        p.channelDefault = "all"
        return p
    }

    public func channelMode(_ id: UInt64) -> String { channelModes[id] ?? channelDefault }

    public mutating func setChannelMode(_ id: UInt64, _ mode: String) {
        channelModes[id] = mode == channelDefault ? nil : mode
    }

    public func muted(now: Int64) -> Bool { mutedUntil > now }

    // a file saved by an older build may lack newer fields
    public init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func v<T: Decodable>(_ k: CodingKeys, _ d: T) -> T { (try? c.decodeIfPresent(T.self, forKey: k)) ?? d }
        direct = v(.direct, direct); mentions = v(.mentions, mentions); replies = v(.replies, replies)
        huddles = v(.huddles, huddles); reactions = v(.reactions, reactions); assigned = v(.assigned, assigned)
        comments = v(.comments, comments); taskChanges = v(.taskChanges, taskChanges); due = v(.due, due)
        dueTime = v(.dueTime, dueTime); meetings = v(.meetings, meetings); meetingMinutes = v(.meetingMinutes, meetingMinutes)
        goals = v(.goals, goals); forms = v(.forms, forms); team = v(.team, team); sound = v(.sound, sound)
        badge = v(.badge, badge); channelDefault = v(.channelDefault, channelDefault)
        channelModes = v(.channelModes, channelModes); mutedUntil = v(.mutedUntil, mutedUntil)
    }
}

/// One thing to tell the person about.
public struct Note: Equatable, Sendable {
    public enum Kind: Sendable { case direct, channel, huddle, reaction, mention, reply, assigned, comment, taskChange, form, member }
    public enum Target: Equatable, Sendable {
        case channel(UInt64, thread: UInt64?)
        case task(UInt64)
        case people
    }

    public let key: String
    public let kind: Kind
    /// In English (as the Mac writes it); `who` and `subject` let a screen
    /// put it in its own language.
    public let title: String
    public let body: String
    public let target: Target
    /// When it happened; nil for things without a time (reactions, joins).
    public let ts: Int64?
    /// The person it is about, and the place or task ("#general", a task
    /// name), or for a reaction its emoji.
    public var who = ""
    public var subject = ""

    public init(key: String, kind: Kind, title: String, body: String, target: Target, ts: Int64?, who: String = "", subject: String = "") {
        self.key = key; self.kind = kind; self.title = title; self.body = body; self.target = target; self.ts = ts; self.who = who; self.subject = subject
    }
}

public enum Notify {
    /// How long a message stays news (FRESH_SECS), and a huddle (HUDDLE_FRESH_SECS).
    public static let freshSecs: Int64 = 24 * 3600
    public static let huddleFreshSecs: Int64 = 5 * 60
    /// More than this many at once go out as one summary (BURST).
    public static let burst = 3

    static func preview(_ m: JSON) -> String {
        let t = m["text"]?.string ?? ""
        if !t.isEmpty { return String(t.prefix(140)) }
        return (m["attachments"]?.array ?? []).isEmpty ? "" : "Sent a photo or video"
    }

    /// notify_candidates: everything the choices ask to be told about.
    /// `manager`: I'm an owner or admin (people joining). `inbox`: the
    /// Inbox rows when the caller has them already (SyncClient.inboxItems).
    public static func candidates(_ data: JSON, prefs n: NotifyPrefs, me: UInt64, manager: Bool, inbox: [InboxItem]? = nil) -> [Note] {
        let members = Mentions.members(of: data)
        let who = { (id: UInt64) in members.first { $0.id == id }?.name ?? "Someone" }
        var out: [Note] = []
        var keys = Set<String>()
        func add(_ note: Note) { if keys.insert(note.key).inserted { out.append(note) } }

        // chat: direct messages, channels set to everything, huddles, reactions
        for c in data["channels"]?.array ?? [] {
            guard let cid = c["id"]?.uint else { continue }
            let dm = c["kind"]?.string == "Direct"
            let people = (c["members"]?.array ?? []).compactMap(\.uint)
            if (dm || c["private"]?.bool == true) && !people.contains(me) { continue }
            let mode = dm ? "all" : n.channelMode(cid)
            if mode == "nothing" { continue }
            let place = dm ? who(people.first { $0 != me } ?? me) : "#\(c["name"]?.string ?? "")"
            for m in c["messages"]?.array ?? [] {
                guard let mid = m["id"]?.uint else { continue }
                let author = m["author"]?.uint ?? 0
                let key = "chat:\(cid):\(mid)"
                let ts = m["ts"]?.int ?? 0
                if author != me && !MessageView.isImported(m) {
                    if m["call"]?.string != nil {
                        if n.huddles {
                            add(Note(key: key, kind: .huddle, title: dm ? "\(who(author)) is calling you" : "\(who(author)) started a huddle in \(place)",
                                     body: "Join from the conversation", target: .channel(cid, thread: nil), ts: ts, who: who(author), subject: dm ? "" : place))
                        }
                    } else if dm {
                        if n.direct { add(Note(key: key, kind: .direct, title: who(author), body: preview(m), target: .channel(cid, thread: nil), ts: ts, who: who(author))) }
                    } else if mode == "all" {
                        add(Note(key: key, kind: .channel, title: "\(who(author)) in \(place)", body: preview(m), target: .channel(cid, thread: nil), ts: ts, who: who(author), subject: place))
                    }
                }
                let replies = m["replies"]?.array ?? []
                if dm && n.direct {
                    for r in replies where (r["author"]?.uint ?? 0) != me && !MessageView.isImported(r) {
                        guard let rid = r["id"]?.uint else { continue }
                        add(Note(key: "chat:\(cid):\(rid)", kind: .direct, title: who(r["author"]?.uint ?? 0), body: preview(r), target: .channel(cid, thread: mid), ts: r["ts"]?.int, who: who(r["author"]?.uint ?? 0)))
                    }
                }
                if n.reactions {
                    for (msg, thread) in [(m, UInt64?.none)] + replies.map({ ($0, UInt64?.some(mid)) }) where msg["author"]?.uint == me {
                        for re in msg["reactions"]?.array ?? [] {
                            let emoji = re["emoji"]?.string ?? ""
                            for u in (re["users"]?.array ?? []).compactMap(\.uint) where u != me {
                                add(Note(key: "react:\(cid):\(msg["id"]?.uint ?? 0):\(emoji):\(u)", kind: .reaction, title: "\(who(u)) reacted \(emoji) to your message",
                                         body: preview(msg), target: .channel(cid, thread: thread), ts: nil, who: who(u), subject: emoji))
                            }
                        }
                    }
                }
            }
        }

        // the Inbox: mentions, thread replies, assignments, comments and changes on my tasks
        let myName = who(me)
        let tasks = Dictionary((data["tasks"]?.array ?? []).compactMap { t in t["id"]?.uint.map { ($0, t) } }, uniquingKeysWith: { a, _ in a })
        let channels = data["channels"]?.array ?? []
        for it in inbox ?? Inbox.items(data, me: me) where it.author != me {
            let key: String, target: Note.Target
            var task: JSON?
            switch it.source {
            case .task(let id):
                // two lines in the same second by the same person share an Inbox key
                key = "task:\(it.key):\(it.text)"
                target = .task(id)
                task = tasks[id]
            case .chat(let ch, _, let thread):
                if channels.contains(where: { $0["id"]?.uint == ch && $0["kind"]?.string == "Channel" }) && n.channelMode(ch) == "nothing" { continue }
                if isImported(data, it) { continue }
                key = it.key
                target = .channel(ch, thread: thread)
            }
            let mine = task.map { Inbox.isMine($0, me: me) } ?? false
            let note: Note
            if it.mention {
                guard n.mentions else { continue }
                note = Note(key: key, kind: .mention, title: "\(who(it.author)) mentioned you in \(it.title)", body: String(it.text.prefix(140)), target: target, ts: it.ts, who: who(it.author), subject: it.title)
            } else if it.reply {
                guard n.replies else { continue }
                note = Note(key: key, kind: .reply, title: "\(who(it.author)) replied in \(it.title)", body: String(it.text.prefix(140)), target: target, ts: it.ts, who: who(it.author), subject: it.title)
            } else if it.assigned {
                guard n.assigned else { continue }
                note = Note(key: key, kind: .assigned, title: "\(who(it.author)) assigned you a task", body: it.title, target: target, ts: it.ts, who: who(it.author), subject: it.title)
            } else if it.comment && task != nil {
                guard n.comments && mine else { continue }
                note = Note(key: key, kind: .comment, title: "\(who(it.author)) commented on \(it.title)", body: String(it.text.prefix(140)), target: target, ts: it.ts, who: who(it.author), subject: it.title)
            } else if task != nil {
                let t = it.text
                let change = t.hasPrefix("changed status") || t.hasPrefix("changed due date") || t.hasPrefix("set priority")
                    || (!myName.isEmpty && t == "removed assignee \(myName)")
                guard n.taskChanges && mine && change else { continue }
                note = Note(key: key, kind: .taskChange, title: it.title, body: "\(who(it.author)) \(t)", target: target, ts: it.ts, who: who(it.author), subject: t)
            } else {
                continue
            }
            add(note)
        }

        // people joining (for owners and admins)
        if n.team && manager {
            for m in data["members"]?.array ?? [] {
                guard let id = m["id"]?.uint, id != me, m["invited"]?.bool != true else { continue }
                add(Note(key: "member:\(id)", kind: .member, title: "\(m["name"]?.string ?? "") joined \(data["workspace"]?.string ?? "")",
                         body: "Say hello in a direct message", target: .people, ts: nil, who: m["name"]?.string ?? "", subject: data["workspace"]?.string ?? ""))
            }
        }

        // answers to my forms
        if n.forms {
            let mineForms = Set((data.at("extra.forms")?.array ?? []).filter { $0["owner"]?.uint == me }.compactMap { $0["name"]?.string })
            if !mineForms.isEmpty {
                for t in data["tasks"]?.array ?? [] {
                    guard let tid = t["id"]?.uint else { continue }
                    for a in t["activity"]?.array ?? [] where a["author"]?.uint != me && a["comment"]?.bool != true {
                        let text = a["text"]?.string ?? ""
                        guard text.hasPrefix("submitted the form ") else { continue }
                        let name = String(text.dropFirst("submitted the form ".count))
                        guard mineForms.contains(name) else { continue }
                        add(Note(key: "form:\(tid)", kind: .form, title: "New answer to \(name)", body: "\(who(a["author"]?.uint ?? 0)) answered: \(t["name"]?.string ?? "")",
                                 target: .task(tid), ts: a["ts"]?.int, who: who(a["author"]?.uint ?? 0), subject: name))
                    }
                }
            }
        }
        return out
    }

    /// An Inbox row from imported (Slack) history: old news, never announced.
    static func isImported(_ data: JSON, _ it: InboxItem) -> Bool {
        guard case .chat(let ch, let msg, let thread) = it.source,
              let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == ch }) else { return false }
        let ms = c["messages"]?.array ?? []
        let m = thread == nil ? ms.first { $0["id"]?.uint == msg } : ms.first { $0["id"]?.uint == thread }?["replies"]?.array?.first { $0["id"]?.uint == msg }
        return m.map(MessageView.isImported) ?? false
    }

    /// What is new since `seen`: fresh enough (a day, five minutes for a
    /// huddle), not already told. `seen` gains every key of `all` (what
    /// exists with every kind on), so turning a kind on later doesn't bring
    /// back old news.
    public static func fresh(_ found: [Note], all: [String], seen: inout Set<String>, now: Int64) -> [Note] {
        var out: [Note] = []
        for n in found where seen.insert(n.key).inserted {
            let limit = n.kind == .huddle ? huddleFreshSecs : freshSecs
            if let ts = n.ts, now - ts > limit { continue }
            out.append(n)
        }
        seen.formUnion(all)
        return out
    }

    /// The once-a-day summary of my tasks due on `day` and overdue by then,
    /// or nil when there are none ("3 tasks are due today", "2 overdue tasks").
    public static func dueSummary(_ view: WorkspaceView, me: UInt64, day: String) -> String? {
        let mine = view.myTasks(me: me)
        let today = mine.filter { $0.due == day }.count
        let late = mine.filter { ($0.due ?? "9999") < day }.count
        switch (today, late) {
        case (0, 0): return nil
        case (1, 0): return "1 task is due today"
        case (let d, 0): return "\(d) tasks are due today"
        case (0, 1): return "1 overdue task"
        case (0, let l): return "\(l) overdue tasks"
        case (let d, let l): return "\(d) due today, \(l) overdue"
        }
    }

    /// The app icon's number: Primary Inbox rows plus unread direct messages.
    public static func badge(_ data: JSON, view: WorkspaceView, me: UInt64, inbox items: [InboxItem]? = nil) -> Int {
        let inbox = (items ?? Inbox.items(data, me: me)).filter { $0.tab == .primary }.count
        let dms = view.visibleChannels(me: me).filter { $0.kind == .direct }.reduce(0) { $0 + $1.unread(me: me) }
        return inbox + dms
    }
}
