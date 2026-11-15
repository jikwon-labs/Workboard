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

    func testNewTaskLogsEveryoneElsePutOnIt() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "For Alex", assignees: [1, 2], me: 1, id: 9000, ts: 50)
        let act = d["tasks"]!.array!.last!["activity"]!.array!
        XCTAssertEqual(act.map { $0["text"] }, ["created this task", "assigned to Alex Kim"])
        XCTAssertEqual(act[1]["target"], 2)
        XCTAssertNil(act[0]["target"])
    }

    func testIdsTheMacAppKeepsSurviveEditsHere() {
        var d = SampleWorkspace.make()
        d.updateItem(at: "tasks", id: 201) { t in
            var act = t["activity"]?.array ?? []
            act.append(["ts": 5, "author": 2, "text": "assigned to Jamie Park", "comment": false, "target": 1])
            act.append(["ts": 6, "author": 2, "text": "@Jamie Park look", "comment": true, "mentions": [1]])
            t["activity"] = .array(act)
        }
        XCTAssertTrue(Edits.addComment(&d, task: 201, text: "ok", me: 1, ts: 7))
        XCTAssertTrue(Edits.setStatus(&d, task: 201, to: "complete", me: 1, ts: 8))
        let act = d["tasks"]!.array!.first { $0["id"] == 201 }!["activity"]!.array!
        XCTAssertEqual(act.first { $0["ts"] == 5 }!["target"], 1)
        XCTAssertEqual(act.first { $0["ts"] == 6 }!["mentions"], [1])
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

    // MARK: message actions, channels, deletes, repeats (round 2)

    func testEditDeleteAndPinMyMessagesOnly() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 301, text: "mine", me: 1, id: 9100, ts: 80)
        XCTAssertTrue(Edits.editMessage(&d, channel: 301, message: 9100, text: "mine, fixed", me: 1, ts: 90))
        let v = WorkspaceView(d).channels.first { $0.id == 301 }!
        XCTAssertEqual(v.messages.last?.text, "mine, fixed")
        XCTAssertEqual(v.messages.last?.edited, 90)
        XCTAssertFalse(Edits.editMessage(&d, channel: 301, message: 313, text: "not mine", me: 1, ts: 91), "someone else's message")
        XCTAssertFalse(Edits.editMessage(&d, channel: 301, message: 9100, text: "  ", me: 1, ts: 92))
        // a reply of mine
        Edits.sendReply(&d, channel: 301, to: 313, text: "reply", me: 1, id: 9101, ts: 93)
        XCTAssertTrue(Edits.editMessage(&d, channel: 301, message: 9101, text: "reply 2", me: 1, ts: 94))
        XCTAssertTrue(Edits.deleteMessage(&d, channel: 301, message: 9101, me: 1))
        XCTAssertFalse(Edits.deleteMessage(&d, channel: 301, message: 313, me: 1))
        XCTAssertTrue(Edits.togglePin(&d, channel: 301, message: 313))
        XCTAssertTrue(WorkspaceView(d).channels.first { $0.id == 301 }!.messages.first { $0.id == 313 }!.pinned)
        XCTAssertTrue(Edits.deleteMessage(&d, channel: 301, message: 9100, me: 1))
        XCTAssertNil(WorkspaceView(d).channels.first { $0.id == 301 }!.messages.first { $0.id == 9100 })
    }

    func testDeletedMessageStaysDeletedAfterMerge() {
        var phone = SampleWorkspace.make()
        Edits.sendMessage(&phone, channel: 301, text: "oops", me: 1, id: 9200, ts: 100)
        var meta = SyncMeta(of: phone)
        Sync.stamp(phone, &meta, now: 1_000)
        phone.set("extra.sync", meta.json)
        let server = phone
        Edits.deleteMessage(&phone, channel: 301, message: 9200, me: 1)
        Sync.stamp(phone, &meta, now: 2_000)
        phone.set("extra.sync", meta.json)
        XCTAssertNotNil(meta.tombs[9200], "a delete leaves a tombstone for the others")
        var merged = server
        Sync.merge(&merged, [phone])
        XCTAssertNil(WorkspaceView(merged).channels.first { $0.id == 301 }!.messages.first { $0.id == 9200 })
    }

    func testCreateChannelPublicAndPrivate() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.createChannel(&d, name: " Road Map ", isPrivate: false, me: 2, id: 9300, ts: 5))
        XCTAssertTrue(Edits.createChannel(&d, name: "secret", isPrivate: true, me: 2, id: 9301, ts: 5))
        XCTAssertFalse(Edits.createChannel(&d, name: "  ", isPrivate: false, me: 2, id: 9302, ts: 5))
        let v = WorkspaceView(d)
        let pub = v.channels.first { $0.id == 9300 }!
        XCTAssertEqual(pub.name, "road-map")
        XCTAssertEqual(pub.members, [1, 2, 3])
        for k in ["id", "name", "kind", "topic", "members", "messages"] { XCTAssertNotNil(d["channels"]!.array!.last![k], k) }
        XCTAssertTrue(v.visibleChannels(me: 2).contains { $0.id == 9301 })
        XCTAssertFalse(v.visibleChannels(me: 1).contains { $0.id == 9301 }, "a private channel is only for its members")
    }

    func testMessageWithOnlyAPicture() {
        var d = SampleWorkspace.make()
        let a = FileAPI.attachment(sha: String(repeating: "a", count: 64), name: "IMG_1.jpg", isVideo: false, bytes: 1234, w: 640, h: 480, secs: 0)
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "", attachments: [a], me: 1, id: 9400, ts: 9))
        let m = WorkspaceView(d).channels.first { $0.id == 301 }!.messages.last!
        XCTAssertEqual(m.text, "")
        XCTAssertEqual(m.attachments.first?.name, "IMG_1.jpg")
        XCTAssertEqual(m.attachments.first?.isVideo, false)
        XCTAssertEqual(d["channels"]![0]!["messages"]!.array!.last!["attachments"]![0]!["kind"], "Image")
        XCTAssertFalse(Edits.sendMessage(&d, channel: 301, text: "", me: 1, id: 9401, ts: 9), "nothing to send")
    }

    func testDeleteTaskTakesSubtasksAndDeleteDocTakesSubPages() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "child", assignees: [], me: 1, id: 9500, ts: 1)
        d.updateItem(at: "tasks", id: 9500) { $0["parent"] = 201 }
        XCTAssertTrue(Edits.deleteTask(&d, task: 201))
        XCTAssertNil(WorkspaceView(d).tasks.first { $0.id == 201 || $0.id == 9500 })
        XCTAssertFalse(Edits.deleteTask(&d, task: 201))
        Edits.addDoc(&d, title: "sub", space: 100, me: 1, id: 9600, ts: 1)
        d.updateItem(at: "docs", id: 9600) { $0["parent"] = 401 }
        XCTAssertTrue(Edits.deleteDoc(&d, doc: 401))
        XCTAssertNil(WorkspaceView(d).docs.first { $0.id == 401 || $0.id == 9600 })
        XCTAssertNotNil(WorkspaceView(d).docs.first { $0.id == 402 })
    }

    func testClosingARepeatingTaskRollsItOn() {
        var d = SampleWorkspace.make()
        let today = Calendar(identifier: .gregorian).date(from: DateComponents(year: 2026, month: 1, day: 31, hour: 12))!
        d.updateItem(at: "tasks", id: 204) { t in
            t["recur"] = "monthly"
            t["due"] = "2026-01-31"
            t["start"] = "2026-01-29"
            t["checklist"] = [["text": "a", "done": true]]
        }
        XCTAssertTrue(Edits.setDone(&d, task: 204, done: true, me: 1, ts: 500, today: today))
        let t = d["tasks"]!.array!.first { $0["id"] == 204 }!
        XCTAssertEqual(t["status"], "to do", "open again")
        XCTAssertEqual(t["due"], "2026-02-28", "a month on (end of month kept, like chrono)")
        XCTAssertEqual(t["start"], "2026-02-26", "moved by as many days as the due date")
        XCTAssertEqual(t["done_at"], .null)
        XCTAssertEqual(t["closed_at"], .null)
        XCTAssertEqual(t["checklist"]?[0]?["done"], false)
        let texts = (t["activity"]?.array ?? []).compactMap { $0["text"]?.string }
        XCTAssertTrue(texts.contains("changed status from TO DO to COMPLETE"))
        XCTAssertEqual(texts.last, "completed a monthly repeat; next due Feb 28")
        // a task without a repeat just closes
        XCTAssertTrue(Edits.setDone(&d, task: 202, done: true, me: 1, ts: 500))
        XCTAssertEqual(d["tasks"]!.array!.first { $0["id"] == 202 }!["status"], "complete")
    }

    func testStatusAutomationsRun() {
        var d = SampleWorkspace.make()
        d.set("extra.automations", [
            ["id": 1, "enabled": true, "trigger_status": "review", "action": "assign", "value": "3", "list": nil],
            ["id": 2, "enabled": true, "trigger_status": "review", "action": "priority", "value": "1", "list": 100],
            ["id": 3, "enabled": true, "trigger_status": "review", "action": "comment", "value": "Please review", "list": 111],
            ["id": 4, "enabled": false, "trigger_status": "review", "action": "priority", "value": "4", "list": nil],
            ["id": 5, "enabled": true, "trigger_status": "review", "action": "priority", "value": "4", "list": 120],
        ])
        XCTAssertTrue(Edits.setStatus(&d, task: 202, to: "review", me: 1, ts: 600))
        let t = WorkspaceView(d).tasks.first { $0.id == 202 }!
        XCTAssertEqual(t.assignees, [1, 2, 3])
        XCTAssertEqual(t.priority, 1, "the space rule ran, the other list's and the disabled one didn't")
        XCTAssertTrue(t.activity.contains { $0.comment && $0.text == "Please review" })
        let ran = t.activity.filter { $0.text.hasPrefix("Automation ran: ") }.map(\.text)
        XCTAssertEqual(ran, ["Automation ran: assign to Sam Lee", "Automation ran: set priority to Urgent", "Automation ran: comment \"Please review\""])
    }

    func testAVideoNamesItsStillAndOldOnesHaveNone() {
        var d = SampleWorkspace.make()
        let still = String(repeating: "b", count: 64)
        let v = FileAPI.attachment(sha: String(repeating: "a", count: 64), name: "Video-1.mov", isVideo: true, bytes: 99, w: 1920, h: 1080, secs: 3, thumb: still)
        XCTAssertEqual(v["thumb"], .string(still))
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "", attachments: [v], me: 1, id: 9410, ts: 9))
        let m = WorkspaceView(d).channels.first { $0.id == 301 }!.messages.last!
        XCTAssertEqual(m.attachments.first?.thumb, still)
        // a picture never names one; a video from before stills has none
        let p = FileAPI.attachment(sha: String(repeating: "c", count: 64), name: "a.jpg", isVideo: false, bytes: 1, w: 1, h: 1, secs: 0, thumb: still)
        XCTAssertNil(p["thumb"])
        let old = FileAPI.attachment(sha: String(repeating: "d", count: 64), name: "b.mov", isVideo: true, bytes: 1, w: 1, h: 1, secs: 1)
        XCTAssertNil(old["thumb"], "left out, as the Mac app does")
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "", attachments: [old, ["file": .string(String(repeating: "e", count: 64)), "name": "c.mov", "kind": "Video", "thumb": "not-a-hash"]], me: 1, id: 9411, ts: 10))
        let m2 = WorkspaceView(d).channels.first { $0.id == 301 }!.messages.last!
        XCTAssertEqual(m2.attachments.map(\.thumb), [nil, nil])
    }
}
