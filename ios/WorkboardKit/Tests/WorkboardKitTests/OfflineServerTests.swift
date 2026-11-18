import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The fake server going away at the same address (what the offline UI test uses).
@MainActor
final class OfflineServerTests: FakeServerTestCase {
    func testAnEditMadeWhileTheServerIsAwayGoesUpWhenItIsBack() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.maxRetryDelay = 0.3
        c.setMe(1)
        c.setServer(url)
        await until("connected", timeout: 8) { c.state.isConnected && c.canEdit }
        server.goOffline(for: 30)
        await until("offline", timeout: 8) { if case .waiting = c.state { return true } else { return false } }
        try await Task.sleep(nanoseconds: 700_000_000)
        XCTAssertFalse(c.state.isConnected, "turned away while offline")
        c.edit { d, me in Edits.sendMessage(&d, channel: 302, text: "Written on the train", me: me, id: c.newId(), ts: nowTs()) }
        XCTAssertEqual(c.waitingCount, 1)
        server.goOnline()
        await until("sent and shown back", timeout: 8) { c.state.isConnected && c.waitingCount == 0 }
        XCTAssertTrue(server.current!.serialized().contains("Written on the train"))
    }
}
