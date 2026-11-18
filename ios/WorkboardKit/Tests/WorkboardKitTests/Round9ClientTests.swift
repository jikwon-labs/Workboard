import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Round 9: a copy saved by a build from before shared maps.
@MainActor
final class Round9ClientTests: FakeServerTestCase {
    /// An older build kept the Slack matches the team's copy brought in
    /// (it keeps fields it doesn't know) but fingerprinted only the shared
    /// settings. The first edit after the update, made offline, is not a
    /// new decision for every match: a teammate's later one still wins
    /// once the phone is back with the team.
    func testAnOlderCopysSlackMatchesAreNotEditsAfterTheUpdate() async throws {
        var base = SampleWorkspace.make()
        // ids an older build would have given already
        Sync.fillActivityIds(&base)
        Sync.fillBoardIds(&base)
        var mine = base
        mine.set("extra.slack_map", ["UA": 2])
        var m = SyncMeta(of: mine)
        m.hashes = Sync.fingerprints(mine)
        m.fieldHashes = Sync.partHashes(mine)
        m.parents = Sync.parentsOf(mine)
        m.valueHashes = Sync.valueHashes(mine).filter { Sync.sharedValues.contains($0.key) }
        m.values["extra.slack_map/UA"] = 100
        m.v = Sync.metaV
        mine.set("extra.sync", m.json)
        let s = storage()
        s.save(snapshot: mine)

        // a teammate pointed UA at member 3 after that
        var team = base
        team.set("extra.slack_map", ["UA": 3])
        var tm = SyncMeta(of: team)
        tm.values["extra.slack_map/UA"] = 200
        tm.valueHashes = Sync.valueHashes(team)
        team.set("extra.sync", tm.json)
        let server = FakeServer(snapshot: team)
        let port = try await server.start(port: 0)
        defer { server.stop() }

        let c = SyncClient(storage: s, tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setMe(1)
        XCTAssertTrue(c.edit { d, _ in Edits.rename(&d, task: 201, to: "Renamed offline", ts: nowTs()) })
        let after = SyncMeta(of: try XCTUnwrap(c.data))
        XCTAssertEqual(after.values["extra.slack_map/UA"], 100, "the old match is nobody's new decision")
        XCTAssertNotNil(after.valueHashes["extra.slack_map/UA"])

        c.setServer("ws://127.0.0.1:\(port)")
        await until("merged with the team") { c.updatesReceived > 0 }
        XCTAssertEqual(c.data?.at("extra.slack_map")?["UA"], 3, "the teammate's later match wins")
        XCTAssertEqual(c.view.tasks.first { $0.id == 201 }?.name, "Renamed offline")
    }
}
