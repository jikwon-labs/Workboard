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
}
