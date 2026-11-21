import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class InboxTests: XCTestCase {
    func testRowsFollowTheMacRules() {
        var d = SampleWorkspace.make()
        // Alex comments on my task, assigns me, and mentions me in chat and a reply
        Edits.addComment(&d, task: 201, text: "Looks good", me: 2, ts: 100)
        Edits.toggleAssignee(&d, task: 205, member: 1, me: 2, ts: 101)
        Edits.sendMessage(&d, channel: 302, text: "@Jamie Park see this", me: 2, id: 900, ts: 102)
        Edits.sendReply(&d, channel: 302, to: 321, text: "agreed", me: 2, id: 901, ts: 103) // my message's thread
        Edits.addTask(&d, list: 112, name: "Alex's", assignees: [2], me: 2, id: 950, ts: 99)
        Edits.addComment(&d, task: 950, text: "not mine", me: 2, ts: 104)                     // a task I'm not on and didn't make
        Edits.addComment(&d, task: 201, text: "mine", me: 1, ts: 105)                         // my own
        let items = Inbox.items(d, me: 1)
        let byKey = Dictionary(items.map { ($0.key, $0) }, uniquingKeysWith: { a, _ in a })
        XCTAssertEqual(byKey["201:100:2"]?.tab, .primary)
        XCTAssertEqual(byKey["205:101:2"]?.assigned, true)
        XCTAssertEqual(byKey["chat:302:900"]?.mention, true)
        XCTAssertEqual(byKey["chat:302:901"]?.reply, true)
        XCTAssertEqual(byKey["950:104:2"]?.tab, .other)
        XCTAssertNil(byKey["201:105:1"])
        XCTAssertEqual(items.first?.key, "950:104:2", "newest first")
        XCTAssertEqual(Inbox.line(byKey["chat:302:901"]!, author: "Alex Kim"), "Alex Kim replied: agreed")
        XCTAssertEqual(Inbox.line(byKey["205:101:2"]!, author: "Alex Kim"), "Alex Kim assigned to Jamie Park")
    }

    /// What goes to Primary follows my notification settings (kept in step
    /// with the Mac), as inbox_items on the Mac does: comments, mentions and
    /// assignments each go to Other when turned off; thread replies are
    /// "notified only" and stay in Primary.
    func testPrimaryFollowsMyNotificationSettingsLikeTheMac() {
        var d = SampleWorkspace.make()
        Edits.addComment(&d, task: 201, text: "Looks good", me: 2, ts: 100)
        Edits.toggleAssignee(&d, task: 205, member: 1, me: 2, ts: 101)
        Edits.sendMessage(&d, channel: 302, text: "@Jamie Park see this", me: 2, id: 900, ts: 102)
        Edits.sendReply(&d, channel: 302, to: 321, text: "agreed", me: 2, id: 901, ts: 103)
        func tabs() -> [String: InboxItem.Tab] {
            Dictionary(Inbox.items(d, me: 1).map { ($0.key, $0.tab) }, uniquingKeysWith: { a, _ in a })
        }
        XCTAssertEqual(tabs()["201:100:2"], .primary)
        XCTAssertEqual(tabs()["chat:302:900"], .primary)
        d.set("extra.notify", .object(["comments": .bool(false), "mentions": .bool(false), "assigned": .bool(false)]))
        let t = tabs()
        XCTAssertEqual(t["201:100:2"], .other, "comments on my tasks off")
        XCTAssertEqual(t["205:101:2"], .other, "assignments off")
        XCTAssertEqual(t["chat:302:900"], .other, "mentions off")
        XCTAssertEqual(t["chat:302:901"], .primary, "replies stay in Primary")
        // a setting the Mac hasn't sent counts as on, like its defaults
        d.set("extra.notify", .object(["direct": .bool(false)]))
        XCTAssertEqual(tabs()["201:100:2"], .primary)
    }

    func testLaterAndClearedAreKeptAndUndone() {
        var d = SampleWorkspace.make()
        Edits.addComment(&d, task: 201, text: "a", me: 2, ts: 100)
        Edits.addComment(&d, task: 201, text: "b", me: 2, ts: 101)
        Inbox.move(&d, keys: ["201:100:2"], to: .later)
        Inbox.move(&d, keys: ["201:101:2"], to: .cleared)
        var tabs = Dictionary(Inbox.items(d, me: 1).map { ($0.key, $0.tab) }, uniquingKeysWith: { a, _ in a })
        XCTAssertEqual(tabs["201:100:2"], .later)
        XCTAssertEqual(tabs["201:101:2"], .cleared)
        Inbox.move(&d, keys: ["201:100:2", "201:101:2"], to: nil)
        tabs = Dictionary(Inbox.items(d, me: 1).map { ($0.key, $0.tab) }, uniquingKeysWith: { a, _ in a })
        XCTAssertEqual(tabs["201:100:2"], .primary)
        XCTAssertEqual(tabs["201:101:2"], .primary)
        XCTAssertFalse(Sync.shared.contains("extra.inbox_cleared"), "never shared")
    }
}
