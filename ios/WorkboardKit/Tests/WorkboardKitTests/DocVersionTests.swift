import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// A doc's Version history, as src/model/doc_versions.rs and its tests.
final class DocVersionTests: FakeServerTestCase {
    func docWs(_ body: String) -> JSON {
        ["workspace": "W", "spaces": [], "tasks": [], "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": [],
         "docs": [["id": 5, "owner": 1, "title": "Plan", "body": .string(body), "created": 1, "updated": 100]], "extra": ["sync": SyncMeta().json]]
    }

    func ids() -> () -> UInt64 {
        var n: UInt64 = 1000
        return { n += 1; return n }
    }

    func versions(_ d: JSON) -> [DocVersionView] {
        DocView(d["docs"]![0]!).versions
    }

    func testSavesCloseTogetherMakeOneVersionAndLaterOnesAnother() {
        var d = docWs("a")
        let id = ids()
        XCTAssertTrue(DocVersions.save(&d, doc: 5, title: "Plan", body: "ab", by: 1, now: 1000, newId: id))
        XCTAssertTrue(DocVersions.save(&d, doc: 5, title: "Plan", body: "abc", by: 1, now: 1000 + DocVersions.gap - 1, newId: id))
        XCTAssertFalse(DocVersions.save(&d, doc: 5, title: "Plan", body: "abc", by: 1, now: 5000, newId: id), "nothing new")
        XCTAssertTrue(DocVersions.save(&d, doc: 5, title: "Plan", body: "abcd", by: 1, now: 1000 + 2 * DocVersions.gap, newId: id))
        XCTAssertTrue(DocVersions.save(&d, doc: 5, title: "Plan", body: "abcde", by: 2, now: 1000 + 2 * DocVersions.gap + 1, newId: id))
        XCTAssertEqual(versions(d).map { "\($0.by):\($0.body)" }, ["2:abcde", "1:abcd", "1:abc", "0:a"])
        XCTAssertEqual(d["docs"]![0]!["body"], "abcde")
        XCTAssertEqual(d["docs"]![0]!["updated"], .int(1000 + 2 * DocVersions.gap + 1))
    }

    func testAnEmptyDocNeedsNoStartVersion() {
        var d = docWs("")
        DocVersions.save(&d, doc: 5, title: "Plan", body: "x", by: 1, now: 200, newId: ids())
        XCTAssertEqual(versions(d).map(\.body), ["x"])
    }

    func testRestoreBringsTextBackAsANewVersionThatCanBeUndone() {
        var d = docWs("one")
        let id = ids()
        DocVersions.save(&d, doc: 5, title: "Plan", body: "two", by: 1, now: 1000, newId: id)
        let first = versions(d)[1]
        XCTAssertEqual(first.body, "one")
        XCTAssertTrue(DocVersions.restore(&d, doc: 5, version: first.id, by: 2, now: 2000, newId: id))
        XCTAssertEqual(d["docs"]![0]!["body"], "one")
        let newest = versions(d)[0]
        XCTAssertEqual(newest.by, 2)
        XCTAssertEqual(newest.restored, 1000)
        XCTAssertFalse(DocVersions.restore(&d, doc: 5, version: first.id, by: 2, now: 2001, newId: id), "already reads that")
        let second = versions(d)[1]
        XCTAssertTrue(DocVersions.restore(&d, doc: 5, version: second.id, by: 2, now: 2100, newId: id))
        XCTAssertEqual(d["docs"]![0]!["body"], "two")
        XCTAssertEqual(versions(d).count, 4)
    }

    func testRestoreKeepsTextNoVersionHadFirst() {
        var d = docWs("one")
        let id = ids()
        DocVersions.save(&d, doc: 5, title: "Plan", body: "two", by: 1, now: 1000, newId: id)
        let first = versions(d)[1].id
        Edits.setDocBody(&d, doc: 5, to: "from an older app", ts: 1500)
        XCTAssertTrue(DocVersions.restore(&d, doc: 5, version: first, by: 1, now: 1900, newId: id))
        let v = versions(d)
        XCTAssertEqual(v[1].body, "from an older app")
        XCTAssertEqual(v[1].by, 0)
        XCTAssertEqual(v[1].at, 1900)
    }

    func testOldAndExtraVersionsGoButTheNewestStays() {
        var d = docWs("0")
        let id = ids()
        for n in 1...60 {
            DocVersions.save(&d, doc: 5, title: "Plan", body: "\(n)", by: UInt64(n % 2 + 1), now: Int64(n) * 1000, newId: id)
        }
        XCTAssertEqual(versions(d).count, DocVersions.kept)
        XCTAssertEqual(versions(d)[0].body, "60")
        let later = 60_000 + DocVersions.days * 24 * 3600 + 1
        DocVersions.save(&d, doc: 5, title: "Plan", body: "late", by: 1, now: later, newId: id)
        XCTAssertEqual(versions(d).map(\.body), ["late"])
    }

    /// The same shape as the Mac's DocVersion, so the Mac app reads it.
    func testAVersionHasTheMacsFields() {
        var d = docWs("one")
        DocVersions.save(&d, doc: 5, title: "Plan", body: "two", by: 3, now: 1000, newId: ids())
        let v = d["docs"]![0]!["versions"]!.array!.first { $0["by"] == 3 }!
        XCTAssertEqual(Set(v.object!.keys), ["id", "by", "at", "title", "body"])
        XCTAssertNil(v["restored"])
    }

    /// Two people save the same doc at once: the newer body wins, and both
    /// versions stay, on both sides.
    func testBothPeoplesVersionsStayWhenOneBodyWins() {
        var ma = SyncMeta()
        var a = docWs("start")
        Sync.stamp(a, &ma, now: 100)
        var mb = ma
        var b = a
        var na: UInt64 = 10
        DocVersions.save(&a, doc: 5, title: "Plan", body: "A text", by: 1, now: 200, newId: { na += 2; return na })
        Sync.stamp(a, &ma, now: 200)
        a.set("extra.sync", ma.json)
        var nb: UInt64 = 11
        DocVersions.save(&b, doc: 5, title: "Plan", body: "B text", by: 2, now: 300, newId: { nb += 2; return nb })
        Sync.stamp(b, &mb, now: 300)
        b.set("extra.sync", mb.json)
        var a2 = a
        XCTAssertTrue(Sync.merge(&a2, [b]))
        Sync.merge(&b, [a])
        for x in [a2, b] {
            XCTAssertEqual(x["docs"]![0]!["body"], "B text")
            // each kept the text it started from too
            XCTAssertEqual(versions(x).map(\.body).sorted(), ["A text", "B text", "start", "start"])
        }
    }

    /// Through the server: a version saved on one phone shows on the
    /// other, and a restore there comes back here.
    func testVersionsGoBetweenPhones() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = try await start(server)
        defer { server.stop() }
        let a = client(url, me: 1)
        let b = client(url, me: 2)
        await until("both connected") { a.canEdit && b.canEdit }
        let before = try XCTUnwrap(a.view.docs.first { $0.id == 401 })
        XCTAssertTrue(a.saveDoc(401, title: before.title, body: "Rewritten on the phone"))
        await until("versions at B") { b.view.docs.first { $0.id == 401 }?.versions.count == 2 }
        let bv = try XCTUnwrap(b.view.docs.first { $0.id == 401 }?.versions)
        XCTAssertEqual(bv.map(\.body), ["Rewritten on the phone", before.body])
        XCTAssertEqual(bv.map(\.by), [1, 0])
        XCTAssertTrue(b.restoreDocVersion(401, version: bv[1].id))
        await until("the restore at A") {
            a.view.docs.first { $0.id == 401 }.map { $0.body == before.body && $0.versions.first?.restored != nil } ?? false
        }
        XCTAssertEqual(a.view.docs.first { $0.id == 401 }?.versions.map(\.id), b.view.docs.first { $0.id == 401 }?.versions.map(\.id))
    }
}
