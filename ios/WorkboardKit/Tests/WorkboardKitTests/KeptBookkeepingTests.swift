import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The app keeps a copy's bookkeeping parsed beside it (Sync+Kept.swift):
/// the copy's tree holds the same shared parts, and snapshot.json comes out
/// as it always did.
@MainActor
final class KeptBookkeepingTests: FakeServerTestCase {
    func shared(_ m: SyncMeta) -> JSON { .object(m.sharedJSON) }

    /// What the tree says equals what the app keeps, and the fingerprints
    /// it keeps are the copy's.
    func assertKeptRight(_ c: SyncClient, file: StaticString = #filePath, line: UInt = #line) throws {
        let d = try XCTUnwrap(c.data)
        XCTAssertEqual(shared(SyncMeta(of: d)), shared(c.syncMeta), "shared parts", file: file, line: line)
        XCTAssertEqual(c.syncMeta.hashes, Sync.fingerprints(d), "fingerprints", file: file, line: line)
        XCTAssertEqual(c.syncMeta.fieldHashes, Sync.partHashes(d), "field fingerprints", file: file, line: line)
        XCTAssertEqual(c.syncMeta.parents, Sync.parentsOf(d), "places", file: file, line: line)
        XCTAssertNil(d.at("extra.sync.field_hashes"), "this device's parts stay out of the tree")
    }

    func testTheKeptBookkeepingFollowsEditsAndMerges() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let s = storage()
        let a = client(url, me: 1, storage: s)
        let b = client(url, me: 2)
        await until("both connected") { a.canEdit && b.canEdit }
        try assertKeptRight(a)

        XCTAssertTrue(a.edit { d, me in Edits.sendMessage(&d, channel: 301, text: "one", me: me, id: a.newId(), ts: nowTs()) })
        try assertKeptRight(a)
        XCTAssertTrue(a.edit { d, me in Edits.rename(&d, task: 201, to: "Renamed", me: me, ts: nowTs()) })
        XCTAssertTrue(a.edit { d, _ in Edits.deleteTask(&d, task: 202) })
        try assertKeptRight(a)
        XCTAssertNotNil(SyncMeta(of: a.data!).tombs[202], "the delete is in the tree for the server")

        b.edit { d, me in
            Edits.sendReply(&d, channel: 301, to: 313, text: "reply from b", me: me, id: b.newId(), ts: nowTs())
            Edits.addComment(&d, task: 201, text: "comment from b", me: me, ts: nowTs())
        }
        await until("b's changes at a") { a.view.channels.first { $0.id == 301 }?.messages.first { $0.id == 313 }?.replies.last?.text == "reply from b" }
        try assertKeptRight(a)
        await until("a's changes at b") { b.view.tasks.first { $0.id == 201 }?.name == "Renamed" && !b.view.tasks.contains { $0.id == 202 } }

        // the file is the whole copy, as before
        a.flushSnapshot()
        let file = try XCTUnwrap(s.loadSnapshot())
        XCTAssertEqual(SyncMeta(of: file), a.syncMeta)
        XCTAssertEqual(Sync.stripSync(file), Sync.stripSync(a.data!))
        // and opens the same again
        a.disconnect()
        let again = SyncClient(storage: s, tokens: MemoryTokenStore())
        XCTAssertEqual(again.syncMeta, a.syncMeta)
        XCTAssertEqual(again.data, a.data)
    }

    func testPatchingWritesWhatEncodingWrites() {
        var d = bigWorkspace(tasks: 20, channels: 2, messages: 10)
        let (tree, meta) = Sync.split(d)
        var t = tree
        var m = meta
        // changes of every kind
        m.stamps[1] = 5; m.stamps[meta.stamps.keys.first!] = nil
        m.tombs[2] = 6; m.revived[3] = 7; m.moved[4] = 8
        m.values["workspace"] = 9
        m.fields[10] = ["name": 11, "": 3]; m.fields[meta.fields.keys.first ?? 99] = nil
        m.v = 5
        Sync.patchShared(&t, old: meta, new: m)
        XCTAssertEqual(t.at("extra.sync"), .object(m.sharedJSON))
        // and back to nothing moved, no field stamps
        var m2 = m
        m2.moved = [:]
        m2.fields = [:]
        Sync.patchShared(&t, old: m, new: m2)
        XCTAssertEqual(t.at("extra.sync"), .object(m2.sharedJSON))
        d.set("extra.sync", m2.json)
        XCTAssertEqual(Sync.joined(t, m2), d)
    }
}
