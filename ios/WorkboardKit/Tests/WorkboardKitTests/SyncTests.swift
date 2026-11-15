import XCTest
@testable import WorkboardKit

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
        Sync.stamp(d, &m, now: 100, pastSeen: true)
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
