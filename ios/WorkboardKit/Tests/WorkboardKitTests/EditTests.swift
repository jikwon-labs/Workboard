import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class EditTests: XCTestCase {
    /// Fields src/model.rs requires (no serde default); a copy missing one
    /// would make the Mac app drop the whole merged workspace.
    let taskRequired = ["id", "list_id", "parent", "name", "description", "status", "priority", "assignees", "start", "due", "tags", "checklist", "activity", "time_entries", "timer_started", "estimate_min", "created", "updated", "order"]
    let messageRequired = ["id", "author", "text", "ts"]

    func testNewTaskHasEveryRequiredField() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "  New one ", assignees: [1], me: 1, id: 9000, ts: 50)
        let t = d["tasks"]!.array!.last!
        for k in taskRequired { XCTAssertNotNil(t[k], k) }
        XCTAssertEqual(t["name"], "New one")
        XCTAssertEqual(t["status"], "to do")
        XCTAssertEqual(t["order"], 207)
    }

    func testMessagesAndRepliesHaveRequiredFields() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "hello", me: 1, id: 9001, ts: 60))
        XCTAssertTrue(Edits.sendReply(&d, channel: 301, to: 313, text: "on it", me: 1, id: 9002, ts: 61))
        let c = d["channels"]![0]!
        let m = c["messages"]!.array!.last!
        for k in messageRequired { XCTAssertNotNil(m[k], k) }
        let reply = c["messages"]!.array!.first { $0["id"] == 313 }!["replies"]!.array!.last!
        for k in messageRequired { XCTAssertNotNil(reply[k], k) }
        XCTAssertFalse(Edits.sendMessage(&d, channel: 301, text: "   ", me: 1, id: 9003, ts: 62))
    }

    func testStatusChangeLogsAndSetsDoneDates() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setStatus(&d, task: 201, to: "complete", me: 1, ts: 70))
        var t = d["tasks"]!.array!.first { $0["id"] == 201 }!
        XCTAssertEqual(t["done_at"], 70)
        XCTAssertEqual(t["closed_at"], 70)
        XCTAssertEqual(t["activity"]!.array!.last!["text"], "changed status from IN PROGRESS to COMPLETE")
        XCTAssertTrue(Edits.setDone(&d, task: 201, done: false, me: 1, ts: 80))
        t = d["tasks"]!.array!.first { $0["id"] == 201 }!
        XCTAssertEqual(t["status"], "to do")
        XCTAssertEqual(t["done_at"], .null)
    }

    func testUnknownFieldsSurviveEditsStampsAndMerges() {
        var phone = SampleWorkspace.make()
        var meta = SyncMeta(of: phone)
        Sync.stamp(phone, &meta, now: 10)
        phone.set("extra.sync", meta.json)
        var server = phone

        Edits.setStatus(&phone, task: 201, to: "review", me: 1, ts: 20)
        Edits.sendMessage(&phone, channel: 301, text: "hi", me: 1, id: 9100, ts: 20)
        Edits.setDocBody(&phone, doc: 401, to: "changed", ts: 20)
        Sync.stamp(phone, &meta, now: 20)
        phone.set("extra.sync", meta.json)
        let pushed = Sync.outgoing(phone)

        // what workboard-server does with a push
        XCTAssertTrue(Sync.merge(&server, [pushed]))
        // and what the phone does with the update that comes back
        Sync.merge(&phone, [server])

        for copy in [pushed, server, phone] {
            XCTAssertEqual(copy["mac_only_top"], ["nested": [1, 2, 3]])
            XCTAssertEqual(copy.at("extra.mac_only_setting"), "kept")
            let t = copy["tasks"]!.array!.first { $0["id"] == 201 }!
            XCTAssertEqual(t["mac_only_task_field"], ["kept": true])
            XCTAssertEqual(t["status"], "review")
            XCTAssertEqual(copy["docs"]![0]!["body"], "changed")
        }
        XCTAssertEqual(pushed.at("extra.sync.hashes"), .object([:]), "fingerprints stay on the phone")
    }

    func testViewReadsSample() {
        let v = WorkspaceView(SampleWorkspace.make())
        XCTAssertEqual(v.myTasks(me: 1).map(\.id).sorted(), [201, 202, 204])
        XCTAssertEqual(v.channels.first { $0.id == 301 }!.unread(me: 1), 1)
        XCTAssertEqual(v.title(of: v.channels.first { $0.id == 303 }!, me: 1), "Alex Kim")
        XCTAssertEqual(v.statuses(forList: 112).map(\.name), ["to do", "in progress", "review", "complete"])
        XCTAssertEqual(v.tasks(inList: 111).count, 2)
    }

    func testCommentPriorityDueAndAssigneeAreLoggedLikeTheMac() {
        var d = SampleWorkspace.make()
        let today = Date()
        XCTAssertTrue(Edits.addComment(&d, task: 205, text: " Looks good ", me: 1, ts: 90))
        XCTAssertTrue(Edits.setPriority(&d, task: 205, to: 1, me: 1, ts: 91))
        XCTAssertTrue(Edits.setDue(&d, task: 205, to: dayString(today.addingTimeInterval(86400)), me: 1, ts: 92, today: today))
        XCTAssertTrue(Edits.toggleAssignee(&d, task: 205, member: 2, me: 1, ts: 93))
        XCTAssertTrue(Edits.toggleAssignee(&d, task: 205, member: 2, me: 1, ts: 94))
        let t = d["tasks"]!.array!.first { $0["id"] == 205 }!
        let texts = t["activity"]!.array!.suffix(5).map { $0["text"]!.string! }
        XCTAssertEqual(texts, ["Looks good", "set priority to Urgent", "changed due date to Tomorrow", "assigned to Alex Kim", "removed assignee Alex Kim"])
        XCTAssertEqual(t["activity"]!.array!.suffix(5).first!["comment"], true)
        XCTAssertEqual(t["priority"], 1)
        XCTAssertEqual(t["assignees"], [])
    }

    func testMacDateLabels() {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        let today = f.date(from: "2026-11-14")!
        XCTAssertEqual(Edits.macDate("2026-11-14", today: today), "Today")
        XCTAssertEqual(Edits.macDate("2026-11-13", today: today), "Yesterday")
        XCTAssertEqual(Edits.macDate("2026-11-17", today: today), "Tue")
        XCTAssertEqual(Edits.macDate("2026-12-25", today: today), "Dec 25")
        XCTAssertEqual(Edits.macDate("2027-01-02", today: today), "1/2/27")
    }

    func testReactionsToggleOnMessagesAndReplies() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.toggleReaction(&d, channel: 301, message: 313, emoji: "👍", me: 1))
        XCTAssertTrue(Edits.toggleReaction(&d, channel: 301, message: 313, emoji: "👍", me: 2))
        XCTAssertTrue(Edits.toggleReaction(&d, channel: 301, message: 312, emoji: "🎉", me: 1)) // a reply
        var v = WorkspaceView(d)
        let m = v.channels[0].messages.first { $0.id == 313 }!
        XCTAssertEqual(m.reactions, [ReactionView(emoji: "👍", users: [1, 2])])
        XCTAssertEqual(v.channels[0].messages.first { $0.id == 311 }!.replies[0].reactions.first?.emoji, "🎉")
        Edits.toggleReaction(&d, channel: 301, message: 313, emoji: "👍", me: 1)
        Edits.toggleReaction(&d, channel: 301, message: 313, emoji: "👍", me: 2)
        v = WorkspaceView(d)
        XCTAssertTrue(v.channels[0].messages.first { $0.id == 313 }!.reactions.isEmpty, "the last one off removes the reaction")
    }

    func testNewDocHasRequiredFieldsAndComesFirst() {
        var d = SampleWorkspace.make()
        Edits.addDoc(&d, title: "", space: 100, me: 1, id: 9200, ts: 95)
        let doc = d["docs"]![0]!
        for k in ["id", "title", "body", "created", "updated"] { XCTAssertNotNil(doc[k], k) }
        XCTAssertEqual(doc["title"], "Untitled")
        XCTAssertEqual(doc["owner"], 1)
    }
}
