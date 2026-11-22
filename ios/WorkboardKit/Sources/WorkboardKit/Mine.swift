// My own reading state and notification choices, kept in step with my other
// devices (my Mac) through the team server: wb_sync::mine in
// crates/wb-sync/src/mine.rs, which explains the rules. Same shapes, same
// results:
//
//   read          {"<channel>": {ts, at}}             newest change (a tie: the later ts)
//   replies_seen  seconds                              the later one wins
//   inbox         {"<row key>": {tab, at}}             newest change per row
//   replies_read  {"<row key>": {on, at}}              newest change per row
//   set           {"notify" | "muted_until": {value, at}}   newest change
//
// `extra.mine_sync` (never shared) is what this iPhone and the server last
// agreed on, so a change made here is told from one made elsewhere. Before
// anything is agreed, what this iPhone has is stamped 0 (it never undoes a
// change made elsewhere since). Parts a newer app adds pass along untouched.

import Foundation

public struct MineView: Equatable, Sendable {
    public var read: [UInt64: Int64] = [:]
    public var repliesSeen: Int64 = 0
    public var inboxCleared: Set<String> = []
    public var inboxLater: Set<String> = []
    public var repliesRead: Set<String> = []
    /// "notify" (the Mac's NotifySettings shape) and "muted_until"; one
    /// this device doesn't have is left out.
    public var settings: [String: JSON] = [:]

    public init() {}
}

public enum Mine {
    /// The settings kept in step (wb_sync::mine::MINE_SETTINGS).
    public static let settingKeys = ["notify", "muted_until"]
    /// Fields of notify each device keeps for itself.
    public static let notifyLocal = ["fired"]

    /// What a workspace copy has.
    public static func view(of data: JSON) -> MineView {
        var v = MineView()
        for c in data["channels"]?.array ?? [] {
            if let id = c["id"]?.uint, let r = c["last_read"]?.int, r > 0 { v.read[id] = r }
        }
        func list(_ k: String) -> Set<String> { Set((data.at("extra.\(k)")?.array ?? []).compactMap(\.string)) }
        v.inboxCleared = list("inbox_cleared")
        v.inboxLater = list("inbox_later")
        v.repliesRead = list("replies_read")
        v.repliesSeen = data.at("extra.replies_seen")?.int ?? 0
        for k in settingKeys {
            if let s = data.at("extra.\(k)"), !s.isNull { v.settings[k] = settingValue(k, s) }
        }
        return v
    }

    /// notify without what stays on each device.
    public static func settingValue(_ key: String, _ v: JSON) -> JSON {
        guard key == "notify", case .object(var o) = v else { return v }
        for k in notifyLocal { o[k] = nil }
        return .object(o)
    }

    static func at(_ e: JSON?) -> Int64 { e?["at"]?.int ?? 0 }
    static func ts(_ e: JSON?) -> Int64 { e?["ts"]?.int ?? 0 }
    /// The parts this version knows; others (from a newer app) pass along.
    static let parts: Set<String> = ["read", "replies_seen", "inbox", "replies_read", "set"]

    static func tab(_ v: MineView, _ key: String) -> String {
        v.inboxCleared.contains(key) ? "cleared" : v.inboxLater.contains(key) ? "later" : ""
    }

    /// The state to send: `known` with what differs in `view` stamped `now`
    /// (milliseconds). Nothing this device lacks counts as a change.
    public static func collect(_ view: MineView, known: JSON, now nowIn: Int64) -> JSON {
        let now = (known.object ?? [:]).isEmpty ? 0 : nowIn
        var read = known["read"]?.object ?? [:]
        for (id, t) in view.read where read[String(id)].map(ts) != t { read[String(id)] = ["ts": .int(t), "at": .int(now)] }
        let seen = max(known["replies_seen"]?.int ?? 0, view.repliesSeen)

        let knownInbox = known["inbox"]?.object ?? [:]
        var inbox = knownInbox
        for k in Set(knownInbox.keys).union(view.inboxCleared).union(view.inboxLater) {
            let here = tab(view, k)
            if here != (knownInbox[k]?["tab"]?.string ?? "") { inbox[k] = ["tab": .string(here), "at": .int(now)] }
        }
        let knownRR = known["replies_read"]?.object ?? [:]
        var rr = knownRR
        for k in Set(knownRR.keys).union(view.repliesRead) {
            let here = view.repliesRead.contains(k)
            if here != (knownRR[k]?["on"]?.bool ?? false) { rr[k] = ["on": .bool(here), "at": .int(now)] }
        }
        var set = known["set"]?.object ?? [:]
        for (k, v) in view.settings where set[k]?["value"] != v { set[k] = ["value": v, "at": .int(now)] }
        var out = known.object ?? [:]
        out["read"] = .object(read)
        out["replies_seen"] = .int(seen)
        out["inbox"] = .object(inbox)
        out["replies_read"] = .object(rr)
        out["set"] = .object(set)
        return .object(out)
    }

    /// Put `other` into `base`; on a tie `base` keeps its own. True when it changed.
    @discardableResult
    public static func merge(_ base: inout JSON, _ other: JSON) -> Bool {
        let before = base
        var b = base.object ?? [:]
        var read = b["read"]?.object ?? [:]
        for (k, v) in other["read"]?.object ?? [:] where v.object != nil {
            if read[k] == nil || (at(v), ts(v)) > (at(read[k]), ts(read[k])) { read[k] = v }
        }
        b["read"] = .object(read)
        for (k, v) in other.object ?? [:] where !parts.contains(k) && b[k] == nil { b[k] = v }
        b["replies_seen"] = .int(max(b["replies_seen"]?.int ?? 0, other["replies_seen"]?.int ?? 0))
        for part in ["inbox", "replies_read", "set"] {
            var mine = b[part]?.object ?? [:]
            for (k, v) in other[part]?.object ?? [:] where v.object != nil {
                if mine[k] == nil || at(v) > at(mine[k]) { mine[k] = v }
            }
            b[part] = .object(mine)
        }
        base = .object(b)
        return base != before
    }

    /// The view a merged state stands for.
    public static func resolve(_ mine: JSON) -> MineView {
        var v = MineView()
        for (k, r) in mine["read"]?.object ?? [:] {
            if let id = UInt64(k) { v.read[id] = ts(r) }
        }
        v.repliesSeen = mine["replies_seen"]?.int ?? 0
        for (k, e) in mine["inbox"]?.object ?? [:] {
            switch e["tab"]?.string {
            case "cleared": v.inboxCleared.insert(k)
            case "later": v.inboxLater.insert(k)
            default: break
            }
        }
        for (k, e) in mine["replies_read"]?.object ?? [:] where e["on"]?.bool == true { v.repliesRead.insert(k) }
        for (k, e) in mine["set"]?.object ?? [:] {
            if let val = e["value"] { v.settings[k] = val }
        }
        return v
    }

    /// `want` in the order of `order` (wb_sync::mine::keep_order).
    static func keepOrder(_ order: [String], _ want: Set<String>) -> [String] {
        var out = order.filter { want.contains($0) }
        let had = Set(out)
        out += want.subtracting(had).sorted()
        return out
    }

    /// Take a merged view into a workspace copy: later read positions,
    /// Inbox rows, replies read and the settings (notify keeps this
    /// device's own fields). True when anything changed.
    @discardableResult
    public static func apply(_ v: MineView, to data: inout JSON) -> Bool {
        let before = data
        if case .array(var chans) = data["channels"] ?? .null {
            for i in chans.indices {
                // the newest change: lower too ("Mark unread" elsewhere)
                guard let id = chans[i]["id"]?.uint, let r = v.read[id], r != (chans[i]["last_read"]?.int ?? 0) else { continue }
                chans[i]["last_read"] = .int(r)
            }
            data["channels"] = .array(chans)
        }
        if v.repliesSeen > (data.at("extra.replies_seen")?.int ?? 0) { data.set("extra.replies_seen", .int(v.repliesSeen)) }
        for (k, want) in [("inbox_cleared", v.inboxCleared), ("inbox_later", v.inboxLater), ("replies_read", v.repliesRead)] {
            let have = (data.at("extra.\(k)")?.array ?? []).compactMap(\.string)
            if Set(have) != want { data.set("extra.\(k)", .array(keepOrder(have, want).map(JSON.string))) }
        }
        if case .object(var n)? = v.settings["notify"] {
            for k in notifyLocal { n[k] = data.at("extra.notify.\(k)") }
            if data.at("extra.notify") != .object(n) { data.set("extra.notify", .object(n)) }
        }
        if let m = v.settings["muted_until"], data.at("extra.muted_until") != m { data.set("extra.muted_until", m) }
        return data != before
    }
}

extension Edits {
    /// Replies by others in thread `parent` marked read (`extra.replies_read`,
    /// keyed like the Inbox's rows: "chat:CHANNEL:REPLY").
    public static func markThreadRead(_ data: inout JSON, channel: UInt64, parent: UInt64, me: UInt64) {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel }),
              let m = (c["messages"]?.array ?? []).first(where: { $0["id"]?.uint == parent }) else { return }
        var read = (data.at("extra.replies_read")?.array ?? []).compactMap(\.string)
        var have = Set(read)
        for r in m["replies"]?.array ?? [] where r["author"]?.uint != me {
            guard let id = r["id"]?.uint else { continue }
            let k = "chat:\(channel):\(id)"
            if have.insert(k).inserted { read.append(k) }
        }
        if read.count != (data.at("extra.replies_read")?.array ?? []).count { data.set("extra.replies_read", .array(read.map(JSON.string))) }
    }
}

extension SyncClient {
    /// The server sent my state: take it in and send back what this iPhone
    /// has that is newer (on a tie the server's stays).
    /// The server sent only what changed in my reading state (mine_part):
    /// the rest is as my devices last agreed, so the whole of it is that
    /// with this in it (on a tie the server's stays).
    func takeMinePart(_ part: JSON) {
        guard let d = data else { return }
        var theirs = part
        Mine.merge(&theirs, d.at("extra.mine_sync") ?? [:])
        takeMine(theirs)
    }

    func takeMine(_ theirs: JSON) {
        mineOK = true
        guard var d = data else { return }
        let ours = Mine.collect(Mine.view(of: d), known: d.at("extra.mine_sync") ?? [:], now: nowMs())
        var merged = theirs
        Mine.merge(&merged, ours)
        Mine.apply(Mine.resolve(merged), to: &d)
        d.set("extra.mine_sync", merged)
        let changed = d != data
        if changed { data = d; refresh() }
        if merged != theirs, let ws = socket { send(.mine(data: merged), on: ws) }
        // read or cleared elsewhere: the icon's number and the choices follow
        if changed { onUpdate() }
    }

    /// The server keeps my state for my other devices (said on this connection).
    public var keepsMine: Bool { mineOK }

    /// My notification choices as kept with the workspace, which my other
    /// devices share; nil until one of them set them.
    public var syncedNotifyPrefs: NotifyPrefs? {
        guard let d = data, let n = d.at("extra.notify"), n.object != nil else { return nil }
        return NotifyPrefs(mac: n, mutedUntil: d.at("extra.muted_until")?.int)
    }

    /// Keep my notification choices with the workspace (for my other devices).
    public func setNotifyPrefs(_ p: NotifyPrefs) {
        editLocal { d in
            d.set("extra.notify", p.mac(base: d.at("extra.notify")))
            d.set("extra.muted_until", .int(p.mutedUntil))
        }
    }

    /// The thread of `parent` is open: its replies by others count as read
    /// (mark_thread_read on the Mac), here and on my other devices.
    public func markThreadRead(channel: UInt64, parent: UInt64) {
        guard let me else { return }
        editLocal { Edits.markThreadRead(&$0, channel: channel, parent: parent, me: me) }
    }

    /// After a change here: send my state when it changed since my devices
    /// last agreed (only to a server that said it keeps it).
    func sendMineIfChanged() {
        guard mineOK, let ws = socket, state.isConnected, var d = data else { return }
        let known = d.at("extra.mine_sync") ?? [:]
        let out = Mine.collect(Mine.view(of: d), known: known, now: nowMs())
        guard out != known else { return }
        send(.mine(data: out), on: ws)
        d.set("extra.mine_sync", out)
        data = d
    }

    func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }
}
