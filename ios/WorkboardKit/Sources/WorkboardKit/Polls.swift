// Polls in chat, as on the Mac (src/app/polls.rs, Message::poll and
// Message::votes in src/model/types.rs): "/poll" writes a question with 2 to
// 10 choices; the message's text is the question with its choices numbered
// (what apps from before polls show), `poll` is what newer apps draw. Each
// person's picks are their own entry in `votes` ("<member>": [choices]),
// merged per person (wb-sync "votes/<member>"), so a vote only ever writes
// the voter's own entry. Its author closes it (or opens it again).

import Foundation

/// A poll in a message (Poll in src/model/types.rs).
public struct PollView: Hashable, Sendable {
    public let question: String
    public let options: [String]
    /// More than one choice may be picked.
    public let multi: Bool
    /// Who picked what isn't shown.
    public let anonymous: Bool
    /// When its author closed it (seconds): no more votes after that.
    public let closed: Int64?

    public var isClosed: Bool { closed != nil }

    init?(_ j: JSON) {
        guard j.object != nil, let q = j["question"]?.string else { return nil }
        question = q
        options = (j["options"]?.array ?? []).map { $0.string ?? "" }
        multi = j["multi"]?.bool ?? false
        anonymous = j["anonymous"]?.bool ?? false
        closed = j["closed"]?.int
    }
}

public enum Polls {
    /// Most and fewest choices a poll has (POLL_MAX_OPTIONS, POLL_MIN_OPTIONS).
    public static let maxOptions = 10
    public static let minOptions = 2

    /// What a poll message says in its text (poll_text): the question and
    /// its choices numbered, each trimmed.
    public static func text(question: String, options: [String]) -> String {
        var s = question.trimmed
        for (i, o) in options.enumerated() { s += "\n\(i + 1). \(o.trimmed)" }
        return s
    }

    /// The choices with words in them, trimmed (PollDraft::filled).
    public static func filled(_ options: [String]) -> [String] {
        options.map(\.trimmed).filter { !$0.isEmpty }
    }

    /// Create poll can be pressed: a question and at least two choices.
    public static func ready(question: String, options: [String]) -> Bool {
        !question.trimmed.isEmpty && filled(options).count >= minOptions
    }

    /// My picks after a tap on choice `i` (Message::toggle_vote): picking a
    /// picked one takes it back; with several allowed it's added (in
    /// order), else it replaces mine. Nil when nothing changes (closed, no
    /// such choice). An empty result means my entry goes.
    public static func toggled(_ mine: [Int], option i: Int, poll: PollView) -> [Int]? {
        guard !poll.isClosed, i >= 0, i < poll.options.count else { return nil }
        var picks = mine
        if let at = picks.firstIndex(of: i) {
            picks.remove(at: at)
        } else if poll.multi {
            picks.append(i)
            picks.sort()
        } else {
            picks = [i]
        }
        return picks
    }

    /// Votes for each choice, in order (poll_counts); picks past the
    /// choices don't count.
    public static func counts(_ votes: [UInt64: [Int]], options n: Int) -> [Int] {
        var out = Array(repeating: 0, count: max(0, n))
        for picks in votes.values {
            for i in picks where i >= 0 && i < n { out[i] += 1 }
        }
        return out
    }

    /// `votes` as read from a message: member -> picks.
    static func votes(_ j: JSON?) -> [UInt64: [Int]] {
        var out: [UInt64: [Int]] = [:]
        for (k, v) in j?.object ?? [:] {
            guard let who = UInt64(k) else { continue }
            let picks = (v.array ?? []).compactMap { $0.int }.map { Int($0) }
            if !picks.isEmpty { out[who] = picks }
        }
        return out
    }

    /// The poll object as the Mac writes it: `multi`, `anonymous` and
    /// `closed` only when set.
    static func object(question: String, options: [String], multi: Bool, anonymous: Bool, closed: Int64? = nil) -> JSON {
        var o: [String: JSON] = ["question": .string(question), "options": .array(options.map(JSON.string))]
        if multi { o["multi"] = .bool(true) }
        if anonymous { o["anonymous"] = .bool(true) }
        if let closed { o["closed"] = .int(closed) }
        return .object(o)
    }
}

extension MessageView {
    /// Votes for each choice of its poll (none without one).
    public var pollCounts: [Int] { Polls.counts(votes, options: poll?.options.count ?? 0) }

    /// How many people voted.
    public var voters: Int { votes.count }

    /// The choices `who` picked.
    public func picks(of who: UInt64?) -> [Int] { who.flatMap { votes[$0] } ?? [] }

    /// The people who picked choice `i`, in member order.
    public func voters(for i: Int) -> [UInt64] { votes.filter { $0.value.contains(i) }.keys.sorted() }
}

extension Edits {
    /// Changes the message or thread reply `message` in `channel`; `change`
    /// may refuse (false), leaving everything as it was.
    @discardableResult
    static func updateMessage(_ data: inout JSON, channel: UInt64, message: UInt64, _ change: (inout JSON) -> Bool) -> Bool {
        var done = false
        data.tryUpdateItem(at: "channels", id: channel) { c in
            if c.tryUpdateItem(at: "messages", id: message, change) {
                done = true
                return true
            }
            guard var msgs = c["messages"]?.array else { return false }
            for i in msgs.indices where msgs[i].tryUpdateItem(at: "replies", id: message, change) {
                c["messages"] = .array(msgs)
                done = true
                return true
            }
            return false
        }
        return done
    }

    /// Posts a poll in `channel`, or as a reply in thread `parent`
    /// (post_poll): the text is the question and choices written out, with
    /// the people it @-mentions. Needs a question and 2 to 10 choices with
    /// words in them (the empty ones are left out).
    @discardableResult
    public static func sendPoll(_ data: inout JSON, channel: UInt64, parent: UInt64? = nil, question: String, options: [String], multi: Bool, anonymous: Bool,
                                online: Set<UInt64> = [], me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let question = question.trimmed
        let options = Polls.filled(options)
        guard !question.isEmpty, (Polls.minOptions...Polls.maxOptions).contains(options.count) else { return false }
        let text = Polls.text(question: question, options: options)
        let mentions = Mentions.chatIDs(in: text, data: data, channel: channel, online: online, author: me, picked: [])
        var m = message(id: id, author: me, text: text, ts: ts)
        m["poll"] = Polls.object(question: question, options: options, multi: multi, anonymous: anonymous)
        Mentions.put(mentions, on: &m)
        if let parent {
            var done = false
            data.updateItem(at: "channels", id: channel) { c in
                done = c.updateItem(at: "messages", id: parent) { p in
                    var reps = p["replies"]?.array ?? []
                    reps.append(m)
                    p["replies"] = .array(reps)
                }
            }
            return done
        }
        return data.updateItem(at: "channels", id: channel) { c in
            var msgs = c["messages"]?.array ?? []
            msgs.append(m)
            c["messages"] = .array(msgs)
            c["last_read"] = .int(ts)
        }
    }

    /// Picks or unpicks choice `option` of the poll in `message` for me
    /// (toggle_vote). Only my own entry in `votes` is written; taking my
    /// last pick back takes my entry out (and `votes` when it's empty).
    /// False when nothing changes (no poll, closed, no such choice).
    @discardableResult
    public static func vote(_ data: inout JSON, channel: UInt64, message: UInt64, option: Int, me: UInt64) -> Bool {
        updateMessage(&data, channel: channel, message: message) { m in
            guard let p = m["poll"].flatMap(PollView.init) else { return false }
            let key = String(me)
            let mine = (m["votes"]?[key]?.array ?? []).compactMap { $0.int }.map { Int($0) }
            guard let picks = Polls.toggled(mine, option: option, poll: p) else { return false }
            var votes = m["votes"]?.object ?? [:]
            votes[key] = picks.isEmpty ? nil : .array(picks.map { .int(Int64($0)) })
            m["votes"] = votes.isEmpty ? nil : .object(votes)
            return true
        }
    }

    /// Its author closes the poll in `message` (`close`), or opens it
    /// again: the poll object is written again with or without `closed`.
    @discardableResult
    public static func closePoll(_ data: inout JSON, channel: UInt64, message: UInt64, close: Bool, me: UInt64, ts: Int64) -> Bool {
        updateMessage(&data, channel: channel, message: message) { m in
            guard m["author"]?.uint == me, var p = m["poll"], p.object != nil else { return false }
            guard (p["closed"] != nil && p["closed"] != .null) != close else { return false }
            p["closed"] = close ? .int(ts) : nil
            m["poll"] = p
            return true
        }
    }
}
