import XCTest
@testable import WorkboardKit

/// The quick ways of fingerprinting, stamping and merging give exactly what
/// the plain ways give.
final class QuickSyncTests: XCTestCase {
    /// Fingerprints as they were taken before: the written text, hashed.
    func oldOwnHash(_ obj: [String: JSON]) -> UInt64 {
        var own: [String: JSON] = [:]
        for (k, v) in obj {
            if Sync.localKeys.contains(k) { continue }
            if let a = v.array, a.isEmpty { continue }
            if Sync.isIdArray(v) {
                let loose = Sync.looseEntries(v)
                if !loose.isEmpty { own[k] = .array(loose) }
                continue
            }
            own[k] = v
        }
        return Sync.fnv(JSON.object(own).serialized().utf8)
    }

    func testFingerprintsAreTheWrittenTextsHash() {
        let values: [JSON] = [
            .null, true, false, 0, -1, .int(Int64.min), .int(Int64.max), .uint(UInt64.max), 1.5, -0.25, 1e300, .double(.infinity),
            "", "a", "quote \" back \\ /", "\n\r\t\u{08}\u{0C}\u{00}\u{01}\u{1F}\u{7F}", "한글", "🎉👍🏽", [], [1, "x", [nil]], [:],
            ["b": 1, "a": ["z": [1, 2], "A": "x"], "é": true, "e": false, "": nil],
        ]
        for v in values {
            XCTAssertEqual(v.fingerprint, Sync.fnv(v.serialized().utf8), "\(v.serialized())")
        }
        for t in oddTexts {
            let v: JSON = ["text": .string(t)]
            XCTAssertEqual(v.fingerprint, Sync.fnv(v.serialized().utf8), t)
        }
    }

    func testItemFingerprintsAreUnchanged() {
        let w = bigWorkspace(tasks: 60, channels: 3, messages: 40)
        var n = 0
        Sync.visit(w) { _, _, obj in
            XCTAssertEqual(Sync.ownHash(obj), self.oldOwnHash(obj))
            n += 1
        }
        XCTAssertGreaterThan(n, 200)
    }

    // MARK: merging

    /// A copy changed the way people change things: a new message, a reply,
    /// a reaction, an edited and a deleted task, a list moved to another
    /// space, a new doc. Stamped at `now`.
    func changed(_ w: JSON, seed: UInt64, now: Int64) -> JSON {
        var r = Rand(s: seed)
        var d = w
        var chs = d["channels"]!.array!
        let c = r.below(chs.count)
        var msgs = chs[c]["messages"]!.array!
        msgs.append(["id": .int(Int64(50_000_000 + seed)), "author": 2, "text": "new from elsewhere", "ts": .int(now / 1000)])
        let m = r.below(msgs.count - 1)
        var replies = msgs[m]["replies"]?.array ?? []
        replies.append(["id": .int(Int64(60_000_000 + seed)), "author": 3, "text": "a reply", "ts": .int(now / 1000)])
        msgs[m]["replies"] = .array(replies)
        let m2 = r.below(msgs.count - 1)
        msgs[m2]["reactions"] = [["emoji": "🎉", "users": [3]]]
        chs[c]["messages"] = .array(msgs)
        d["channels"] = .array(chs)
        var tasks = d["tasks"]!.array!
        let t = r.below(tasks.count)
        tasks[t]["name"] = .string("renamed \(seed)")
        tasks.remove(at: (t + 1) % tasks.count)
        d["tasks"] = .array(tasks)
        var spaces = d["spaces"]!.array!
        // from the space with the most lists to the next one
        let s = spaces.indices.max { (spaces[$0]["lists"]?.array?.count ?? 0) < (spaces[$1]["lists"]?.array?.count ?? 0) }!
        var from = spaces[s]["lists"]!.array!
        let moved = from.removeLast()
        spaces[s]["lists"] = .array(from)
        let t2 = (s + 1) % spaces.count
        var to = spaces[t2]["lists"]!.array!
        to.append(moved)
        spaces[t2]["lists"] = .array(to)
        d["spaces"] = .array(spaces)
        var docs = d["docs"]!.array!
        docs.append(["id": .int(Int64(70_000_000 + seed)), "title": "new doc", "body": "", "parent": nil])
        d["docs"] = .array(docs)
        var meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: now)
        d.set("extra.sync", meta.json)
        return d
    }

    /// The copy as a full merge writes it (with nothing else to merge).
    func normal(_ v: JSON) -> JSON {
        let was = Sync.quickMerge
        Sync.quickMerge = false
        defer { Sync.quickMerge = was }
        var v = v
        Sync.merge(&v, [])
        return v
    }

    func merged(_ local: JSON, _ others: [JSON], quick: Bool) -> JSON {
        let was = Sync.quickMerge
        Sync.quickMerge = quick
        defer { Sync.quickMerge = was }
        var l = local
        Sync.merge(&l, others)
        return l
    }

    func assertSameMerge(_ local: JSON, _ others: [JSON], file: StaticString = #filePath, line: UInt = #line) {
        let quick = normal(merged(local, others, quick: true))
        let full = normal(merged(local, others, quick: false))
        XCTAssertEqual(Sync.stripSync(quick).serialized(), Sync.stripSync(full).serialized(), "the copies", file: file, line: line)
        XCTAssertEqual(SyncMeta(of: quick), SyncMeta(of: full), "the bookkeeping", file: file, line: line)
    }

    func testAQuickMergeOfADeltaIsTheFullMerge() {
        let base = bigWorkspace(tasks: 80, channels: 4, messages: 60)
        for seed in UInt64(1)...6 {
            let theirs = changed(base, seed: seed, now: 1_800_000_000_000 + Int64(seed))
            let part = Sync.delta(since: SyncSummary(of: base), Sync.outgoing(theirs))!
            // and this device changed something of its own meanwhile
            let mine = changed(base, seed: seed + 100, now: 1_800_000_000_500)
            assertSameMerge(base, [part])
            assertSameMerge(mine, [part])
            assertSameMerge(mine, [Sync.outgoing(theirs)])
            assertSameMerge(mine, [part, Sync.outgoing(changed(theirs, seed: seed + 200, now: 1_800_000_001_000))])
            // a copy a merge wrote comes out byte for byte the same
            let tidy = normal(mine)
            XCTAssertEqual(merged(tidy, [part], quick: true).serialized(), merged(tidy, [part], quick: false).serialized())
        }
    }

    func testAQuickMergeDropsWhatAnotherCopyDeleted() {
        let base = bigWorkspace(tasks: 30, channels: 2, messages: 20)
        var theirs = base
        // a channel and a message deleted elsewhere
        var chs = theirs["channels"]!.array!
        chs.removeFirst()
        var msgs = chs[0]["messages"]!.array!
        msgs.remove(at: 3)
        chs[0]["messages"] = .array(msgs)
        theirs["channels"] = .array(chs)
        var meta = SyncMeta(of: theirs)
        Sync.stamp(theirs, &meta, now: 1_800_000_000_000)
        theirs.set("extra.sync", meta.json)
        let part = Sync.delta(since: SyncSummary(of: base), Sync.outgoing(theirs))!
        assertSameMerge(base, [part])
        let got = merged(base, [part], quick: true)
        XCTAssertEqual(got["channels"]?.array?.count, 1)
        XCTAssertEqual(got["channels"]?[0]?["messages"]?.array?.count, 19)
    }

    // MARK: fingerprints kept between changes

    func assertCacheIsFull(_ cache: FingerprintCache, _ d: JSON, file: StaticString = #filePath, line: UInt = #line) {
        cache.update(d)
        XCTAssertEqual(cache.hashes, Sync.fingerprints(d), "fingerprints", file: file, line: line)
        XCTAssertEqual(cache.parts, Sync.partHashes(d), "field fingerprints", file: file, line: line)
        XCTAssertEqual(cache.parents, Sync.parentsOf(d), "places", file: file, line: line)
    }

    func testKeptFingerprintsFollowEveryChange() {
        let cache = FingerprintCache()
        var d = bigWorkspace(tasks: 50, channels: 3, messages: 30)
        assertCacheIsFull(cache, d)
        for seed in UInt64(1)...5 {
            // edits: new message and reply, a reaction, renamed and deleted
            // tasks, a list moved to another space, a new doc
            d = changed(d, seed: seed, now: 1_800_000_000_000 + Int64(seed))
            assertCacheIsFull(cache, d)
            // a merge builds the copy anew: equal items in new storage
            let other = changed(d, seed: seed + 50, now: 1_800_000_000_100 + Int64(seed))
            Sync.merge(&d, [Sync.delta(since: SyncSummary(of: d), Sync.outgoing(other))!], cache: cache)
            assertCacheIsFull(cache, d)
        }
        // a whole channel goes, then comes back elsewhere in the list
        var chs = d["channels"]!.array!
        let gone = chs.removeFirst()
        d["channels"] = .array(chs)
        assertCacheIsFull(cache, d)
        chs.append(gone)
        d["channels"] = .array(chs)
        assertCacheIsFull(cache, d)
        // a message moves into a thread as a reply, a list into a folder
        var msgs = chs[0]["messages"]!.array!
        let m = msgs.removeLast()
        var replies = msgs[0]["replies"]?.array ?? []
        replies.append(m)
        msgs[0]["replies"] = .array(replies)
        chs[0]["messages"] = .array(msgs)
        d["channels"] = .array(chs)
        assertCacheIsFull(cache, d)
        // everything gone, then back
        assertCacheIsFull(cache, ws([], []))
        assertCacheIsFull(cache, d)
    }

    func testStampingWithKeptFingerprintsIsTheSame() {
        let cache = FingerprintCache()
        var a = bigWorkspace(tasks: 40, channels: 2, messages: 20)
        var b = a
        var ma = SyncMeta(of: a), mb = ma
        for seed in UInt64(1)...4 {
            let next = changed(a, seed: seed, now: 1_800_000_000_000)
            a = next
            b = next
            Sync.stamp(a, &ma, now: 1_800_000_000_000 + Int64(seed), cache: cache)
            Sync.stamp(b, &mb, now: 1_800_000_000_000 + Int64(seed))
            XCTAssertEqual(ma, mb)
        }
    }
}
