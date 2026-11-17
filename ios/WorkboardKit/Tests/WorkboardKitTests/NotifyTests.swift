import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The Mac's notify_candidates rules, on the phone.
final class NotifyTests: XCTestCase {
    func keys(_ d: JSON, _ p: NotifyPrefs = NotifyPrefs(), manager: Bool = false) -> [String] {
        Notify.candidates(d, prefs: p, me: 1, manager: manager).map(\.key)
    }

    func testDirectMessagesAlwaysButChannelsOnlyWhenMentionedByDefault() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 303, text: "dm", me: 2, id: 900, ts: 10)
        Edits.sendMessage(&d, channel: 301, text: "plain", me: 2, id: 901, ts: 11)
        Edits.sendMessage(&d, channel: 301, text: "@Jamie Park look", me: 2, id: 902, ts: 12)
        Edits.sendMessage(&d, channel: 301, text: "mine", me: 1, id: 903, ts: 13)
        let k = keys(d)
        XCTAssertTrue(k.contains("chat:303:900"))
        XCTAssertFalse(k.contains("chat:301:901"), "channels default to mentions only")
        XCTAssertTrue(k.contains("chat:301:902"))
        XCTAssertFalse(k.contains("chat:301:903"))
        var all = NotifyPrefs()
        all.setChannelMode(301, "all")
        XCTAssertTrue(keys(d, all).contains("chat:301:901"))
        var off = NotifyPrefs()
        off.setChannelMode(301, "nothing")
        XCTAssertFalse(keys(d, off).contains("chat:301:902"), "a channel set to nothing says nothing, mentions too")
        var noDM = NotifyPrefs()
        noDM.direct = false
        XCTAssertFalse(keys(d, noDM).contains("chat:303:900"))
    }

    func testTaskNewsFollowTheChoices() {
        var d = SampleWorkspace.make()
        Edits.toggleAssignee(&d, task: 205, member: 1, me: 2, ts: 10)       // assigned to me
        Edits.addComment(&d, task: 201, text: "nice", me: 2, ts: 11)        // on my task
        Edits.setStatus(&d, task: 201, to: "review", me: 2, ts: 12)         // a change on my task
        let notes = Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false)
        XCTAssertEqual(notes.filter { $0.kind == .assigned }.count, 1)
        XCTAssertEqual(notes.first { $0.kind == .comment }?.title, "Alex Kim commented on Write the launch post")
        XCTAssertEqual(notes.first { $0.kind == .taskChange }?.body, "Alex Kim changed status from IN PROGRESS to REVIEW")
        var p = NotifyPrefs()
        p.assigned = false; p.comments = false; p.taskChanges = false
        XCTAssertTrue(Notify.candidates(d, prefs: p, me: 1, manager: false).filter { [.assigned, .comment, .taskChange].contains($0.kind) }.isEmpty)
    }

    func testHuddlesReactionsFormsAndJoins() {
        var d = SampleWorkspace.make()
        Edits.postHuddle(&d, channel: 303, me: 2, id: 900, ts: 10)
        Edits.toggleReaction(&d, channel: 302, message: 321, emoji: "👍", me: 2)
        Edits.addForm(&d, list: 120, me: 1, id: 830)
        _ = Edits.submitForm(&d, form: 830, answers: [0: "Laptop"], me: 2, id: 9000, ts: 11)
        let notes = Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: true)
        XCTAssertEqual(notes.first { $0.kind == .huddle }?.title, "Alex Kim is calling you")
        XCTAssertNil(notes.first { $0.kind == .reaction }, "reactions are off by default")
        XCTAssertNotNil(Notify.candidates(d, prefs: .everything, me: 1, manager: true).first { $0.kind == .reaction })
        XCTAssertEqual(notes.first { $0.kind == .form }?.title, "New answer to New Form")
        XCTAssertEqual(notes.filter { $0.kind == .member }.count, 2)
        XCTAssertTrue(Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false).filter { $0.kind == .member }.isEmpty)
    }

    func testOnlyFreshNewsOnceAndHuddlesShortly() {
        let now: Int64 = 100_000
        let notes = [
            Note(key: "a", kind: .direct, title: "", body: "", target: .people, ts: now - 10),
            Note(key: "b", kind: .direct, title: "", body: "", target: .people, ts: now - Notify.freshSecs - 1),
            Note(key: "h", kind: .huddle, title: "", body: "", target: .people, ts: now - 600),
            Note(key: "r", kind: .reaction, title: "", body: "", target: .people, ts: nil),
        ]
        var seen: Set<String> = []
        XCTAssertEqual(Notify.fresh(notes, all: ["a", "b", "h", "r", "x"], seen: &seen, now: now).map(\.key), ["a", "r"])
        XCTAssertTrue(Notify.fresh(notes, all: [], seen: &seen, now: now).isEmpty, "told once")
        XCTAssertTrue(seen.contains("x"), "what exists with every kind on isn't news later")
    }

    func testPrefsSaveAndReadBackAndOldFilesKeepDefaults() throws {
        var p = NotifyPrefs()
        p.setChannelMode(301, "all")
        p.mutedUntil = 5
        let back = try JSONDecoder().decode(NotifyPrefs.self, from: JSONEncoder().encode(p))
        XCTAssertEqual(back, p)
        let old = try JSONDecoder().decode(NotifyPrefs.self, from: Data(#"{"direct": false}"#.utf8))
        XCTAssertFalse(old.direct)
        XCTAssertTrue(old.mentions)
        XCTAssertEqual(old.channelDefault, "mentions")
        p.setChannelMode(301, "mentions")
        XCTAssertNil(p.channelModes[301], "the default isn't kept as an exception")
    }

    func testDueSummaryAndBadge() {
        var d = SampleWorkspace.make()
        let v = WorkspaceView(d)
        let today = dayString()
        // 201 due today, 202 two days late (both mine); 204 later
        XCTAssertEqual(Notify.dueSummary(v, me: 1, day: today), "1 due today, 1 overdue")
        Edits.addComment(&d, task: 201, text: "x", me: 2, ts: 5)
        XCTAssertEqual(Notify.badge(d, view: WorkspaceView(d), me: 1), 2, "one Inbox row and one unread DM")
    }
}
