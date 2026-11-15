import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The four tests in src/sync.rs, ported one to one.
final class RustPortTests: XCTestCase {
    func ws(_ tasks: JSON, _ channels: JSON, _ meta: SyncMeta) -> JSON {
        ["workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": channels, "docs": [], "extra": ["sync": meta.json]]
    }

    func texts(_ v: JSON, _ path: (JSON) -> JSON?) -> [String] {
        (path(v)?.array ?? []).compactMap { $0["text"]?.string }
    }

    func testTwoPeopleChattingKeepBothMessages() {
        var ma = SyncMeta(), mb = SyncMeta()
        let ch = { (msgs: JSON, read: Int64) -> JSON in [["id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [], "messages": msgs, "last_read": .int(read)]] }
        let base = ch([["id": 11, "author": 1, "text": "hi", "ts": 100]], 0)
        var a = ws([], base, ma)
        Sync.stamp(a, &ma, now: 100)
        var b = ws([], base, mb)
        Sync.stamp(b, &mb, now: 100)
        a = ws([], ch([["id": 11, "author": 1, "text": "hi", "ts": 100], ["id": 21, "author": 1, "text": "from A", "ts": 200]], 500), ma)
        Sync.stamp(a, &ma, now: 200)
        a.set("extra.sync", ma.json)
        b = ws([], ch([["id": 11, "author": 1, "text": "hi", "ts": 100], ["id": 31, "author": 2, "text": "from B", "ts": 150]], 9), mb)
        Sync.stamp(b, &mb, now: 150)
        b.set("extra.sync", mb.json)
        XCTAssertTrue(Sync.merge(&a, [b]))
        XCTAssertEqual(texts(a) { $0["channels"]?[0]?["messages"] }, ["hi", "from B", "from A"])
        XCTAssertEqual(a["channels"]?[0]?["last_read"], 500, "read position stays personal")
    }

    func testNewerEditWinsAndDeletesStick() {
        var ma = SyncMeta(), mb = SyncMeta()
        let t = { (name: String, id: Int64) -> JSON in ["id": .int(id), "name": .string(name), "activity": []] }
        var a = ws([t("one", 1), t("two", 2)], [], ma)
        Sync.stamp(a, &ma, now: 10)
        var b = a
        mb.hashes = ma.hashes
        mb.stamps = ma.stamps
        // A renames task 1 later than B does; B deletes task 2
        a["tasks"] = [t("one by A", 1), t("two", 2)]
        Sync.stamp(a, &ma, now: 30)
        a.set("extra.sync", ma.json)
        b["tasks"] = [t("one by B", 1)]
        Sync.stamp(b, &mb, now: 20)
        b.set("extra.sync", mb.json)
        Sync.merge(&a, [b])
        XCTAssertEqual((a["tasks"]?.array ?? []).compactMap { $0["name"]?.string }, ["one by A"])
        // and B sees the same after reading A
        Sync.merge(&b, [a])
        XCTAssertEqual((b["tasks"]?.array ?? []).compactMap { $0["name"]?.string }, ["one by A"])
    }

    func testCommentsFromBothSidesAreKept() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "activity": []]], [], ma)
        Sync.stamp(a, &ma, now: 10)
        var mb = ma
        var b = a
        a.set("tasks", [["id": 1, "name": "t", "activity": [["ts": 20, "author": 1, "text": "A says", "comment": true]]]])
        Sync.stamp(a, &ma, now: 20)
        a.set("extra.sync", ma.json)
        b.set("tasks", [["id": 1, "name": "t", "activity": [["ts": 25, "author": 2, "text": "B says", "comment": true]]]])
        Sync.stamp(b, &mb, now: 25)
        b.set("extra.sync", mb.json)
        Sync.merge(&a, [b])
        XCTAssertEqual(a["tasks"]?[0]?["activity"]?.array?.count, 2)
    }

    func testRestoredItemComesBackForEveryone() {
        var m = SyncMeta()
        let a = ws([["id": 1, "name": "t"]], [], m)
        Sync.stamp(a, &m, now: 10)
        let gone = ws([], [], m)
        Sync.stamp(gone, &m, now: 20)
        XCTAssertNotNil(m.tombs[1])
        Sync.stamp(a, &m, now: 30)
        XCTAssertGreaterThan(m.revived[1]!, m.tombs[1]!)
        var other = ws([], [], SyncMeta(tombs: [1: 20]))
        var mine = a
        mine.set("extra.sync", m.json)
        Sync.merge(&other, [mine])
        XCTAssertEqual(other["tasks"]?.array?.count, 1)
    }
}

final class StampTests: XCTestCase {
    func testFingerprintsAreStableAndIgnoreLocalKeysAndChildren() {
        let a: JSON = ["channels": [["id": 1, "name": "x", "last_read": 5, "messages": [["id": 2, "text": "m", "ts": 1]]]]]
        var b = a
        b.set("channels", [["id": 1, "name": "x", "last_read": 99, "messages": [["id": 2, "text": "m", "ts": 1], ["id": 3, "text": "n", "ts": 2]]]])
        let fa = Sync.fingerprints(a), fb = Sync.fingerprints(b)
        XCTAssertEqual(fa, Sync.fingerprints(a))
        XCTAssertEqual(fa[1], fb[1], "read position and new messages are not the channel's own fields")
        XCTAssertNotNil(fb[3])
    }

    func testFingerprintDoesNotDependOnKeyOrderOfParsing() throws {
        let x = try JSON(parsing: #"{"tasks":[{"id":1,"name":"a","status":"to do"}]}"#)
        let y = try JSON(parsing: #"{"tasks":[{"status":"to do","name":"a","id":1}]}"#)
        XCTAssertEqual(Sync.fingerprints(x), Sync.fingerprints(y))
    }

    func testPastSeenBeatsAheadClock() {
        var m = SyncMeta()
        var d: JSON = ["tasks": [["id": 1, "name": "a"]]]
        Sync.stamp(d, &m, now: 10)
        m.stamps[1] = 500 // another device with a clock ahead stamped it
        d.set("tasks", [["id": 1, "name": "b"]])
        Sync.stamp(d, &m, now: 100)
        XCTAssertEqual(m.stamps[1], 501)
    }

    func testMetaRoundTripKeepsBigHashes() throws {
        let m = SyncMeta(stamps: [1: 10], tombs: [2: 20], revived: [3: 30], hashes: [4: UInt64.max - 7])
        let back = SyncMeta(json: try JSON(parsing: m.json.serialized()))
        XCTAssertEqual(back, m)
    }
}

final class JSONTests: XCTestCase {
    func testRoundTripKeepsNumberKinds() throws {
        let text = #"{"a":1,"b":-2,"c":1.5,"d":18446744073709551615,"e":"x\n\"y\" é 😀","f":[true,false,null],"g":{}}"#
        let j = try JSON(parsing: text)
        XCTAssertEqual(j["a"], .int(1))
        XCTAssertEqual(j["c"], .double(1.5))
        XCTAssertEqual(j["d"], .uint(UInt64.max))
        XCTAssertEqual(j["e"]?.string, "x\n\"y\" é 😀")
        XCTAssertEqual(try JSON(parsing: j.serialized()), j)
    }

    func testRejectsBrokenText() {
        XCTAssertThrowsError(try JSON(parsing: "{\"a\":"))
        XCTAssertThrowsError(try JSON(parsing: "[1,]x"))
    }

    func testSetPathCreatesObjects() {
        var j: JSON = ["x": 1]
        j.set("extra.sync.hashes", [:])
        XCTAssertEqual(j.at("extra.sync.hashes"), .object([:]))
        XCTAssertEqual(j["x"], 1)
    }
}

final class IdTests: XCTestCase {
    func testIdsEndInTagAndKeepRising() {
        var m = IdMaker(tag: 7)
        let a = m.next(nowMillis: 1000)
        XCTAssertEqual(a, 1000 * 1024 + 7)
        let b = m.next(nowMillis: 1000)
        XCTAssertGreaterThan(b, a)
        XCTAssertEqual(b % 1024, 7)
        let c = m.next(nowMillis: 900) // clock went back
        XCTAssertGreaterThan(c, b)
        XCTAssertEqual(c % 1024, 7)
    }

    func testTagIsModulo1024() {
        var m = IdMaker(tag: 1030)
        XCTAssertEqual(m.next(nowMillis: 1) % 1024, 6)
    }
}

/// The newer tests in crates/wb-sync/src/lib.rs, ported one to one, so the
/// phone merges exactly like the Mac app and the server.
final class RustPortTests2: XCTestCase {
    func sp(_ spaces: JSON, _ meta: SyncMeta) -> JSON {
        ["workspace": "W", "spaces": spaces, "tasks": [], "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": [], "docs": [], "extra": ["sync": meta.json]]
    }

    func list(_ id: Int64) -> JSON { ["id": .int(id), "name": .string("L\(id)")] }
    func space(_ id: Int64, _ folders: JSON, _ lists: JSON) -> JSON { ["id": .int(id), "name": .string("S\(id)"), "folders": folders, "lists": lists] }
    func folder(_ id: Int64, _ lists: JSON) -> JSON { ["id": .int(id), "name": .string("F\(id)"), "lists": lists] }

    /// Two devices that start from the same synced copy.
    func pair(_ spaces: JSON) -> (JSON, SyncMeta, JSON, SyncMeta) {
        var ma = SyncMeta()
        var a = sp(spaces, ma)
        Sync.stamp(a, &ma, now: 10)
        a.set("extra.sync", ma.json)
        return (a, ma, a, ma)
    }

    func change(_ v: inout JSON, _ m: inout SyncMeta, _ spaces: JSON, _ now: Int64) {
        v["spaces"] = spaces
        Sync.stamp(v, &m, now: now)
        v.set("extra.sync", m.json)
    }

    /// Where every list is: (parent id, list id), in order.
    func places(_ v: JSON) -> [[UInt64]] {
        var out: [[UInt64]] = []
        for s in v["spaces"]?.array ?? [] {
            for l in s["lists"]?.array ?? [] { out.append([s["id"]!.uint!, l["id"]!.uint!]) }
            for f in s["folders"]?.array ?? [] {
                for l in f["lists"]?.array ?? [] { out.append([f["id"]!.uint!, l["id"]!.uint!]) }
            }
        }
        return out
    }

    func testAMovedListAppearsOnceForEveryone() {
        // space to space
        var (a, ma, b, _) = pair([space(100, [], [list(101), list(102)]), space(200, [], [])])
        change(&a, &ma, [space(100, [], [list(102)]), space(200, [], [list(101)])], 30)
        Sync.merge(&a, [b])
        XCTAssertEqual(places(a), [[100, 102], [200, 101]])
        Sync.merge(&b, [a])
        XCTAssertEqual(places(b), [[100, 102], [200, 101]])

        // folder to another space, and back from a space into a folder
        var (c, mc, d, _) = pair([space(100, [folder(300, [list(101)])], []), space(200, [], [])])
        change(&c, &mc, [space(100, [folder(300, [])], []), space(200, [], [list(101)])], 30)
        var stale = d
        Sync.merge(&d, [c])
        XCTAssertEqual(places(d), [[200, 101]])
        Sync.merge(&c, [stale])
        XCTAssertEqual(places(c), [[200, 101]])
        var md = SyncMeta(of: d)
        change(&d, &md, [space(100, [folder(300, [list(101)])], []), space(200, [], [])], 40)
        stale = c
        Sync.merge(&c, [d])
        XCTAssertEqual(places(c), [[300, 101]])
        Sync.merge(&d, [stale])
        XCTAssertEqual(places(d), [[300, 101]])
    }

    func testAListAddedWhileAnotherMovedIsKept() {
        var (a, ma, b, mb) = pair([space(100, [], [list(101), list(102)]), space(200, [], [])])
        // A moves 101 away; B, not knowing, adds 103 to the old space
        change(&a, &ma, [space(100, [], [list(102)]), space(200, [], [list(101)])], 30)
        change(&b, &mb, [space(100, [], [list(101), list(102), list(103)]), space(200, [], [])], 25)
        var (a0, b0) = (a, b)
        Sync.merge(&a, [b0])
        Sync.merge(&b, [a0])
        XCTAssertEqual(places(a), [[100, 102], [100, 103], [200, 101]])
        XCTAssertEqual(places(b), places(a))

        // a folder moves while someone adds a list inside it
        (a, ma, b, mb) = pair([space(100, [folder(300, [list(101)])], []), space(200, [], [])])
        change(&a, &ma, [space(100, [], []), space(200, [folder(300, [list(101)])], [])], 30)
        change(&b, &mb, [space(100, [folder(300, [list(101), list(104)])], []), space(200, [], [])], 25)
        (a0, b0) = (a, b)
        Sync.merge(&a, [b0])
        Sync.merge(&b, [a0])
        XCTAssertEqual(places(a), [[300, 101], [300, 104]])
        XCTAssertEqual(a["spaces"]?[1]?["folders"]?[0]?["id"], 300)
        XCTAssertEqual(a["spaces"]?[0]?["folders"]?.array?.count, 0)
        XCTAssertEqual(places(b), places(a))
    }

    func testUpdatingTheAppDoesNotMarkInnerItemsAsEdited() {
        let d = sp([space(100, [], [list(101)])], SyncMeta())
        var m = SyncMeta(hashes: Sync.fingerprints(d, placed: false))
        m.stamps = [100: 10, 101: 10]
        Sync.stamp(d, &m, now: 99)
        XCTAssertEqual(m.stamps[101], 10)
        XCTAssertEqual(m.v, 1)
    }

    func ws(_ tasks: JSON, _ meta: SyncMeta) -> JSON {
        ["workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": [], "docs": [], "extra": ["sync": meta.json]]
    }

    func testADeleteWinsEvenWithAClockBehind() {
        var m = SyncMeta()
        let a = ws([["id": 1, "name": "t"]], m)
        Sync.stamp(a, &m, now: 1000)
        // restored on a device whose clock is ahead
        m.revived[1] = 2000
        let gone = ws([], m)
        Sync.stamp(gone, &m, now: 20)
        XCTAssertGreaterThan(m.tombs[1]!, 2000)
        var other = a
        var mine = gone
        mine.set("extra.sync", m.json)
        Sync.merge(&other, [mine])
        XCTAssertEqual(other["tasks"]?.array?.count, 0)
    }

    func testWorkspaceSettingsAreSharedNewestWins() {
        var (a, ma, b, mb) = pair([])
        a.set("extra.apps", ["priorities": true])
        b.set("extra.apps", ["priorities": true])
        Sync.stamp(a, &ma, now: 11)
        Sync.stamp(b, &mb, now: 11)
        // A renames the workspace, B turns off priorities later
        a["workspace"] = "Team"
        Sync.stamp(a, &ma, now: 20)
        a.set("extra.sync", ma.json)
        b.set("extra.apps", ["priorities": false])
        Sync.stamp(b, &mb, now: 30)
        b.set("extra.sync", mb.json)
        let (a0, b0) = (a, b)
        XCTAssertTrue(Sync.merge(&a, [b0]))
        Sync.merge(&b, [a0])
        for v in [a, b] {
            XCTAssertEqual(v["workspace"], "Team")
            XCTAssertEqual(v.at("extra.apps.priorities"), false)
        }
        // a setting nobody changed yet stays as each device has it
        var (c, _, d, _) = pair([])
        d.set("extra.workspace_color", [1, 2, 3])
        Sync.merge(&c, [d])
        XCTAssertNil(c.at("extra.workspace_color"))
    }

    func testActivityIdsMatchTheRustOnes() {
        // values printed by wb_sync::activity_id
        XCTAssertEqual(Sync.activityId(task: 201, ts: 1_700_000_000, author: 3, comment: true, text: "Looks good 👍", nth: 0), 8258850863592127377)
        XCTAssertEqual(Sync.activityId(task: 5, ts: -1, author: 0, comment: false, text: "created this task", nth: 2), 7191864566954938370)
        // same text twice in a task: different ids; existing ids are kept
        var d: JSON = ["tasks": [["id": 9, "activity": [
            ["ts": 1, "author": 1, "text": "x", "comment": false],
            ["ts": 1, "author": 1, "text": "x", "comment": false],
            ["ts": 2, "author": 1, "text": "y", "comment": true, "id": 77],
        ]]]]
        XCTAssertTrue(Sync.fillActivityIds(&d))
        let ids = (d["tasks"]?[0]?["activity"]?.array ?? []).compactMap { $0["id"]?.uint }
        XCTAssertEqual(ids, [Sync.activityId(task: 9, ts: 1, author: 1, comment: false, text: "x", nth: 0), Sync.activityId(task: 9, ts: 1, author: 1, comment: false, text: "x", nth: 1), 77])
        XCTAssertFalse(Sync.fillActivityIds(&d), "nothing left to fill")
    }

    func testMetaKeepsVersionAndSharedSettings() throws {
        let m = SyncMeta(stamps: [1: 10], v: 1, values: ["workspace": 5], valueHashes: ["workspace": UInt64.max - 1])
        XCTAssertEqual(SyncMeta(json: try JSON(parsing: m.json.serialized())), m)
        // the copy for the server leaves this device's fingerprints out
        var d: JSON = ["extra": ["sync": m.json]]
        d = Sync.outgoing(d)
        XCTAssertEqual(d.at("extra.sync.value_hashes"), .object([:]))
        XCTAssertEqual(d.at("extra.sync.v"), 1)
    }
}

/// `hidden` from newer servers (wb_sync::hide): out of the copy, no tombstone.
final class HideTests: XCTestCase {
    func testHiddenItemsLeaveWithoutTombstones() throws {
        var d = SampleWorkspace.make()
        var meta = SyncMeta()
        Sync.stamp(d, &meta, now: 10)
        d.set("extra.sync", meta.json)
        let gone = Sync.hide(&d, ids: [303, 999])
        XCTAssertEqual(gone, [303, 331], "the DM and the message in it")
        XCTAssertNil(WorkspaceView(d).channels.first { $0.id == 303 })
        var m = SyncMeta(of: d)
        XCTAssertNil(m.stamps[331])
        XCTAssertNil(m.hashes[303])
        Sync.stamp(d, &m, now: 20)
        XCTAssertNil(m.tombs[303], "hiding is not deleting")
        XCTAssertNil(m.tombs[331])
        // and the server's merge keeps it for the others
        var server = SampleWorkspace.make()
        var sm = SyncMeta()
        Sync.stamp(server, &sm, now: 10)
        server.set("extra.sync", sm.json)
        d.set("extra.sync", m.json)
        Sync.merge(&server, [d])
        XCTAssertNotNil(WorkspaceView(server).channels.first { $0.id == 303 })
        // the field is read from welcome and update; older servers leave it out
        XCTAssertEqual(ServerMessage(text: #"{"type":"update","data":{},"rev":2,"hidden":[303]}"#), .update(data: [:], rev: 2, hidden: [303]))
        guard case let .welcome(w) = ServerMessage(text: #"{"type":"welcome","tag":1,"snapshot":null,"rev":0,"member":1,"role":"member","hidden":[5,6]}"#) else { return XCTFail() }
        XCTAssertEqual(w.hidden, [5, 6])
    }
}

/// A field set back to its default by a Mac copy that leaves defaults out
/// (SyncMeta.v 2) stays at its default here too (wb_sync Ctx::omits_defaults).
final class DefaultFieldTests: XCTestCase {
    func testAStoppedTimerDoesNotComeBack() throws {
        var phone = SampleWorkspace.make()
        var task = phone["tasks"]?.array?.first ?? [:]
        let id = task["id"]?.uint ?? 0
        task["timer_by"] = 2
        phone.set("tasks", .array([task]))
        var pm = SyncMeta()
        Sync.stamp(phone, &pm, now: 10)
        phone.set("extra.sync", pm.json)
        // the Mac stopped the timer later: the field left its copy
        var mac = phone
        var mo = task.object ?? [:]
        mo.removeValue(forKey: "timer_by")
        let mtask = JSON.object(mo)
        mac.set("tasks", .array([mtask]))
        var mm = SyncMeta(stamps: [id: 50], v: 2)
        mm.hashes = pm.hashes
        mac.set("extra.sync", mm.json)
        Sync.merge(&phone, [mac])
        XCTAssertNil(phone["tasks"]?.array?.first?["timer_by"], "a stopped timer doesn't come back")
        // a copy from an app that doesn't leave defaults out still keeps what it doesn't know
        var phone2 = SampleWorkspace.make()
        var t2 = phone2["tasks"]?.array?.first ?? [:]
        t2["watchers"] = [4]
        phone2.set("tasks", .array([t2]))
        var old = phone2
        var oo = t2.object ?? [:]
        oo.removeValue(forKey: "watchers")
        let ot = JSON.object(oo)
        old.set("tasks", .array([ot]))
        old.set("extra.sync", SyncMeta(stamps: [id: 50], v: 1).json)
        Sync.merge(&phone2, [old])
        XCTAssertEqual(phone2["tasks"]?.array?.first?["watchers"], [4])
    }
}
