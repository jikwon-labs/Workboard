import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// snapshot.json written a moment after changes stop, off the main thread,
/// and the Inbox rows worked out once per change.
@MainActor
final class SnapshotTests: FakeServerTestCase {
    var clients: [SyncClient] = []

    override func tearDown() async throws {
        // nothing still on its way recreates a folder after it is removed
        for c in clients { c.flushSnapshot() }
        try await super.tearDown()
    }

    /// A client with the sample workspace saved on this device (no server).
    func client(_ s: Storage, delay: TimeInterval, maxDelay: TimeInterval = 5) -> SyncClient {
        s.save(snapshot: SampleWorkspace.make())
        let c = SyncClient(storage: s, tokens: MemoryTokenStore())
        c.saveDelay = delay
        c.saveMaxDelay = maxDelay
        c.setMe(1)
        clients.append(c)
        return c
    }

    func savedName(_ s: Storage, task: UInt64 = 201) -> String? {
        s.loadSnapshot().flatMap { WorkspaceView($0).tasks.first { $0.id == task }?.name }
    }

    func testTheCopyIsSavedOnceChangesStop() async {
        let s = storage()
        let c = client(s, delay: 0.3)
        let original = savedName(s)
        XCTAssertNotNil(original)
        for i in 1...3 { XCTAssertTrue(c.edit { d, _ in Edits.rename(&d, task: 201, to: "Renamed \(i)", ts: nowTs()) }) }
        c.editLocal { d in Edits.setNotepad(&d, to: "jot") }
        XCTAssertEqual(savedName(s), original, "not written on every change")
        await until("saved") { self.savedName(s) == "Renamed 3" }
        XCTAssertEqual(s.loadSnapshot()?.at("extra.notepad")?.string, "jot", "with the newest copy")
    }

    func testFlushWritesTheNewestCopyRightAway() {
        let s = storage()
        let c = client(s, delay: 60)
        XCTAssertTrue(c.edit { d, _ in Edits.rename(&d, task: 201, to: "Flushed", ts: nowTs()) })
        c.flushSnapshot()
        XCTAssertEqual(savedName(s), "Flushed")
        c.editLocal { d in Edits.setNotepad(&d, to: "later") }
        c.flushSnapshot()
        XCTAssertEqual(s.loadSnapshot()?.at("extra.notepad")?.string, "later")
        // a relaunch sees it
        XCTAssertEqual(SyncClient(storage: s, tokens: MemoryTokenStore()).view.tasks.first { $0.id == 201 }?.name, "Flushed")
    }

    func testChangesThatNeverStopStillSaveByTheLongestWait() async throws {
        let s = storage()
        let c = client(s, delay: 0.3, maxDelay: 0.8)
        let original = savedName(s)
        let end = Date().addingTimeInterval(2)
        var saved = false
        var i = 0
        // a change every 0.1 s: the short wait never runs out
        while Date() < end {
            i += 1
            c.edit { d, _ in Edits.rename(&d, task: 201, to: "Typing \(i)", ts: nowTs()) }
            try await Task.sleep(nanoseconds: 100_000_000)
            if savedName(s) != original { saved = true; break }
        }
        XCTAssertTrue(saved, "saved while the changes kept coming")
    }

    func testWritesLandInOrderAndARemovalIsLast() {
        let s = storage()
        let w = SnapshotWriter(storage: s)
        let big: JSON = .array((0..<20_000).map { .int(Int64($0)) })
        for i in 0..<40 {
            // big and small copies mixed: a slower write never lands after a later one
            w.write(i % 2 == 0 ? ["n": .int(Int64(i)), "big": big] : ["n": .int(Int64(i))])
        }
        w.wait()
        XCTAssertEqual(s.loadSnapshot()?["n"]?.int, 39)
        w.write(["n": 40, "big": big])
        w.remove()
        XCTAssertFalse(FileManager.default.fileExists(atPath: s.snapshotURL.path), "a write on its way doesn't bring it back")
    }

    func testAForgottenCopyStaysGone() async throws {
        let s = storage()
        let c = client(s, delay: 0.2)
        XCTAssertTrue(c.edit { d, _ in Edits.rename(&d, task: 201, to: "Pending", ts: nowTs()) })
        c.workspaceDeleted("gone")
        XCTAssertFalse(FileManager.default.fileExists(atPath: s.snapshotURL.path))
        try await Task.sleep(nanoseconds: 500_000_000)
        XCTAssertFalse(FileManager.default.fileExists(atPath: s.snapshotURL.path), "the pending save was called off")
        c.flushSnapshot()
        XCTAssertFalse(FileManager.default.fileExists(atPath: s.snapshotURL.path), "nothing left to write")
    }

    func testInboxRowsAreWorkedOutOncePerChange() throws {
        let s = storage()
        let c = client(s, delay: 60)
        let first = c.inboxItems(me: 1)
        let rev = c.dataRevision
        XCTAssertEqual(c.inboxItems(me: 1), first)
        XCTAssertEqual(c.dataRevision, rev, "reading doesn't count as a change")
        XCTAssertEqual(first, Inbox.items(try XCTUnwrap(c.data), me: 1))

        // a teammate's comment on my task shows at once
        XCTAssertTrue(c.edit { d, _ in Edits.addComment(&d, task: 201, text: "Looks good", me: 2, ts: 100) })
        XCTAssertNotEqual(c.dataRevision, rev)
        let after = c.inboxItems(me: 1)
        XCTAssertEqual(after.filter { $0.tab == .primary }.count, first.filter { $0.tab == .primary }.count + 1)
        XCTAssertEqual(after.first { $0.key == "201:100:2" }?.tab, .primary)

        // cleared: moves right away too
        c.editLocal { d in Inbox.move(&d, keys: ["201:100:2"], to: .cleared) }
        XCTAssertEqual(c.inboxItems(me: 1).first { $0.key == "201:100:2" }?.tab, .cleared)
        // someone else's rows aren't mine
        XCTAssertEqual(c.inboxItems(me: 2), Inbox.items(try XCTUnwrap(c.data), me: 2))
    }

    func testNotifyGivesTheSameWithTheInboxPassedIn() {
        var d = SampleWorkspace.make()
        Edits.addComment(&d, task: 201, text: "Looks good", me: 2, ts: 100)
        Edits.sendMessage(&d, channel: 302, text: "@Jamie Park see this", me: 2, id: 900, ts: 102)
        let inbox = Inbox.items(d, me: 1)
        for p in [NotifyPrefs(), .everything] {
            XCTAssertEqual(Notify.candidates(d, prefs: p, me: 1, manager: true, inbox: inbox).map(\.key),
                           Notify.candidates(d, prefs: p, me: 1, manager: true).map(\.key))
        }
        let v = WorkspaceView(d)
        XCTAssertEqual(Notify.badge(d, view: v, me: 1, inbox: inbox), Notify.badge(d, view: v, me: 1))
        XCTAssertGreaterThan(Notify.badge(d, view: v, me: 1, inbox: inbox), 0)
    }
}
