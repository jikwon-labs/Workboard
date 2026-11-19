// Sending, the Mac app's way (src/app/chat.rs, src/app/schedule.rs):
// forwarding a message to another conversation, a thread reply also sent
// to the channel (`thread_of` on the channel's copy, which doesn't notify),
// and messages the team server keeps to post later.

import Foundation

/// A message forwarded from another conversation (Message::forwarded).
public struct ForwardedView: Hashable, Sendable {
    public let channel: UInt64
    public let message: UInt64
    public let author: UInt64
    public let ts: Int64
    public let text: String

    init?(_ j: JSON) {
        guard let c = j["channel"]?.uint, let m = j["message"]?.uint else { return nil }
        channel = c
        message = m
        author = j["author"]?.uint ?? 0
        ts = j["ts"]?.int ?? 0
        text = j["text"]?.string ?? ""
    }
}

/// A message the server keeps to post at `at` (wb_net::ScheduledMsg).
public struct ScheduledItem: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let channel: UInt64
    public let parent: UInt64?
    public let text: String
    public let at: Int64
    public let mentions: [UInt64]

    public init(id: UInt64, channel: UInt64, parent: UInt64? = nil, text: String, at: Int64, mentions: [UInt64] = []) {
        self.id = id; self.channel = channel; self.parent = parent; self.text = text; self.at = at; self.mentions = mentions
    }

    init?(_ j: JSON) {
        guard let id = j["id"]?.uint, let c = j["channel"]?.uint else { return nil }
        self.init(id: id, channel: c, parent: j["parent"]?.uint, text: j["text"]?.string ?? "", at: j["at"]?.int ?? 0,
                  mentions: (j["mentions"]?.array ?? []).compactMap(\.uint))
    }

    public var json: JSON {
        var o: [String: JSON] = ["id": .id(id), "channel": .id(channel), "text": .string(text), "at": .int(at)]
        if let parent { o["parent"] = .id(parent) }
        if !mentions.isEmpty { o["mentions"] = .array(mentions.map(JSON.id)) }
        return .object(o)
    }
}

/// The times the clock offers (src/app/schedule.rs schedule_choices).
public enum ScheduleTimes {
    public static func choices(now: Date = Date(), calendar cal: Calendar = .current) -> [(String, Int64)] {
        let tomorrow = cal.date(bySettingHour: 9, minute: 0, second: 0, of: cal.date(byAdding: .day, value: 1, to: now)!)!
        let weekday = cal.component(.weekday, from: now) // 1 Sunday ... 2 Monday
        var ahead = (9 - weekday) % 7
        if ahead == 0 { ahead = 7 }
        let monday = cal.date(bySettingHour: 9, minute: 0, second: 0, of: cal.date(byAdding: .day, value: ahead, to: now)!)!
        let t = Int64(now.timeIntervalSince1970)
        return [("In 1 hour", t + 3600), ("Tomorrow at 9:00 AM", Int64(tomorrow.timeIntervalSince1970)),
                ("Monday at 9:00 AM", Int64(monday.timeIntervalSince1970))]
    }
}

extension Edits {
    /// Post `message` of channel `from` (a forwarded one carries its first
    /// message) in channel `to`, with my `note`.
    @discardableResult
    public static func forward(_ data: inout JSON, from: UInt64, message: UInt64, to: UInt64, note: String, online: Set<UInt64> = [], me: UInt64, id: UInt64, ts: Int64) -> Bool {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == from }) else { return false }
        let all = (c["messages"]?.array ?? []).flatMap { [$0] + ($0["replies"]?.array ?? []) }
        guard let orig = all.first(where: { $0["id"]?.uint == message }) else { return false }
        let carried: JSON = orig["forwarded"].flatMap { $0.object != nil ? $0 : nil } ?? .object([
            "channel": .id(from), "message": .id(message), "author": .id(orig["author"]?.uint ?? 0),
            "ts": .int(orig["ts"]?.int ?? 0), "text": .string(orig["text"]?.string ?? ""),
        ])
        let text = note.trimmed
        let mentions = Mentions.chatIDs(in: text, data: data, channel: to, online: online, author: me)
        return data.updateItem(at: "channels", id: to) { ch in
            var m = Self.message(id: id, author: me, text: text, ts: ts)
            m["forwarded"] = carried
            Mentions.put(mentions, on: &m)
            var msgs = ch["messages"]?.array ?? []
            msgs.append(m)
            ch["messages"] = .array(msgs)
            ch["last_read"] = .int(ts)
        }
    }

    /// "Also send to the channel": the channel's copy of reply `reply` in
    /// thread `thread`, marked with it.
    @discardableResult
    public static func copyReplyToChannel(_ data: inout JSON, channel: UInt64, thread: UInt64, reply: UInt64, me: UInt64, id: UInt64) -> Bool {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel }),
              let parent = (c["messages"]?.array ?? []).first(where: { $0["id"]?.uint == thread }),
              var r = (parent["replies"]?.array ?? []).first(where: { $0["id"]?.uint == reply }),
              r["author"]?.uint == me else { return false }
        r["id"] = .id(id)
        r["thread_of"] = .id(thread)
        r["replies"] = []
        r["reactions"] = []
        r["preview"] = nil
        return data.updateItem(at: "channels", id: channel) { ch in
            var msgs = ch["messages"]?.array ?? []
            msgs.append(r)
            ch["messages"] = .array(msgs)
        }
    }

    /// Post a scheduled message now, as written (Send now).
    @discardableResult
    public static func postScheduled(_ data: inout JSON, _ item: ScheduledItem, me: UInt64, ts: Int64) -> Bool {
        if let p = item.parent {
            var done = false
            data.updateItem(at: "channels", id: item.channel) { c in
                done = c.updateItem(at: "messages", id: p) { m in
                    var reps = m["replies"]?.array ?? []
                    var r = message(id: item.id, author: me, text: item.text, ts: ts)
                    Mentions.put(item.mentions, on: &r)
                    reps.append(r)
                    m["replies"] = .array(reps)
                }
            }
            return done
        }
        return data.updateItem(at: "channels", id: item.channel) { c in
            var m = message(id: item.id, author: me, text: item.text, ts: ts)
            Mentions.put(item.mentions, on: &m)
            var msgs = c["messages"]?.array ?? []
            msgs.append(m)
            c["messages"] = .array(msgs)
        }
    }
}

extension SyncClient {
    /// Ask the server to post `text` in `channel` at `at`. False when not
    /// connected (the server keeps scheduled messages).
    @discardableResult
    public func schedule(channel: UInt64, parent: UInt64? = nil, text: String, picked: [UInt64] = [], at: Int64) -> Bool {
        let t = text.trimmed
        guard !t.isEmpty, state.isConnected, let ws = socket, let d = data, let me else { return false }
        let item = ScheduledItem(id: newId(), channel: channel, parent: parent, text: t, at: at,
                                 mentions: Mentions.chatIDs(in: t, data: d, channel: channel, online: online, author: me, picked: picked))
        send(.schedule(item: item.json), on: ws)
        // shown now; the server's answer replaces the list
        scheduled = ((scheduled ?? []).filter { $0.id != item.id } + [item]).sorted { ($0.at, $0.id) < ($1.at, $1.id) }
        return true
    }

    public func unschedule(_ id: UInt64) {
        if let ws = socket { send(.unschedule(id: id), on: ws) }
        scheduled?.removeAll { $0.id == id }
    }

    public func askScheduled() {
        if state.isConnected, let ws = socket { send(.scheduledList, on: ws) }
    }

    /// My scheduled messages not posted yet (the server's list catches up
    /// on its next answer).
    public var waitingToSend: [ScheduledItem] {
        guard let list = scheduled, let d = data else { return scheduled ?? [] }
        let posted = Set((d["channels"]?.array ?? []).flatMap { c in
            (c["messages"]?.array ?? []).flatMap { m in [m["id"]?.uint] + (m["replies"]?.array ?? []).map { $0["id"]?.uint } }
        }.compactMap { $0 })
        return list.filter { !posted.contains($0.id) }
    }
}
