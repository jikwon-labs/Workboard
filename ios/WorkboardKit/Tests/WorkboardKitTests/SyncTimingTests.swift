import XCTest
@testable import WorkboardKit

/// How long stamping, merging and pushing take on a big workspace. Only
/// with WB_PERF=1 (it prints; nothing is checked).
@MainActor
final class SyncTimingTests: XCTestCase {
    func testTimings() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["WB_PERF"] == "1", "WB_PERF=1 to time")
        var w = bigWorkspace(tasks: 2_000, channels: 20, messages: 1_000)
        print("size", w.serialized().utf8.count / 1024, "KB")
        print("fingerprints", timed { _ = Sync.fingerprints(w) })
        print("partHashes", timed { _ = Sync.partHashes(w) })
        print("meta parse", timed { _ = SyncMeta(of: w) })
        let meta = SyncMeta(of: w)
        print("meta json", timed { _ = meta.json })
        // one edit: a new message in channel 0
        var edited = w
        var chs = edited["channels"]!.array!
        var msgs = chs[0]["messages"]!.array!
        msgs.append(["id": 99_999_999, "author": 1, "text": "new", "ts": 1_800_000_000])
        chs[0]["messages"] = .array(msgs)
        edited["channels"] = .array(chs)
        var m2 = meta
        print("stamp", timed { Sync.stamp(edited, &m2, now: 1_800_000_000_000) })
        edited.set("extra.sync", m2.json)
        // the server's delta with that message, merged into the old copy
        let summary = SyncSummary(of: w)
        var part: JSON?
        print("delta", timed { part = Sync.delta(since: summary, Sync.outgoing(edited)) })
        print("merge delta", timed { Sync.merge(&w, [part!]) })
        print("view", timed { _ = WorkspaceView(w) })
    }

    func testClientTimings() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["WB_PERF"] == "1", "WB_PERF=1 to time")
        let w = bigWorkspace(tasks: 2_000, channels: 20, messages: 1_000)
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("wbperf-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let s = Storage(folder: dir)
        s.save(snapshot: w)
        var c: SyncClient!
        print("open", timed { c = SyncClient(storage: s, tokens: MemoryTokenStore()) })
        c.setMe(1)
        let ch = c.view.channels[0].id
        print("first edit", timed { c.edit { d, me in Edits.sendMessage(&d, channel: ch, text: "warm", me: me, id: 90_000_001, ts: 1_800_000_000) } })
        var times: [Double] = []
        for i in 0..<10 {
            times.append(timed { c.edit { d, me in Edits.sendMessage(&d, channel: ch, text: "msg \(i)", me: me, id: 90_000_100 + UInt64(i), ts: 1_800_000_001 + Int64(i)) } })
        }
        print("edit (median of 10)", times.sorted()[5])
        // a teammate's message comes in as a delta
        var theirs = Sync.joined(c.data!, c.syncMeta)
        var chs = theirs["channels"]!.array!
        var msgs = chs[1]["messages"]!.array!
        msgs.append(["id": 91_000_000, "author": 2, "text": "from a teammate", "ts": 1_800_000_100])
        chs[1]["messages"] = .array(msgs)
        theirs["channels"] = .array(chs)
        var tm = SyncMeta(of: theirs)
        Sync.stamp(theirs, &tm, now: 1_800_000_100_000)
        theirs.set("extra.sync", tm.json)
        let part = Sync.delta(since: SyncSummary(of: Sync.outgoing(c.data!)), Sync.outgoing(theirs))!
        do {
            var local = c.data!
            var meta = c.syncMeta
            let cache = c.prints
            print(" merge only", timed { Sync.merge(&local, meta: &meta, [part], cache: cache) })
            var l2 = c.data!
            var m2 = c.syncMeta
            print(" merge again", timed { Sync.merge(&l2, meta: &m2, [part], cache: cache) })
            var l3 = c.data!
            print(" fills", timed { Sync.fillActivityIds(&l3); Sync.fillBoardIds(&l3); Sync.fillGoalIds(&l3) })
            print(" cache update", timed { cache.update(c.data!) })
            print(" view", timed { _ = WorkspaceView(c.data!) })
            print(" mine", timed { c.sendMineIfChanged() })
        }
        print("delta in", timed { c.receiveForTests(.delta(data: part, rev: 1, base: 0, hidden: [])) })
        print("flush", timed { c.flushSnapshot() })
    }
}
