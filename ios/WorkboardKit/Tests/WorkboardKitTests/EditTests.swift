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

    func testDueTimeFromTheMacIsShownKeptAndClearedWithTheDate() {
        var d = SampleWorkspace.make()
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        let today = f.date(from: "2026-11-14")!
        // the Mac app set a due date with a time
        d.updateItem(at: "tasks", id: 205) { j in
            j["due"] = "2026-11-14"
            j["due_time"] = "14:30"
        }
        var t = WorkspaceView(d).tasks.first { $0.id == 205 }!
        XCTAssertEqual(t.dueTime, "14:30")
        XCTAssertEqual(t.dueTimeLabel, "2:30 PM")
        XCTAssertFalse(t.isOverdue(today: "2026-11-14", now: "14:29"))
        XCTAssertTrue(t.isOverdue(today: "2026-11-14", now: "14:30"))
        XCTAssertTrue(t.isOverdue(today: "2026-11-15", now: "08:00"))
        // moving the day keeps the time, and the history says so
        XCTAssertTrue(Edits.setDue(&d, task: 205, to: "2026-11-15", me: 1, ts: 95, today: today))
        t = WorkspaceView(d).tasks.first { $0.id == 205 }!
        XCTAssertEqual(t.dueTime, "14:30")
        XCTAssertEqual(t.activity.last?.text, "changed due date to Tomorrow at 2:30 PM")
        // clearing the date clears the time, written as null so a merge keeps it gone
        XCTAssertTrue(Edits.setDue(&d, task: 205, to: nil, me: 1, ts: 96, today: today))
        var raw = d["tasks"]!.array!.first { $0["id"] == 205 }!
        XCTAssertEqual(raw["due_time"], .null)
        XCTAssertNil(WorkspaceView(d).tasks.first { $0.id == 205 }!.dueTime)
        // a new date afterwards starts without a time
        XCTAssertTrue(Edits.setDue(&d, task: 205, to: "2026-11-20", me: 1, ts: 97, today: today))
        raw = d["tasks"]!.array!.first { $0["id"] == 205 }!
        XCTAssertEqual(raw["due_time"], .null)
        // a task that never had a time gets no field
        XCTAssertTrue(Edits.setDue(&d, task: 204, to: nil, me: 1, ts: 98, today: today))
        XCTAssertNil(d["tasks"]!.array!.first { $0["id"] == 204 }!["due_time"])
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

    func testAssignAutomationLogsTheTargetAndReplacesWhenSingleAssignee() {
        var d = SampleWorkspace.make()
        d.set("extra.automations", [["id": 1, "enabled": true, "trigger_status": "review", "action": "assign", "value": "3", "list": nil]])
        XCTAssertTrue(Edits.setStatus(&d, task: 202, to: "review", me: 1, ts: 600))
        let line = task(d, 202)["activity"]!.array!.first { $0["text"] == "assigned to Sam Lee" }
        XCTAssertEqual(line?["target"], 3, "the Inbox goes by the target, like about_line on the Mac")
        XCTAssertEqual(line?["comment"], false)

        // Apps > Multiple Assignees off: the rule's person replaces the others, nothing else logged
        var single = SampleWorkspace.make()
        single.set("extra.automations", [["id": 1, "enabled": true, "trigger_status": "review", "action": "assign", "value": "3", "list": nil]])
        single.set("extra.apps", ["multiple_assignees": false])
        XCTAssertTrue(Edits.setStatus(&single, task: 202, to: "review", me: 1, ts: 600))
        let t = WorkspaceView(single).tasks.first { $0.id == 202 }!
        XCTAssertEqual(t.assignees, [3])
        XCTAssertFalse(t.activity.contains { $0.text.hasPrefix("assigned to") })
        XCTAssertEqual(t.activity.last?.text, "Automation ran: assign to Sam Lee")
    }

    func testMoveAutomationTakesSubtasksAndFitsStatuses() {
        var d = SampleWorkspace.make()
        // a second space with its own statuses, and a subtask under 202
        var spaces = d["spaces"]!.array!
        spaces.append(["id": 500, "name": "Ops", "statuses": [["name": "open", "color": [1, 2, 3], "kind": "Open"], ["name": "review", "color": [1, 2, 3], "kind": "Custom"]],
                       "folders": [], "lists": [["id": 501, "name": "Queue", "archived": false]], "archived": false])
        d["spaces"] = .array(spaces)
        Edits.addTask(&d, list: 111, name: "Sub", assignees: [], me: 1, id: 9100, ts: 10)
        d.updateItem(at: "tasks", id: 9100) { $0["parent"] = 202; $0["status"] = "in progress" }
        d.set("extra.automations", [["id": 1, "enabled": true, "trigger_status": "review", "action": "move", "value": "501", "list": nil]])
        XCTAssertTrue(Edits.setStatus(&d, task: 202, to: "review", me: 1, ts: 600))
        let t = task(d, 202), sub = task(d, 9100)
        XCTAssertEqual(t["list_id"], 501)
        XCTAssertEqual(t["status"], "review", "a status the new list has stays")
        XCTAssertEqual(sub["list_id"], 501, "subtasks go along")
        XCTAssertEqual(sub["status"], "open", "a status it doesn't have falls back to the first")
        XCTAssertEqual(sub["parent"], 202)
        let texts = t["activity"]!.array!.compactMap { $0["text"]?.string }
        XCTAssertEqual(Array(texts.suffix(3)), ["changed status from TO DO to REVIEW", "Automation ran: move to Queue", "moved this task to Queue"])
        XCTAssertFalse(sub["activity"]!.array!.contains { $0["text"]?.string?.hasPrefix("moved") == true }, "only the task itself logs the move")

        // a rule naming a list that doesn't exist logs but moves nothing
        var e = SampleWorkspace.make()
        e.set("extra.automations", [["id": 1, "enabled": true, "trigger_status": "review", "action": "move", "value": "999", "list": nil]])
        XCTAssertTrue(Edits.setStatus(&e, task: 202, to: "review", me: 1, ts: 600))
        XCTAssertEqual(task(e, 202)["list_id"], 111)
        XCTAssertEqual(task(e, 202)["activity"]!.array!.last?["text"], "Automation ran: move to List")
    }

    func testMovingASubtaskAloneMakesItATask() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "Sub", assignees: [], me: 1, id: 9100, ts: 10)
        d.updateItem(at: "tasks", id: 9100) { $0["parent"] = 202 }
        XCTAssertTrue(Edits.moveTasks(&d, ids: [9100], to: 120, me: 1, ts: 20))
        XCTAssertEqual(task(d, 9100)["parent"], .null)
        XCTAssertEqual(task(d, 9100)["list_id"], 120)
        XCTAssertEqual(task(d, 202)["list_id"], 111)
        XCTAssertFalse(Edits.moveTasks(&d, ids: [9100], to: 777, me: 1, ts: 21), "no such list")
    }

    func testMentionsFollowTheMacRule() {
        let people: [(id: UInt64, name: String)] = [(1, "Alex"), (2, "Alex Kim"), (3, "지원"), (4, "Sam"), (5, "Sam")]
        XCTAssertEqual(Mentions.ids(in: "@Alex Kim look", members: people), [2], "the longest name wins")
        XCTAssertEqual(Mentions.ids(in: "@Alex look", members: people), [1])
        XCTAssertEqual(Mentions.ids(in: "@Alexander hi", members: people), [], "a Latin name running on is not that name")
        XCTAssertEqual(Mentions.ids(in: "@지원님 안녕", members: people), [3], "anything else may follow")
        XCTAssertEqual(Mentions.ids(in: "mail me@Alex", members: people), [], "@ must start a word")
        XCTAssertEqual(Mentions.ids(in: "hi\n@Alex, and @Alex again", members: people), [1])
        XCTAssertEqual(Mentions.ids(in: "@Sam!", members: people), [4, 5], "two share the name: both")
        XCTAssertEqual(Mentions.ids(in: "@Sam!", members: people, picked: [5]), [5], "unless one was picked")
        XCTAssertEqual(Mentions.ids(in: "@Alex.", members: people), [1])
        XCTAssertEqual(Mentions.ids(in: "@", members: people), [])
    }

    func testSendingAndEditingWorkOutMentions() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "@Alex Kim can you look?", me: 1, id: 9001, ts: 60))
        func msg() -> JSON { d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 9001 }! }
        XCTAssertEqual(msg()["mentions"], [2])
        XCTAssertTrue(Edits.editMessage(&d, channel: 301, message: 9001, text: "@Sam Lee can you look?", me: 1, ts: 61))
        XCTAssertEqual(msg()["mentions"], [3], "the edit works them out again")
        XCTAssertTrue(Edits.editMessage(&d, channel: 301, message: 9001, text: "never mind", me: 1, ts: 62))
        XCTAssertNil(msg()["mentions"], "left out when nobody is mentioned, as the Mac writes it")
        XCTAssertNil(d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 313 }!["mentions"])
        // replies and task comments too
        XCTAssertTrue(Edits.sendReply(&d, channel: 301, to: 313, text: "@Jamie Park ok", me: 3, id: 9002, ts: 63))
        XCTAssertEqual(d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 313 }!["replies"]!.array!.last!["mentions"], [1])
        XCTAssertTrue(Edits.editMessage(&d, channel: 301, message: 9002, text: "@Alex Kim ok", me: 3, ts: 64))
        XCTAssertEqual(d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 313 }!["replies"]!.array!.last!["mentions"], [2])
        XCTAssertTrue(Edits.addComment(&d, task: 201, text: "@Sam Lee see this", me: 1, ts: 65))
        XCTAssertEqual(task(d, 201)["activity"]!.array!.last!["mentions"], [3])
    }

    func testHuddleCardsCantBeEdited() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.postHuddle(&d, channel: 301, me: 1, id: 9001, ts: 60))
        XCTAssertFalse(Edits.editMessage(&d, channel: 301, message: 9001, text: "Something else", me: 1, ts: 61))
        XCTAssertTrue(Edits.postCall(&d, channel: 301, link: "https://meet.jit.si/x", me: 1, id: 9002, ts: 62))
        XCTAssertFalse(Edits.editMessage(&d, channel: 301, message: 9002, text: "Something else", me: 1, ts: 63))
        XCTAssertTrue(WorkspaceView(d).channels[0].messages.first { $0.id == 9001 }!.isEditable(by: 1) == false)
        XCTAssertTrue(WorkspaceView(d).channels[0].messages.first { $0.id == 313 }!.isEditable(by: 3))
        XCTAssertFalse(WorkspaceView(d).channels[0].messages.first { $0.id == 313 }!.isEditable(by: 1), "not mine")
    }

    func testNewChannelRecordsItsCreator() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.createChannel(&d, name: "Ops", isPrivate: false, me: 2, id: 9300, ts: 70))
        XCTAssertEqual(d["channels"]!.array!.last!["creator"], 2)
    }

    func testDMReusesTheOneTheMacShowsOrStartsANewOne() {
        var d = SampleWorkspace.make()
        // 303 is Jamie (1) and Alex (2), with messages
        XCTAssertEqual(Edits.openDM(&d, with: 2, me: 1, id: 9600, ts: 5), 303)
        XCTAssertEqual(Edits.openDM(&d, with: 1, me: 2, id: 9601, ts: 5), 303, "the same conversation from the other side")
        let before = d["channels"]!.array!.count
        XCTAssertEqual(Edits.openDM(&d, with: 3, me: 1, id: 9602, ts: 6), 9602)
        XCTAssertEqual(d["channels"]!.array!.count, before + 1)
        let c = d["channels"]!.array!.last!
        for k in ["id", "name", "kind", "topic", "members", "messages"] { XCTAssertNotNil(c[k], k) }
        XCTAssertEqual(c["kind"], "Direct")
        XCTAssertNil(c["creator"], "the Mac app leaves creator out of a direct message")
        let v = WorkspaceView(d)
        XCTAssertEqual(v.channels.last!.members, [1, 3])
        XCTAssertTrue(v.visibleChannels(me: 3).contains { $0.id == 9602 })
        XCTAssertFalse(v.visibleChannels(me: 2).contains { $0.id == 9602 }, "only the two people in it see it")
        XCTAssertEqual(Edits.openDM(&d, with: 3, me: 1, id: 9603, ts: 7), 9602, "started once, then reused")
        XCTAssertNil(Edits.openDM(&d, with: 1, me: 1, id: 9604, ts: 7), "not with myself")
        XCTAssertNil(Edits.openDM(&d, with: 99, me: 1, id: 9605, ts: 7), "not with someone who isn't here")
        XCTAssertEqual(d["channels"]!.array!.count, before + 1)
    }

    func testDMPicksTheCopyWithMessagesThenTheOldest() {
        var d = SampleWorkspace.make()
        let empty: JSON = ["id": 9700, "name": "", "kind": "Direct", "topic": "", "members": [3, 1], "messages": [], "last_read": 0]
        let older: JSON = ["id": 9701, "name": "", "kind": "Direct", "topic": "", "members": [1, 3], "messages": [], "last_read": 0]
        d.append(at: "channels", empty)
        d.append(at: "channels", older)
        XCTAssertEqual(Edits.openDM(&d, with: 3, me: 1, id: 9800, ts: 1), 9700, "no messages anywhere: the oldest")
        XCTAssertTrue(Edits.sendMessage(&d, channel: 9701, text: "hi", me: 3, id: 9801, ts: 2))
        XCTAssertEqual(Edits.openDM(&d, with: 3, me: 1, id: 9802, ts: 3), 9701, "the one with messages")
        // a group conversation with the same two people and someone else isn't it
        let group: JSON = ["id": 9500, "name": "", "kind": "Direct", "topic": "", "members": [1, 2, 3], "messages": [], "last_read": 0]
        d.append(at: "channels", group)
        XCTAssertEqual(Edits.openDM(&d, with: 3, me: 1, id: 9803, ts: 4), 9701)
    }

    func testSubtaskGoesInItsParentsListUnderIt() {
        var d = SampleWorkspace.make()
        let parent = WorkspaceView(d).tasks.first { $0.id == 201 }!
        Edits.addTask(&d, list: parent.listId, name: " Write copy ", assignees: [], parent: 201, me: 1, id: 9900, ts: 40)
        let t = d["tasks"]!.array!.last!
        for k in taskRequired { XCTAssertNotNil(t[k], k) }
        XCTAssertEqual(t["parent"], 201)
        XCTAssertEqual(t["list_id"], .id(parent.listId))
        let v = WorkspaceView(d)
        XCTAssertEqual(v.subtasks(of: 201).map(\.id), [9900])
        XCTAssertEqual(v.subtasks(of: 201).first?.name, "Write copy")
        XCTAssertFalse(v.tasks(inList: parent.listId).contains { $0.id == 9900 }, "a subtask shows under its parent, not in the list")
        Edits.addTask(&d, list: 111, name: "top", assignees: [], me: 1, id: 9901, ts: 41)
        XCTAssertEqual(d["tasks"]!.array!.last!["parent"], .null, "a task without a parent keeps parent: null")
    }

    func testChecklistAddTickAndRemove() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addCheckItem(&d, task: 201, text: "  Draft  ", ts: 10))
        XCTAssertTrue(Edits.addCheckItem(&d, task: 201, text: "Review", ts: 11))
        XCTAssertFalse(Edits.addCheckItem(&d, task: 201, text: "   ", ts: 12), "an empty item isn't added")
        XCTAssertFalse(Edits.addCheckItem(&d, task: 99999, text: "x", ts: 12), "no such task")
        func task() -> JSON { d["tasks"]!.array!.first { $0["id"] == 201 }! }
        XCTAssertEqual(task()["checklist"], [["text": "Draft", "done": false], ["text": "Review", "done": false]])
        XCTAssertEqual(task()["updated"], 11)

        XCTAssertTrue(Edits.setCheckItem(&d, task: 201, index: 1, done: true, ts: 13))
        XCTAssertFalse(Edits.setCheckItem(&d, task: 201, index: 1, done: true, ts: 14), "already ticked: nothing changes")
        XCTAssertFalse(Edits.setCheckItem(&d, task: 201, index: 5, done: true, ts: 14))
        XCTAssertEqual(task()["updated"], 13)
        let v = WorkspaceView(d).tasks.first { $0.id == 201 }!
        XCTAssertEqual(v.checklist.map(\.text), ["Draft", "Review"])
        XCTAssertEqual(v.checklist.map(\.done), [false, true])

        // an item the Mac app gave a person keeps them when ticked here
        d.updateItem(at: "tasks", id: 201) { $0["checklist"] = [["text": "Call", "done": false, "assignee": 2]] }
        XCTAssertTrue(Edits.setCheckItem(&d, task: 201, index: 0, done: true, ts: 15))
        XCTAssertEqual(task()["checklist"], [["text": "Call", "done": true, "assignee": 2]])
        XCTAssertEqual(WorkspaceView(d).tasks.first { $0.id == 201 }!.checklist.first?.assignee, 2)

        XCTAssertTrue(Edits.removeCheckItem(&d, task: 201, index: 0, ts: 16))
        XCTAssertFalse(Edits.removeCheckItem(&d, task: 201, index: 0, ts: 17))
        XCTAssertEqual(task()["checklist"], [])
        XCTAssertEqual(task()["updated"], 16)
    }

    func testDeletedTaskGoesToTrashAndComesBack() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "Sub", assignees: [], me: 1, id: 9100, ts: 10)
        d.updateItem(at: "tasks", id: 9100) { $0["parent"] = 202 }
        let before = d["tasks"]!.array!.filter { [202, 9100].contains($0["id"]!.uint!) }
        let item = try! XCTUnwrap(Edits.trashTask(&d, task: 202, id: 7000, ts: 80))
        XCTAssertEqual(item.kind, "Task")
        XCTAssertEqual(item.title, "Check pricing page copy")
        XCTAssertEqual(item.deleted, 80)
        XCTAssertEqual(item.items, before, "the task and its subtask, as they were")
        XCTAssertNil(d["tasks"]!.array!.first { $0["id"] == 202 })
        XCTAssertNil(d["tasks"]!.array!.first { $0["id"] == 9100 })
        XCTAssertNil(Edits.trashTask(&d, task: 202, id: 7001, ts: 81), "already gone")
        // the Mac's TrashItem shape round trips
        XCTAssertEqual(TrashItem(item.json), item)
        XCTAssertEqual(Set(item.json.object!.keys), ["id", "kind", "title", "deleted", "payload"])

        XCTAssertEqual(Edits.restore(&d, item, me: 1, ts: 90), .restored)
        XCTAssertEqual(task(d, 202), before[0])
        XCTAssertEqual(task(d, 9100), before[1])
    }

    func testRestoredTaskWhoseListIsGoneGoesToTheFirstList() {
        var d = SampleWorkspace.make()
        let item = try! XCTUnwrap(Edits.trashTask(&d, task: 203, id: 7000, ts: 80))
        // the App list (112) goes; the first list is Website (111, in the Launch folder)
        d.set("spaces", .array(d["spaces"]!.array!.map { s in
            var s = s
            s["folders"] = .array(s["folders"]!.array!.map { f in var f = f; f["lists"] = .array(f["lists"]!.array!.filter { $0["id"] != 112 }); return f })
            return s
        }))
        XCTAssertEqual(Edits.restore(&d, item, me: 1, ts: 90), .movedTo("Website"))
        XCTAssertEqual(task(d, 203)["list_id"], 111)
        XCTAssertEqual(task(d, 203)["status"], "review", "the status exists there too")

        var empty = SampleWorkspace.make()
        let lost = try! XCTUnwrap(Edits.trashTask(&empty, task: 203, id: 7001, ts: 80))
        empty["spaces"] = []
        if case .nowhere = Edits.restore(&empty, lost, me: 1, ts: 90) {} else { XCTFail("nowhere to go") }
        XCTAssertNil(empty["tasks"]!.array!.first { $0["id"] == 203 })
    }

    func testDeletedDocGoesToTrashWithSubPagesAndComesBack() {
        var d = SampleWorkspace.make()
        Edits.addDoc(&d, title: "Child", space: 100, me: 1, id: 9200, ts: 10)
        d.updateItem(at: "docs", id: 9200) { $0["parent"] = 401 }
        let item = try! XCTUnwrap(Edits.trashDoc(&d, doc: 401, id: 7000, ts: 80))
        XCTAssertEqual(item.kind, "Doc")
        XCTAssertEqual(item.title, "Launch plan")
        XCTAssertEqual(item.items.compactMap { $0["id"]?.uint }, [401, 9200])
        XCTAssertEqual(WorkspaceView(d).docs.map(\.id), [402])
        XCTAssertEqual(Edits.restore(&d, item, me: 1, ts: 90), .restored)
        XCTAssertEqual(Set(WorkspaceView(d).docs.map(\.id)), [401, 402, 9200])
        XCTAssertEqual(d["docs"]!.array!.first { $0["id"] == 9200 }!["parent"], 401)

        // restoring only a sub page whose parent is gone puts it at the top
        var e = SampleWorkspace.make()
        Edits.addDoc(&e, title: "Child", space: 100, me: 1, id: 9200, ts: 10)
        e.updateItem(at: "docs", id: 9200) { $0["parent"] = 401 }
        let child = try! XCTUnwrap(Edits.trashDoc(&e, doc: 9200, id: 7001, ts: 80))
        XCTAssertTrue(Edits.deleteDoc(&e, doc: 401))
        XCTAssertEqual(Edits.restore(&e, child, me: 1, ts: 90), .restored)
        XCTAssertEqual(e["docs"]!.array!.first { $0["id"] == 9200 }!["parent"], .null)
    }

    func testRestoreRevivesForOthers() {
        // delete stamps a tombstone; restoring records a revival that wins in a merge
        var d = SampleWorkspace.make()
        var meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: 1000)
        d.set("extra.sync", meta.json)
        let item = try! XCTUnwrap(Edits.trashTask(&d, task: 205, id: 7000, ts: 80))
        meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: 2000)
        d.set("extra.sync", meta.json)
        XCTAssertNotNil(SyncMeta(of: d).tombs[205])
        var other = d // a teammate's copy without the task
        XCTAssertEqual(Edits.restore(&d, item, me: 1, ts: 90), .restored)
        meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: 3000)
        d.set("extra.sync", meta.json)
        XCTAssertEqual(SyncMeta(of: d).revived[205], 3000)
        Sync.merge(&other, [d])
        XCTAssertNotNil(other["tasks"]!.array!.first { $0["id"] == 205 }, "the restored task comes back for others")
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

/// Round 4: the @ list in the iPhone's message box.
final class MentionPickTests: XCTestCase {
    func data() -> JSON {
        [
            "members": [
                ["id": 1, "name": "Jamie Park"], ["id": 2, "name": "Alex", "email": "alex@a.test"],
                ["id": 5, "name": "Alex", "email": "alex@b.test"], ["id": 6, "name": "Alexis", "invited": true],
            ],
            "channels": [["id": 301, "name": "general", "kind": "Channel", "members": [], "messages": [
                ["id": 311, "author": 1, "text": "hi", "ts": 1, "replies": []],
            ]]],
            "tasks": [["id": 201, "name": "T", "activity": []]],
        ]
    }

    func testThePersonPickedFromTheListIsTheOneMentioned() {
        var d = data()
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "@Alex look", picked: [5], me: 1, id: 900, ts: 2))
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "@Alex look", me: 1, id: 901, ts: 3))
        XCTAssertTrue(Edits.sendReply(&d, channel: 301, to: 311, text: "@Alex you", picked: [2], me: 1, id: 902, ts: 4))
        XCTAssertTrue(Edits.addComment(&d, task: 201, text: "@Alex here", picked: [5], me: 1, ts: 5))
        let msgs = d["channels"]![0]!["messages"]!.array!
        XCTAssertEqual(msgs.first { $0["id"] == 900 }!["mentions"], [5], "the picked Alex")
        XCTAssertEqual(msgs.first { $0["id"] == 901 }!["mentions"], [2, 5], "typed by hand: both")
        XCTAssertEqual(msgs.first { $0["id"] == 311 }!["replies"]![0]!["mentions"], [2])
        XCTAssertEqual(d["tasks"]![0]!["activity"]![0]!["mentions"], [5])
        // an edit keeps the pick (editMessage reads it back)
        Edits.editMessage(&d, channel: 301, message: 900, text: "@Alex look again", me: 1, ts: 6)
        XCTAssertEqual(d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 900 }!["mentions"], [5])
    }

    func testTheListFollowsWhatIsTyped() {
        XCTAssertEqual(Mentions.query(in: "@"), "")
        XCTAssertEqual(Mentions.query(in: "hi @al"), "al")
        XCTAssertEqual(Mentions.query(in: "hi @Alex K"), "Alex K")
        XCTAssertNil(Mentions.query(in: "mail me@al"), "@ must start a word")
        XCTAssertNil(Mentions.query(in: "hi @ al"))
        XCTAssertNil(Mentions.query(in: "hi"))
        XCTAssertNil(Mentions.query(in: "@al\nnext"))
        let members = WorkspaceView(data()).members
        XCTAssertEqual(Mentions.candidates("al", members: members, me: 1).map(\.id), [2, 5], "not the invited one")
        XCTAssertEqual(Mentions.candidates("", members: members, me: 1).map(\.id), [2, 5], "not me")
        XCTAssertEqual(Mentions.candidates("jam", members: members, me: 2).map(\.id), [1])
        XCTAssertEqual(Mentions.complete("hi @al", with: "Alex"), "hi @Alex ")
        XCTAssertEqual(Mentions.complete("@", with: "Jamie Park"), "@Jamie Park ")
    }
}
