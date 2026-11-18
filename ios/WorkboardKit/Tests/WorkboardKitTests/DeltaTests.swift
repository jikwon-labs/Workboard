import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Deltas (wb_sync::delta, wb-net's session): after the first whole copy
/// only what changed travels, both ways.
@MainActor
final class DeltaTests: FakeServerTestCase {
    func save(_ v: inout JSON, _ now: Int64) {
        var m = SyncMeta(of: v)
        Sync.stamp(v, &m, now: now)
        v.set("extra.sync", m.json)
    }

    /// The Rust test a_delta_holds_only_what_changed_and_merges_like_the_whole_copy.
    func testADeltaHoldsOnlyWhatChangedAndMergesLikeTheWholeCopy() {
        var a = ws([["id": 1, "name": "one"], ["id": 2, "name": "two"]],
                   [["id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [["id": 11, "author": 1, "text": "hi", "ts": 5]]]])
        a["spaces"] = [["id": 100, "name": "S", "folders": [], "lists": [["id": 101, "name": "L"]]], ["id": 200, "name": "T", "folders": [], "lists": []]]
        save(&a, 10)
        let other = a
        var sum = SyncSummary(of: Sync.outgoing(a))
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)), "nothing changed")
        var chans = a["channels"]!.array!
        chans[0]["messages"] = .array(chans[0]["messages"]!.array! + [["id": 12, "author": 1, "text": "new", "ts": 6]])
        a["channels"] = .array(chans)
        a["tasks"] = [["id": 1, "name": "one!"]]
        a["spaces"] = [["id": 100, "name": "S", "folders": [], "lists": []], ["id": 200, "name": "T", "folders": [], "lists": [["id": 101, "name": "L"]]]]
        a["workspace"] = "Renamed"
        save(&a, 20)
        let d = Sync.delta(since: sum, Sync.outgoing(a))!
        XCTAssertEqual(d["tasks"], [["id": 1, "name": "one!"]])
        XCTAssertEqual(d["channels"]?[0]?["messages"]?.array?.count, 1, "only the new message")
        XCTAssertEqual(d["workspace"], "Renamed")
        XCTAssertNotNil(d.at("extra.sync.tombs.2"))
        XCTAssertNil(d["members"])
        var x = other, y = other
        Sync.merge(&x, [d])
        Sync.merge(&y, [Sync.outgoing(a)])
        XCTAssertEqual(Sync.stripSync(x), Sync.stripSync(y))
        XCTAssertEqual(SyncMeta(of: x).stamps, SyncMeta(of: y).stamps)
        sum.absorb(d)
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)), "once the server has it, nothing is left")
    }

    func testDeltaMessagesRoundTrip() {
        let m = ServerMessage.delta(data: ["tasks": []], rev: 5, base: 4, hidden: [9])
        XCTAssertEqual(ServerMessage(text: m.text), m)
        XCTAssertEqual(ServerMessage(text: #"{"type":"delta","data":{"x":1},"rev":3,"base":2}"#), .delta(data: ["x": 1], rev: 3, base: 2))
        XCTAssertEqual(ClientMessage(text: ClientMessage.resync.text), .resync)
        var h = Hello(device: "d")
        h.deltas = true
        XCTAssertEqual(ClientMessage.hello(h).json["deltas"], true)
        guard case let .welcome(w) = ServerMessage(text: #"{"type":"welcome","tag":1,"snapshot":null,"rev":0,"deltas":true}"#) else { return XCTFail() }
        XCTAssertTrue(w.deltas)
    }

    final class Pushes: @unchecked Sendable {
        let lock = NSLock()
        var all: [JSON] = []
        func add(_ j: JSON) { lock.lock(); all.append(j); lock.unlock() }
        var last: JSON? { lock.lock(); defer { lock.unlock() }; return all.last }
    }

    func testThePhonePushesAndGetsOnlyWhatChanged() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        server.deltas = true
        let pushes = Pushes()
        server.onPush = { pushes.add($0) }
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1), b = client(url, me: 2)
        await until("both connected") { a.canEdit && b.canEdit }
        a.edit { d, _ in Edits.rename(&d, task: 201, to: "renamed on A", ts: nowTs()) }
        await until("the rename at B") { b.view.tasks.first { $0.id == 201 }?.name == "renamed on A" }
        let p = try XCTUnwrap(pushes.last)
        XCTAssertEqual(p["tasks"]?.array?.compactMap { $0["id"]?.uint }, [201], "only the renamed task went up")
        XCTAssertNil(p["channels"])
        // two phones changing different fields of one task at once
        a.edit { d, _ in Edits.rename(&d, task: 202, to: "A's name", ts: nowTs()) }
        b.edit { d, me in Edits.setStatus(&d, task: 202, to: "in progress", me: me, ts: nowTs()) }
        for c in [a, b] {
            await until("both changes") {
                let t = c.view.tasks.first { $0.id == 202 }
                return t?.name == "A's name" && t?.status == "in progress"
            }
        }
        // a lost revision: the next delta doesn't follow, so B asks for everything
        server.dropNextDelta()
        a.edit { d, _ in Edits.rename(&d, task: 203, to: "lost on the way", ts: nowTs()) }
        try await Task.sleep(nanoseconds: 300_000_000)
        a.edit { d, _ in Edits.rename(&d, task: 204, to: "after the loss", ts: nowTs()) }
        await until("B caught up") {
            b.view.tasks.first { $0.id == 203 }?.name == "lost on the way"
                && b.view.tasks.first { $0.id == 204 }?.name == "after the loss"
        }
        XCTAssertGreaterThanOrEqual(server.resyncs, 1)
    }
}
