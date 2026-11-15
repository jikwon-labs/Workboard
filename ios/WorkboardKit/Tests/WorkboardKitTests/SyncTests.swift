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
        XCTAssertEqual(m.v, Sync.metaV)
    }

    func testRenamingAListYouNeverSawMoveKeepsTheMove() {
        var (a, ma, b, mb) = pair([space(100, [], [list(101)]), space(200, [], [])])
        // A moves the list; later B, not knowing, renames it where B has it
        change(&a, &ma, [space(100, [], []), space(200, [], [list(101)])], 30)
        var renamed = list(101)
        renamed["name"] = "Renamed by B"
        change(&b, &mb, [space(100, [], [renamed]), space(200, [], [])], 40)
        let (a0, b0) = (a, b)
        Sync.merge(&a, [b0])
        Sync.merge(&b, [a0])
        for v in [a, b] {
            XCTAssertEqual(places(v), [[200, 101]], "the move stays")
            XCTAssertEqual(v["spaces"]?[1]?["lists"]?[0]?["name"], "Renamed by B", "and so does the rename")
        }
        XCTAssertEqual(SyncMeta(of: a).moved[101], 30, "the move time travels with the copy")
        // a copy from an app that doesn't keep move times still moves it by its stamp
        var (c, _, old, mo) = pair([space(100, [], [list(101)]), space(200, [], [])])
        change(&old, &mo, [space(100, [], []), space(200, [], [list(101)])], 30)
        var om = SyncMeta(of: old)
        om.v = 1
        om.moved = [:]
        old.set("extra.sync", om.json)
        Sync.merge(&c, [old])
        XCTAssertEqual(places(c), [[200, 101]])
    }

    func testAPhoneCopyNoLongerUndoesAMacMove() {
        // the Mac (a) moves a list; the phone, which renamed another list,
        // merges it and pushes its copy back: the move stays
        var (mac, mm, phone, pm) = pair([space(100, [], [list(101), list(102)]), space(200, [], [])])
        change(&mac, &mm, [space(100, [], [list(102)]), space(200, [], [list(101)])], 30)
        var r = list(102)
        r["name"] = "renamed"
        change(&phone, &pm, [space(100, [], [list(101), r]), space(200, [], [])], 40)
        Sync.merge(&phone, [mac])
        XCTAssertEqual(SyncMeta(of: phone).v, Sync.metaV)
        XCTAssertNotNil(SyncMeta(of: phone).moved[101])
        Sync.merge(&mac, [Sync.outgoing(phone)])
        XCTAssertEqual(places(mac), [[100, 102], [200, 101]])
        XCTAssertEqual(mac["spaces"]?[0]?["lists"]?[0]?["name"], "renamed")
    }

    func testBookmarksAreShared() {
        var (a, ma, b, _) = pair([])
        a.set("extra.bookmarks", [["id": 5, "title": "Docs", "url": "https://example.com"]])
        Sync.stamp(a, &ma, now: 20)
        a.set("extra.sync", ma.json)
        Sync.merge(&b, [a])
        XCTAssertEqual(b.at("extra.bookmarks")?[0]?["title"], "Docs")
    }

    func testCarryHashesKeepsTheOldShapeFromCountingAsAnEdit() {
        var d = sp([], SyncMeta())
        d.set("extra.forms", [["id": 2, "name": "F", "list_id": 3, "description": "", "fields": []]])
        var m = SyncMeta(hashes: Sync.fingerprints(Sync.legacyShape(d)), v: 1)
        m.stamps = [2: 10]
        Sync.carryHashes(&m, legacy: Sync.legacyShape(d), current: d)
        Sync.stamp(d, &m, now: 99)
        XCTAssertEqual(m.stamps[2], 10, "nothing counts as edited")
        XCTAssertEqual(Sync.legacyShape(d).at("extra.forms")?[0]?["owner"], 0)
    }

    func testAnOlderCopyRecordsMoveTimesOnlyForRealMoves() {
        var (a, _, old, mo) = pair([space(100, [], [list(101), list(102)]), space(200, [], [])])
        a["tasks"] = [["id": 7, "name": "t"]]
        var ma = SyncMeta(of: a)
        Sync.stamp(a, &ma, now: 15)
        a.set("extra.sync", ma.json)
        change(&old, &mo, [space(100, [], [list(102)]), space(200, [], [list(101)])], 30)
        var om = SyncMeta(of: old)
        om.v = 1
        om.moved = [:]
        old.set("extra.sync", om.json)
        Sync.merge(&a, [old])
        XCTAssertEqual(places(a), [[100, 102], [200, 101]])
        XCTAssertEqual(Array(SyncMeta(of: a).moved.keys), [101], "only the list that moved")
        var bloated: [UInt64: Int64] = [7: 5, 100: 5, 101: 30, 102: 5, 999: 5]
        Sync.shrinkMoved(&bloated, a)
        XCTAssertEqual(bloated, [101: 30, 102: 5])
    }

    func testPersonalItemsStayOnThisDevice() {
        var (a, _, _, _) = pair([])
        a.set("extra.bookmarks", [["id": 5, "title": "Mine", "url": "u", "personal": true], ["id": 6, "title": "Team", "url": "v"]])
        XCTAssertEqual(Sync.outgoing(a).at("extra.bookmarks"), [["id": 6, "title": "Team", "url": "v"]])
    }

    func testHelloSaysTheAppVersionAndSyncFormat() {
        let h = Hello(device: "d1", app: "1.0", sync: UInt64(Sync.metaV))
        let text = ClientMessage.hello(h).text
        XCTAssertTrue(text.contains(#""app":"1.0""#) && text.contains(#""sync":4"#), text)
        XCTAssertEqual(ClientMessage(text: text), .hello(h))
    }

    func testMetaKeepsMoveTimesAndLeavesThisDevicesPlacesOut() throws {
        var m = SyncMeta(stamps: [1: 10], v: Sync.metaV)
        m.moved = [101: 30]
        m.parents = [101: 200]
        m.fields = [1: ["": 5, "name": 10]]
        XCTAssertEqual(SyncMeta(json: try JSON(parsing: m.json.serialized())), m)
        let out = Sync.outgoing(["extra": ["sync": m.json]])
        XCTAssertEqual(out.at("extra.sync.moved.101"), 30)
        XCTAssertNil(out.at("extra.sync.parents"))
        XCTAssertEqual(out.at("extra.sync.fields.1.name"), 10)
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

/// Field-level merging (wb_sync merge_fields), ported one to one from the
/// Rust tests of the same names.
final class FieldMergeTests: XCTestCase {
    func ws(_ tasks: JSON, _ channels: JSON, _ meta: SyncMeta) -> JSON {
        ["workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": channels, "docs": [], "extra": ["sync": meta.json]]
    }

    func save(_ v: inout JSON, _ m: inout SyncMeta, _ now: Int64) {
        Sync.fillActivityIds(&v)
        Sync.fillBoardIds(&v)
        Sync.stamp(v, &m, now: now)
        v.set("extra.sync", m.json)
    }

    func swap(_ a: inout JSON, _ b: inout JSON) {
        let (a0, b0) = (a, b)
        Sync.merge(&a, [b0])
        Sync.merge(&b, [a0])
    }

    func setTask(_ v: inout JSON, _ i: Int, _ k: String, _ x: JSON?) {
        var tasks = v["tasks"]?.array ?? []
        tasks[i][k] = x
        v["tasks"] = .array(tasks)
    }

    func testTwoPeopleChangingDifferentFieldsOfOneTaskKeepBoth() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "status": "to do", "priority": 0]], [], ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        setTask(&a, 0, "name", "renamed by A")
        save(&a, &ma, 20)
        setTask(&b, 0, "status", "done")
        save(&b, &mb, 30)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["name"], "renamed by A")
            XCTAssertEqual(v["tasks"]?[0]?["status"], "done")
        }
        XCTAssertEqual(SyncMeta(of: a).fields, SyncMeta(of: b).fields, "both sides agree")
        // the same field: the newest change wins on both sides
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        setTask(&b, 0, "name", "B early")
        save(&b, &mb, 40)
        setTask(&a, 0, "name", "A late")
        setTask(&a, 0, "priority", 2)
        save(&a, &ma, 50)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["name"], "A late")
            XCTAssertEqual(v["tasks"]?[0]?["priority"], 2)
            XCTAssertEqual(v["tasks"]?[0]?["status"], "done")
        }
        var a2 = a
        XCTAssertFalse(Sync.merge(&a2, [b]), "nothing left to merge")
        // a field taken away stays away while another field changes elsewhere
        ma = SyncMeta(of: a)
        setTask(&a, 0, "watchers", [7])
        save(&a, &ma, 60)
        Sync.merge(&b, [a])
        mb = SyncMeta(of: b)
        setTask(&b, 0, "watchers", nil)
        save(&b, &mb, 70)
        setTask(&a, 0, "name", "A again")
        save(&a, &ma, 80)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["name"], "A again")
            XCTAssertNil(v["tasks"]?[0]?["watchers"])
        }
    }

    func testACopyWithoutFieldStampsCountsEveryFieldAtItsStamp() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "status": "to do"]], [], ma)
        save(&a, &ma, 10)
        var old = a
        setTask(&a, 0, "status", "done")
        save(&a, &ma, 20)
        setTask(&old, 0, "name", "old app")
        old.set("extra.sync", ["stamps": ["1": 30], "v": 2])
        Sync.merge(&a, [old])
        XCTAssertEqual(a["tasks"]?[0]?["name"], "old app")
        XCTAssertEqual(a["tasks"]?[0]?["status"], "to do")
    }

    func chat(_ reactions: JSON) -> JSON {
        [["id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [["id": 11, "author": 1, "text": "hi", "ts": 5, "reactions": reactions]]]]
    }

    func pairs(_ v: JSON) -> [String] {
        (Sync.reactionPairs(v.at("channels")?[0]?["messages"]?[0]?["reactions"] ?? []) ?? []).map { "\($0.0)/\($0.1)" }.sorted()
    }

    func setReactions(_ v: inout JSON, _ r: JSON) {
        var ch = v["channels"]!
        var chans = ch.array!
        var msgs = chans[0]["messages"]!.array!
        msgs[0]["reactions"] = r
        chans[0]["messages"] = .array(msgs)
        ch = .array(chans)
        v["channels"] = ch
    }

    func testReactionsMadeAtOnceAreAllKeptAndRemovalsStick() {
        var ma = SyncMeta()
        var a = ws([], chat([]), ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        a["channels"] = chat([["emoji": "👍", "users": [1]]])
        save(&a, &ma, 20)
        b["channels"] = chat([["emoji": "❤️", "users": [2]], ["emoji": "👍", "users": [2]]])
        save(&b, &mb, 30)
        swap(&a, &b)
        XCTAssertEqual(pairs(a), ["❤️/2", "👍/1", "👍/2"].sorted())
        XCTAssertEqual(a.at("channels")?[0]?["messages"]?[0]?["reactions"], b.at("channels")?[0]?["messages"]?[0]?["reactions"])
        // A takes back their 👍 while B, not knowing, adds 🎉
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        setReactions(&a, [["emoji": "👍", "users": [2]], ["emoji": "❤️", "users": [2]]])
        save(&a, &ma, 40)
        setReactions(&b, [["emoji": "👍", "users": [1, 2]], ["emoji": "❤️", "users": [2]], ["emoji": "🎉", "users": [2]]])
        save(&b, &mb, 50)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(pairs(v), ["❤️/2", "🎉/2", "👍/2"].sorted(), "the removed 👍 stays removed")
        }
    }

    func testTimersStartedAtOnceBothKeepRunning() {
        func task(_ extra: [String: JSON]) -> JSON {
            var t: [String: JSON] = ["id": 1, "name": "t", "timer_started": .null]
            for (k, v) in extra { t[k] = v }
            return [.object(t)]
        }
        var ma = SyncMeta()
        var a = ws(task([:]), [], ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        a["tasks"] = task(["timer_started": 100, "timer_by": 1])
        save(&a, &ma, 20)
        b["tasks"] = task(["timer_started": 105, "timer_by": 2])
        save(&b, &mb, 21)
        swap(&a, &b)
        for v in [a, b] {
            let t = v["tasks"]?[0]
            XCTAssertEqual(t?["timer_started"], 100)
            XCTAssertEqual(t?["timer_by"], 1)
            XCTAssertEqual(t?["timers"], [["member": 2, "start": 105]])
        }
        // A stops theirs (B's moves into the old field) while B renames the task
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        a["tasks"] = task(["timer_started": 105, "timer_by": 2])
        save(&a, &ma, 30)
        setTask(&b, 0, "name", "renamed")
        save(&b, &mb, 40)
        swap(&a, &b)
        for v in [a, b] {
            let t = v["tasks"]?[0]
            XCTAssertEqual(t?["name"], "renamed")
            XCTAssertEqual(t?["timer_started"], 105, "A's stop sticks")
            XCTAssertEqual(t?["timer_by"], 2)
            XCTAssertNil(t?["timers"])
        }
    }

    func testStrokesDrawnAtOnceAreAllKeptAndErasingSticks() {
        func stroke(_ x: Double) -> JSON { ["color": [0, 0, 0], "width": 2.0, "points": [[.double(x), 1.25], [.double(x + 1), 2.0]]] }
        func board(_ strokes: JSON) -> JSON { [["id": 50, "name": "B", "strokes": strokes, "notes": [["pos": [1.0, 2.0], "text": "old", "color": [1, 2, 3]]]]] }
        var ma = SyncMeta()
        var a = ws([], [], ma)
        a.set("extra.whiteboards", board([stroke(0.1)]))
        var old = a
        save(&a, &ma, 10)
        Sync.fillBoardIds(&old)
        let xid = a.at("extra.whiteboards")?[0]?["strokes"]?[0]?["id"]
        XCTAssertGreaterThanOrEqual(xid?.uint ?? 0, 1 << 62)
        XCTAssertEqual(old.at("extra.whiteboards")?[0]?["strokes"]?[0]?["id"], xid)
        XCTAssertEqual(old.at("extra.whiteboards")?[0]?["notes"]?[0]?["id"], a.at("extra.whiteboards")?[0]?["notes"]?[0]?["id"])
        var (b, mb) = (a, ma)
        var wa = a.at("extra.whiteboards")!.array!
        wa[0]["strokes"] = [stroke(5)]
        a.set("extra.whiteboards", .array(wa))
        save(&a, &ma, 20)
        var wb = b.at("extra.whiteboards")!.array!
        wb[0]["strokes"] = .array(wb[0]["strokes"]!.array! + [stroke(9)])
        b.set("extra.whiteboards", .array(wb))
        save(&b, &mb, 30)
        swap(&a, &b)
        for v in [a, b] {
            let xs = (v.at("extra.whiteboards")?[0]?["strokes"]?.array ?? []).compactMap { $0["points"]?[0]?[0]?.double }.sorted()
            XCTAssertEqual(xs, [5, 9])
        }
    }

    func testBoardIdsMatchTheRustOnes() {
        // values printed by wb_sync::board_item_id
        XCTAssertEqual(Sync.boardItemId(board: 50, kind: "strokes", sig: "0,0,0|1,13;11,20", nth: 0), 6148156056053566450)
        XCTAssertEqual(Sync.boardSig("strokes", ["color": [0, 0, 0], "points": [[0.1, 1.25], [1.1, 2.0]]]), "0,0,0|1,13;11,20")
        XCTAssertEqual(Sync.boardSig("notes", ["color": [1, 2, 3], "text": "old", "pos": [1.0, 2.0]]), "1,2,3|old|10|20")
    }

    // MARK: round 4 (wb_sync tests of the same names)

    func testAStrokeWithoutAnIdLeavesTheKnownStrokesAlone() {
        func stroke(_ id: UInt64, _ x: Double) -> JSON {
            var s: JSON = ["color": [0, 0, 0], "width": 2.0, "points": [[.double(x), 1.0]]]
            if id != 0 { s["id"] = .id(id) }
            return s
        }
        var m = SyncMeta()
        var a = ws([], [], m)
        a.set("extra.whiteboards", [["id": 50, "name": "B", "strokes": [stroke(51, 1), stroke(52, 2)], "notes": []]])
        Sync.stamp(a, &m, now: 10)
        var withZero = stroke(0, 4)
        withZero["id"] = 0
        for fresh in [stroke(0, 3), withZero] {
            var m2 = m
            var b = a
            var boards = b.at("extra.whiteboards")!.array!
            boards[0]["strokes"] = .array(boards[0]["strokes"]!.array! + [fresh])
            b.set("extra.whiteboards", .array(boards))
            Sync.stamp(b, &m2, now: 20)
            XCTAssertTrue(m2.tombs.isEmpty, "nothing deleted: \(m2.tombs)")
            XCTAssertNotNil(m2.stamps[51])
            XCTAssertNotNil(m2.stamps[52])
            XCTAssertGreaterThan(m2.stamps[50]!, m.stamps[50]!, "the board changed")
        }
    }

    func testEntriesWithoutAnIdStayBesideTheItemsTheySitWith() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "things": [["id": 5, "name": "five"], ["id": 6, "name": "six"]]]], [], ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        setTask(&a, 0, "things", [["id": 5, "name": "five"], ["id": 6, "name": "six"], ["name": "loose"]])
        save(&a, &ma, 20)
        XCTAssertTrue(ma.tombs.isEmpty)
        setTask(&b, 0, "things", [["id": 5, "name": "five"], ["id": 6, "name": "six by B"]])
        setTask(&b, 0, "name", "renamed by B")
        save(&b, &mb, 30)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["things"], [["id": 5, "name": "five"], ["id": 6, "name": "six by B"], ["name": "loose"]])
            XCTAssertEqual(v["tasks"]?[0]?["name"], "renamed by B")
        }
        var since = SyncSummary(of: b)
        var mb2 = SyncMeta(of: b)
        mb2.fieldHashes = Sync.partHashes(b)
        setTask(&b, 0, "name", "again")
        save(&b, &mb2, 40)
        let d = Sync.delta(since: since, b)!
        XCTAssertEqual(d["tasks"]?[0]?["things"], [["name": "loose"]])
        since.absorb(d)
        XCTAssertNil(Sync.delta(since: since, b))
    }

    func entriesOf(_ v: JSON) -> [[Int64]] {
        (v["tasks"]?[0]?["time_entries"]?.array ?? []).map { [Int64($0["member"]?.uint ?? 0), $0["start"]?.int ?? 0, $0["end"]?.int ?? 0] }
    }

    func testTwoPeopleStoppingTheirTimersAtOnceBothKeepTheirTime() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "time_entries": [["start": 1, "end": 5]]]], [], ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        setTask(&a, 0, "time_entries", [["start": 1, "end": 5], ["start": 100, "end": 200, "member": 1]])
        save(&a, &ma, 20)
        setTask(&b, 0, "time_entries", [["start": 1, "end": 5], ["start": 90, "end": 201, "member": 2]])
        save(&b, &mb, 21)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(entriesOf(v), [[0, 1, 5], [2, 90, 201], [1, 100, 200]])
        }
        XCTAssertEqual(a["tasks"], b["tasks"])
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        ma.fieldHashes = Sync.partHashes(a)
        mb.fieldHashes = Sync.partHashes(b)
        setTask(&a, 0, "time_entries", [["start": 90, "end": 150, "member": 2], ["start": 100, "end": 200, "member": 1]])
        save(&a, &ma, 30)
        setTask(&b, 0, "name", "renamed")
        save(&b, &mb, 40)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["name"], "renamed")
            XCTAssertEqual(entriesOf(v), [[2, 90, 150], [1, 100, 200]], "the deleted entry stays deleted")
        }
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        ma.fieldHashes = Sync.partHashes(a)
        mb.fieldHashes = Sync.partHashes(b)
        setTask(&a, 0, "time_entries", [])
        save(&a, &ma, 50)
        setTask(&b, 0, "name", "again")
        save(&b, &mb, 60)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["time_entries"], [])
        }
    }

    func testAWholeFieldStampFromAnOlderAppCountsForEachEntry() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "time_entries": [["start": 1, "end": 5]], "custom": ["7": "x"]]], [], ma)
        save(&a, &ma, 10)
        var old = a
        setTask(&old, 0, "time_entries", [["start": 1, "end": 9]])
        setTask(&old, 0, "custom", ["7": "y"])
        old.set("extra.sync", ["v": 3, "stamps": ["1": 30], "fields": ["1": ["": 10, "time_entries": 30, "custom": 30]]])
        setTask(&a, 0, "name", "A")
        save(&a, &ma, 20)
        Sync.merge(&a, [old])
        XCTAssertEqual(entriesOf(a), [[0, 1, 9]])
        XCTAssertEqual(a["tasks"]?[0]?["custom"], ["7": "y"])
        XCTAssertEqual(a["tasks"]?[0]?["name"], "A")
        // fingerprints kept from before: the new split alone is no change
        var m = SyncMeta()
        let v = ws([["id": 1, "name": "t", "time_entries": [["start": 1, "end": 5]], "custom": ["7": "x"]]], [], m)
        Sync.stamp(v, &m, now: 10)
        var h = m.fieldHashes[1]!.filter { Sync.partField($0.key) != "time_entries" && Sync.partField($0.key) != "custom" }
        h["time_entries"] = Sync.partHash([["start": 1, "end": 5]])
        h["custom"] = Sync.partHash(["7": "x"])
        m.fieldHashes[1] = h
        var v2 = v
        setTask(&v2, 0, "name", "renamed")
        Sync.stamp(v2, &m, now: 20)
        XCTAssertTrue(m.fields[1]!.keys.allSatisfy { $0 == "name" || $0.isEmpty }, "only the name changed: \(m.fields[1]!)")
    }

    func testDifferentCustomFieldsChangedAtOnceBothStay() {
        var ma = SyncMeta()
        var a = ws([["id": 1, "name": "t", "custom": ["7": "x", "8": "y"]]], [], ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        setTask(&a, 0, "custom", ["7": "x by A", "8": "y"])
        save(&a, &ma, 20)
        setTask(&b, 0, "custom", ["7": "x", "8": "y", "9": "z by B"])
        save(&b, &mb, 30)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["custom"], ["7": "x by A", "8": "y", "9": "z by B"])
        }
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        ma.fieldHashes = Sync.partHashes(a)
        mb.fieldHashes = Sync.partHashes(b)
        setTask(&a, 0, "custom", ["7": "x by A", "9": "z by B"])
        save(&a, &ma, 40)
        setTask(&b, 0, "custom", ["7": "x by A", "8": "y", "9": "z again"])
        save(&b, &mb, 50)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(v["tasks"]?[0]?["custom"], ["7": "x by A", "9": "z again"])
        }
    }

    func testReactionsEndUpInTheSameOrderEverywhere() {
        var ma = SyncMeta()
        var a = ws([], chat([]), ma)
        save(&a, &ma, 10)
        var (b, mb) = (a, ma)
        a["channels"] = chat([["emoji": "👍", "users": [2, 1]], ["emoji": "❤️", "users": [3]]])
        save(&a, &ma, 20)
        b["channels"] = chat([["emoji": "❤️", "users": [3]], ["emoji": "👍", "users": [1, 2]]])
        save(&b, &mb, 20)
        swap(&a, &b)
        XCTAssertEqual(a.at("channels")?[0]?["messages"]?[0]?["reactions"], b.at("channels")?[0]?["messages"]?[0]?["reactions"])
        XCTAssertEqual(Sync.fingerprints(a), Sync.fingerprints(b))
        XCTAssertEqual(a.at("channels")?[0]?["messages"]?[0]?["reactions"], [["emoji": "❤️", "users": [3]], ["emoji": "👍", "users": [1, 2]]])
    }

    func testAFormat3CopyBecomesFormat4WithoutCountingAsAnEdit() {
        let t: JSON = [["id": 1, "name": "t", "time_entries": [["start": 1, "end": 5, "member": 2]], "custom": ["7": "x"]]]
        // saved by a round-3 app: one fingerprint for each whole field
        var old = SyncMeta()
        let v = ws(t, [], old)
        Sync.stamp(v, &old, now: 10)
        var h = old.fieldHashes[1]!.filter { Sync.partField($0.key) != "time_entries" && Sync.partField($0.key) != "custom" }
        h["time_entries"] = Sync.partHash([["start": 1, "end": 5, "member": 2]])
        h["custom"] = Sync.partHash(["7": "x"])
        old.fieldHashes[1] = h
        old.v = 3
        // and by a round-4 app, which split them already but wrote 3
        var split = SyncMeta()
        Sync.stamp(v, &split, now: 10)
        split.v = 3
        for var m in [old, split] {
            let before = (m.stamps, m.fields)
            Sync.stamp(v, &m, now: 20)
            XCTAssertEqual(m.v, Sync.metaV)
            XCTAssertEqual(Sync.metaV, 4, "the same as wb_sync::META_V")
            XCTAssertEqual(m.stamps, before.0, "the new format alone is no change")
            XCTAssertEqual(m.fields, before.1)
            let parts = m.fieldHashes[1]!
            XCTAssertNotNil(parts["time_entries/2/1"], "\(parts)")
            XCTAssertNotNil(parts["custom/7"])
            XCTAssertNil(parts["time_entries"])
            XCTAssertNil(parts["custom"])
        }
    }

    func slackMap(_ v: JSON) -> JSON { v.at("extra.slack_map") ?? .null }

    func setSlack(_ v: inout JSON, _ key: String, _ x: JSON?) {
        var m = v.at("extra.slack_map")?.object ?? [:]
        m[key] = x
        v.set("extra.slack_map", .object(m))
    }

    /// The same steps and results as wb_sync's
    /// `shared_map_entries_merge_one_by_one`: the phone merges a shared map
    /// exactly like a Mac.
    func testSharedMapEntriesMergeOneByOne() {
        var ma = SyncMeta()
        var a = ws([], [], ma)
        a.set("extra.slack_map", ["U0": 7])
        save(&a, &ma, 10)
        XCTAssertTrue(ma.values.isEmpty, "a copy stamped for the first time keeps no stamp")
        var (b, mb) = (a, ma)
        // A points UA at 2, then B points UA at 3 and UB at 2
        setSlack(&a, "UA", 2)
        save(&a, &ma, 20)
        XCTAssertEqual(ma.values["extra.slack_map/UA"], 20)
        setSlack(&b, "UA", 3)
        setSlack(&b, "UB", 2)
        save(&b, &mb, 30)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(slackMap(v), ["U0": 7, "UA": 3, "UB": 2])
            XCTAssertEqual(SyncMeta(of: v).values["extra.slack_map/UA"], 30)
        }
        // at once: A moves U0, B moves UB; both stay
        ma = SyncMeta(of: a)
        mb = SyncMeta(of: b)
        ma.valueHashes = Sync.valueHashes(a)
        mb.valueHashes = Sync.valueHashes(b)
        setSlack(&a, "U0", 9)
        save(&a, &ma, 40)
        setSlack(&b, "UB", 4)
        save(&b, &mb, 41)
        // and later B takes UA away
        setSlack(&b, "UA", nil)
        save(&b, &mb, 50)
        swap(&a, &b)
        for v in [a, b] {
            XCTAssertEqual(slackMap(v), ["U0": 9, "UB": 4])
            XCTAssertEqual(SyncMeta(of: v).values["extra.slack_map/UA"], 50)
        }
        // a copy that never had UA set (stamp 0) doesn't bring it back, and
        // one set again later does come back
        var old = ws([], [], SyncMeta())
        old.set("extra.slack_map", ["UA": 2, "UC": 5])
        var x = a
        Sync.merge(&x, [old])
        XCTAssertEqual(slackMap(x), ["U0": 9, "UB": 4, "UC": 5])
        // two copies that each worked out an entry nobody decided (stamp 0)
        // end up with the same one
        var p = ws([], [], SyncMeta())
        p.set("extra.slack_map", ["UD": 2])
        var q = ws([], [], SyncMeta())
        q.set("extra.slack_map", ["UD": 3])
        swap(&p, &q)
        XCTAssertEqual(slackMap(p), ["UD": 3])
        XCTAssertEqual(slackMap(q), ["UD": 3])
        // a copy without the map gets none made up
        var e = ws([], [], SyncMeta())
        Sync.merge(&e, [ws([], [], SyncMeta())])
        XCTAssertNil(e.at("extra.slack_map"))
    }

    func testACarriedMapEntryIsNotADecision() {
        var m = SyncMeta()
        var a = ws([], [], m)
        save(&a, &m, 10)
        // an upgrade works one out of older data
        a.set("extra.slack_map", ["UA": 2])
        Sync.carryMapEntry(&m, "extra.slack_map", "UA", 2)
        save(&a, &m, 20)
        XCTAssertNil(m.values["extra.slack_map/UA"], "not set now")
        // a decision on another copy wins over it
        var b = a
        var mb = m
        setSlack(&b, "UA", 3)
        save(&b, &mb, 15)
        Sync.merge(&a, [b])
        XCTAssertEqual(slackMap(a), ["UA": 3])
        // without the carry it would have counted as set now
        var n = SyncMeta()
        var c = ws([], [], n)
        save(&c, &n, 10)
        c.set("extra.slack_map", ["UA": 2])
        save(&c, &n, 20)
        XCTAssertEqual(n.values["extra.slack_map/UA"], 20)
    }

    func testADeltaCarriesOnlyTheChangedMapEntries() {
        var m = SyncMeta()
        var a = ws([], [], m)
        a.set("extra.slack_map", ["UA": 2, "UB": 3])
        save(&a, &m, 10)
        let other = a
        var sum = SyncSummary(of: Sync.outgoing(a))
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)))
        setSlack(&a, "UA", 4)
        setSlack(&a, "UC", 5)
        setSlack(&a, "UB", nil)
        save(&a, &m, 20)
        let d = Sync.delta(since: sum, Sync.outgoing(a))!
        XCTAssertEqual(d.at("extra.slack_map"), ["UA": 4, "UC": 5])
        XCTAssertEqual(d.at("extra.sync")?["values"]?["extra.slack_map/UB"], 20)
        var (x, y) = (other, other)
        Sync.merge(&x, [d])
        Sync.merge(&y, [Sync.outgoing(a)])
        XCTAssertEqual(slackMap(x), ["UA": 4, "UC": 5])
        XCTAssertEqual(slackMap(x), slackMap(y))
        XCTAssertEqual(SyncMeta(of: x).values, SyncMeta(of: y).values)
        sum.absorb(d)
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)))
    }

    func testAnOlderAppThatDropsTheMapTakesNoEntryAway() {
        var m = SyncMeta()
        var a = ws([], [], m)
        save(&a, &m, 10)
        a.set("extra.slack_map", ["UA": 2])
        save(&a, &m, 20)
        // an older app kept the stamps but not the map, and fingerprints
        // of the shared settings only
        var old = a
        var extra = old["extra"]!
        extra["slack_map"] = nil
        old["extra"] = extra
        var om = SyncMeta(of: old)
        om.valueHashes = om.valueHashes.filter { Sync.sharedValues.contains($0.key) }
        XCTAssertEqual(om.values["extra.slack_map/UA"], 20)
        old.set("extra.sync", om.json)
        var x = a
        Sync.merge(&x, [old])
        XCTAssertEqual(slackMap(x), ["UA": 2])
        var y = old
        Sync.merge(&y, [a])
        XCTAssertEqual(slackMap(y), ["UA": 2])
    }
}
