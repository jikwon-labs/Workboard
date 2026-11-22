import Foundation
@testable import WorkboardKit

// A large made-up workspace for checking that the quick paths (fingerprints
// fed byte by byte, fingerprints kept between changes, merges that keep
// untouched items) give exactly what the plain way gives, and for timing them.

struct Rand {
    var s: UInt64
    mutating func next() -> UInt64 {
        s = s &* 6364136223846793005 &+ 1442695040888963407
        return s >> 33
    }
    mutating func below(_ n: Int) -> Int { Int(next() % UInt64(max(n, 1))) }
}

/// Texts that exercise the writer's escapes and non-ASCII text.
let oddTexts = ["plain", "quote \" and \\ slash", "line\nbreak\ttab\r", "ctl \u{01}\u{1F}\u{08}\u{0C}", "한글 메시지", "emoji 🎉👍🏽", "/ slash", ""]

/// `tasks` tasks in 3 spaces of 4 lists, `channels` channels of `messages`
/// messages each (every 5th with replies, every 7th with reactions), and
/// a few docs. Stamped once, so it is a copy as a phone keeps it.
func bigWorkspace(tasks: Int, channels: Int, messages: Int, seed: UInt64 = 1) -> JSON {
    var r = Rand(s: seed)
    var nextId: Int64 = 1_000
    func id() -> Int64 { nextId += 1; return nextId }
    var spaces: [JSON] = []
    var lists: [Int64] = []
    for s in 0..<3 {
        var ls: [JSON] = []
        for l in 0..<4 {
            let lid = id()
            lists.append(lid)
            ls.append(["id": .int(lid), "name": .string("List \(s).\(l)"), "statuses": ["to do", "done"]])
        }
        spaces.append(["id": .int(id()), "name": .string("Space \(s)"), "lists": .array(ls), "folders": []])
    }
    var ts: [JSON] = []
    for t in 0..<tasks {
        var o: [String: JSON] = [
            "id": .int(id()), "name": .string("Task \(t) " + oddTexts[t % oddTexts.count]), "list": .int(lists[r.below(lists.count)]),
            "status": t % 3 == 0 ? "done" : "to do", "priority": .int(Int64(t % 4)), "description": .string(String(repeating: "words ", count: r.below(20))),
            "activity": .array((0..<(t % 3)).map { a in ["ts": .int(Int64(1_700_000_000 + a)), "author": 1, "text": .string("comment \(a)"), "comment": true] }),
            "tags": t % 4 == 0 ? ["a", "b"] : [], "due": t % 5 == 0 ? .int(1_800_000_000) : .null, "score": .double(Double(t) / 3),
        ]
        if t % 6 == 0 { o["time_entries"] = [["member": 1, "start": .int(Int64(1_700_000_000 + t)), "secs": 60]] }
        if t % 8 == 0 { o["custom"] = ["f1": .string("v\(t)"), "f2": .int(Int64(t))] }
        if t % 9 == 0 { o["timer_started"] = .int(1_700_000_100); o["timer_by"] = 2 }
        ts.append(.object(o))
    }
    var chs: [JSON] = []
    var clock: Int64 = 1_700_000_000
    for c in 0..<channels {
        var ms: [JSON] = []
        for m in 0..<messages {
            clock += 1
            var o: [String: JSON] = ["id": .int(id()), "author": .int(Int64(1 + m % 3)), "text": .string("message \(m) " + oddTexts[m % oddTexts.count]), "ts": .int(clock)]
            if m % 5 == 0 {
                o["replies"] = .array((0..<2).map { k in ["id": .int(id()), "author": 2, "text": .string("reply \(k)"), "ts": .int(clock + Int64(k) + 1)] })
            }
            if m % 7 == 0 { o["reactions"] = [["emoji": "👍", "users": [1, 2]]] }
            ms.append(.object(o))
        }
        chs.append(["id": .int(id()), "name": .string("channel-\(c)"), "kind": "Channel", "topic": "", "members": [1, 2, 3], "messages": .array(ms), "last_read": .int(clock)])
    }
    let docs: [JSON] = (0..<5).map { d in ["id": .int(id()), "title": .string("Doc \(d)"), "body": .string(oddTexts[d % oddTexts.count]), "parent": .null] }
    var w: JSON = [
        "workspace": "Big", "spaces": .array(spaces), "tasks": .array(ts), "channels": .array(chs), "docs": .array(docs),
        "members": [["id": 1, "name": "Ann"], ["id": 2, "name": "Bo"], ["id": 3, "name": "Cy"]], "me": 1, "next_id": .int(nextId + 1),
        "favorites": [], "extra": ["sync": SyncMeta().json, "slack_map": ["U1": 1, "U2": 2], "workspace_color": "#336699"],
    ]
    var meta = SyncMeta()
    Sync.stamp(w, &meta, now: 1_700_000_000_000)
    w.set("extra.sync", meta.json)
    return w
}

/// Seconds `f` took.
func timed(_ f: () -> Void) -> Double {
    let t = Date()
    f()
    return Date().timeIntervalSince(t)
}
