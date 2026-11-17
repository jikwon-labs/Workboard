import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The task fields the Mac app has and the phone now edits too, written the
/// way src/app.rs and src/app/taskfields.rs write them.
final class TaskFieldTests: XCTestCase {
    func task(_ d: JSON, _ id: UInt64) -> JSON { d["tasks"]!.array!.first { $0["id"]?.uint == id }! }
    func lastLine(_ d: JSON, _ id: UInt64) -> String { task(d, id)["activity"]!.array!.last!["text"]!.string! }
    let today = ISO8601DateFormatter().date(from: "2026-11-17T12:00:00Z")!

    func testStartDateAndTimesAreLoggedAndGoWithTheirDate() {
        var d = SampleWorkspace.make(now: today)
        XCTAssertTrue(Edits.setStart(&d, task: 205, to: "2026-11-18", me: 1, ts: 10, today: today))
        XCTAssertEqual(lastLine(d, 205), "changed start date to Tomorrow")
        XCTAssertTrue(Edits.setStartTime(&d, task: 205, to: "9:05", me: 1, ts: 11, today: today))
        XCTAssertEqual(task(d, 205)["start_time"], "09:05")
        XCTAssertEqual(lastLine(d, 205), "changed start date to Tomorrow at 9:05 AM")
        XCTAssertEqual(WorkspaceView(d).tasks.first { $0.id == 205 }!.startTime, "09:05")
        // no time without a due date
        XCTAssertFalse(Edits.setDueTime(&d, task: 205, to: "14:30", me: 1, ts: 12, today: today))
        XCTAssertNil(task(d, 205)["due_time"])
        XCTAssertFalse(Edits.setStartTime(&d, task: 205, to: "25:00", me: 1, ts: 12, today: today))
        // clearing the date takes the time, written as null
        XCTAssertTrue(Edits.setStart(&d, task: 205, to: nil, me: 1, ts: 13, today: today))
        XCTAssertEqual(task(d, 205)["start_time"], .null)
        XCTAssertEqual(lastLine(d, 205), "changed start date to none")
    }

    func testDueTimeIsSetAndRemoved() {
        var d = SampleWorkspace.make(now: today)
        XCTAssertTrue(Edits.setDueTime(&d, task: 201, to: "14:30", me: 1, ts: 10, today: today))
        XCTAssertEqual(lastLine(d, 201), "changed due date to Today at 2:30 PM")
        XCTAssertTrue(Edits.setDueTime(&d, task: 201, to: nil, me: 1, ts: 11, today: today))
        XCTAssertEqual(task(d, 201)["due_time"], .null)
        XCTAssertEqual(lastLine(d, 201), "changed due date to Today")
    }

    func testTagsAreLowerCaseAndLogged() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addTag(&d, task: 201, tag: " Launch ", me: 1, ts: 10))
        XCTAssertFalse(Edits.addTag(&d, task: 201, tag: "launch", me: 1, ts: 11))
        XCTAssertEqual(task(d, 201)["tags"], ["launch"])
        XCTAssertEqual(lastLine(d, 201), "added tag launch")
        XCTAssertEqual(WorkspaceView(d).allTags, ["launch"])
        XCTAssertTrue(Edits.removeTag(&d, task: 201, tag: "launch", me: 1, ts: 12))
        XCTAssertEqual(lastLine(d, 201), "removed tag launch")
        XCTAssertEqual(task(d, 201)["tags"], [])
    }

    func testWatchersToggle() {
        var d = SampleWorkspace.make()
        Edits.toggleWatcher(&d, task: 201, member: 2, ts: 10)
        XCTAssertEqual(WorkspaceView(d).tasks.first { $0.id == 201 }!.watchers, [2])
        Edits.toggleWatcher(&d, task: 201, member: 2, ts: 11)
        XCTAssertEqual(task(d, 201)["watchers"], [])
    }

    func testRelationshipsAreStoredOnOneSideAndReadOnBoth() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addRelation(&d, task: 201, .waitingOn, other: 202, me: 1, ts: 10))
        XCTAssertEqual(lastLine(d, 201), "is waiting on Check pricing page copy")
        XCTAssertTrue(Edits.addRelation(&d, task: 201, .blocking, other: 203, me: 1, ts: 11))
        // Blocking is written on the other task as Waiting on
        XCTAssertEqual(task(d, 203)["waiting_on"], [201])
        XCTAssertEqual(lastLine(d, 203), "is waiting on Write the launch post")
        XCTAssertTrue(Edits.addRelation(&d, task: 204, .linked, other: 201, me: 1, ts: 12))
        XCTAssertFalse(Edits.addRelation(&d, task: 201, .linked, other: 201, me: 1, ts: 12))
        let r = WorkspaceView(d).relations(of: 201)
        XCTAssertEqual(r.waitingOn.map(\.id), [202])
        XCTAssertEqual(r.blocking.map(\.id), [203])
        XCTAssertEqual(r.linked.map(\.id), [204])
        XCTAssertTrue(Edits.removeRelation(&d, task: 201, .blocking, other: 203, ts: 13))
        XCTAssertTrue(Edits.removeRelation(&d, task: 201, .linked, other: 204, ts: 13))
        let after = WorkspaceView(d).relations(of: 201)
        XCTAssertTrue(after.blocking.isEmpty)
        XCTAssertTrue(after.linked.isEmpty)
    }

    func testParentMovesTheTaskIntoItsListAndNeverUnderItself() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setParent(&d, task: 205, to: 201, me: 1, ts: 10))
        XCTAssertEqual(task(d, 205)["list_id"], 111)
        XCTAssertEqual(lastLine(d, 205), "made this a subtask of Write the launch post")
        // 201 can't go under its own subtask
        XCTAssertFalse(Edits.setParent(&d, task: 201, to: 205, me: 1, ts: 11))
        XCTAssertFalse(WorkspaceView(d).parentCandidates(for: 201).contains { $0.id == 205 })
        XCTAssertTrue(Edits.setParent(&d, task: 205, to: nil, me: 1, ts: 12))
        XCTAssertEqual(task(d, 205)["parent"], .null)
        XCTAssertEqual(lastLine(d, 205), "made this a top-level task")
    }

    func testRepeatGivesADueDateWhenThereIsNone() {
        var d = SampleWorkspace.make(now: today)
        XCTAssertTrue(Edits.setRecur(&d, task: 205, to: "weekly", me: 1, ts: 10, today: today))
        XCTAssertEqual(task(d, 205)["recur"], "weekly")
        XCTAssertEqual(task(d, 205)["due"]?.string, dayString(today))
        XCTAssertTrue(Edits.setRecur(&d, task: 205, to: nil, me: 1, ts: 11, today: today))
        XCTAssertEqual(task(d, 205)["recur"], .null)
    }

    func testEstimateParsingMatchesTheMac() {
        XCTAssertEqual(Edits.parseEstimate("2h 30m"), 150)
        XCTAssertEqual(Edits.parseEstimate("1.5h"), 90)
        XCTAssertEqual(Edits.parseEstimate("45m"), 45)
        XCTAssertEqual(Edits.parseEstimate("1d"), 480)
        XCTAssertEqual(Edits.parseEstimate("2:30"), 150)
        XCTAssertEqual(Edits.parseEstimate("2"), 120)
        XCTAssertEqual(Edits.parseEstimate("2h 30"), 150)
        XCTAssertEqual(Edits.parseEstimate("1\u{c2dc}\u{ac04} 10\u{bd84}"), 70)
        XCTAssertNil(Edits.parseEstimate("soon"))
        XCTAssertNil(Edits.parseEstimate(""))
        XCTAssertEqual(Edits.fmtEstimate(150), "2h 30m")
        XCTAssertEqual(Edits.fmtEstimate(120), "2h")
        XCTAssertEqual(Edits.fmtEstimate(45), "45m")
    }

    func testEstimateLogUpdatesMyRecentLine() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setEstimate(&d, task: 201, minutes: 150, me: 1, ts: 1000))
        XCTAssertEqual(lastLine(d, 201), "set the time estimate to 2h 30m")
        let n = task(d, 201)["activity"]!.array!.count
        Edits.setEstimate(&d, task: 201, minutes: 45, me: 1, ts: 1100)
        XCTAssertEqual(task(d, 201)["activity"]!.array!.count, n)
        XCTAssertEqual(lastLine(d, 201), "set the time estimate to 45m 0s")
        Edits.setEstimate(&d, task: 201, minutes: nil, me: 1, ts: 2000)
        XCTAssertEqual(task(d, 201)["estimate_min"], .null)
        XCTAssertEqual(task(d, 201)["activity"]!.array!.count, n + 1)
        XCTAssertEqual(lastLine(d, 201), "removed the time estimate")
    }

    func testTimersForTwoPeopleKeepEachOnesTime() {
        var d = SampleWorkspace.make()
        Edits.startTimer(&d, task: 201, me: 1, ts: 100)
        Edits.startTimer(&d, task: 201, me: 2, ts: 150)
        Edits.startTimer(&d, task: 201, me: 1, ts: 160) // already running
        XCTAssertEqual(task(d, 201)["timer_started"], 100)
        XCTAssertEqual(task(d, 201)["timer_by"], 1)
        XCTAssertEqual(task(d, 201)["timers"]!.array!.count, 1)
        let v = WorkspaceView(d)
        let t = v.tasks.first { $0.id == 201 }!
        XCTAssertEqual(v.myTimerStart(t, me: 2), 150)
        XCTAssertEqual(v.trackedSecs(t, now: 200), 100 + 50)
        // stopping mine moves Alex's timer into the field older apps read
        XCTAssertTrue(Edits.stopTimer(&d, task: 201, me: 1, ts: 300))
        XCTAssertEqual(task(d, 201)["timer_started"], 150)
        XCTAssertEqual(task(d, 201)["timer_by"], 2)
        XCTAssertNil(task(d, 201)["timers"])
        XCTAssertEqual(task(d, 201)["time_entries"], [["start": 100, "end": 300, "member": 1]])
        XCTAssertFalse(Edits.stopTimer(&d, task: 201, me: 1, ts: 310))
        XCTAssertTrue(Edits.stopTimer(&d, task: 201, me: 2, ts: 400))
        XCTAssertEqual(task(d, 201)["timer_started"], .null)
        XCTAssertNil(task(d, 201)["timer_by"])
        XCTAssertEqual(task(d, 201)["time_entries"]!.array!.count, 2)
    }

    func testAnOldTimerWithoutAnOwnerIsMineOnlyWhenAlone() {
        var d = SampleWorkspace.make()
        d.updateItem(at: "tasks", id: 201) { $0["timer_started"] = 100 }
        XCTAssertFalse(Edits.stopTimer(&d, task: 201, me: 1, ts: 200))
        d["members"] = .array([d["members"]!.array![0]])
        XCTAssertTrue(Edits.stopTimer(&d, task: 201, me: 1, ts: 200))
    }

    func testTimeEntriesAddAndRemove() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addTimeEntry(&d, task: 201, minutes: 90, yesterday: true, me: 2, ts: 100_000))
        let e = task(d, 201)["time_entries"]!.array![0]
        XCTAssertEqual(e["end"], .int(100_000 - 86400))
        XCTAssertEqual(e["start"], .int(100_000 - 86400 - 5400))
        XCTAssertFalse(Edits.addTimeEntry(&d, task: 201, minutes: 0, yesterday: false, me: 2, ts: 100_000))
        XCTAssertTrue(Edits.removeTimeEntry(&d, task: 201, member: 2, start: 100_000 - 86400 - 5400))
        XCTAssertEqual(task(d, 201)["time_entries"], [])
    }

    func testCustomFieldValuesAreTextAndEmptyRemovesThem() {
        var d = SampleWorkspace.make()
        d.set("extra.fields", [["id": 500, "name": "Budget", "kind": "number", "options": []], ["id": 501, "name": "Size", "kind": "dropdown", "options": ["S", "M"]]])
        XCTAssertTrue(Edits.setCustom(&d, task: 201, field: 500, to: "1200", ts: 10))
        XCTAssertTrue(Edits.setCustom(&d, task: 201, field: 501, to: "M", ts: 10))
        XCTAssertEqual(task(d, 201)["custom"], ["500": "1200", "501": "M"])
        let v = WorkspaceView(d)
        XCTAssertEqual(v.fields.map(\.name), ["Budget", "Size"])
        XCTAssertEqual(v.fields[1].options, ["S", "M"])
        XCTAssertEqual(v.tasks.first { $0.id == 201 }!.custom[500], "1200")
        XCTAssertTrue(Edits.setCustom(&d, task: 201, field: 500, to: "", ts: 11))
        XCTAssertEqual(task(d, 201)["custom"], ["501": "M"])
    }

    func testMilestoneIsLeftOutWhenOff() {
        var d = SampleWorkspace.make()
        Edits.setMilestone(&d, task: 201, true, ts: 10)
        XCTAssertEqual(task(d, 201)["milestone"], true)
        Edits.setMilestone(&d, task: 201, false, ts: 11)
        XCTAssertNil(task(d, 201)["milestone"])
    }

    func testChecklistRenameAssignAndConvert() {
        var d = SampleWorkspace.make()
        Edits.addCheckItem(&d, task: 201, text: "Draft", ts: 10)
        XCTAssertTrue(Edits.renameCheckItem(&d, task: 201, index: 0, to: "First draft", ts: 11))
        XCTAssertFalse(Edits.renameCheckItem(&d, task: 201, index: 0, to: "  ", ts: 11))
        XCTAssertTrue(Edits.setCheckAssignee(&d, task: 201, index: 0, to: 2, ts: 12))
        XCTAssertEqual(task(d, 201)["checklist"], [["text": "First draft", "done": false, "assignee": 2]])
        XCTAssertTrue(Edits.checkItemToSubtask(&d, task: 201, index: 0, me: 1, id: 9000, ts: 13))
        XCTAssertEqual(task(d, 201)["checklist"], [])
        let sub = task(d, 9000)
        XCTAssertEqual(sub["parent"], 201)
        XCTAssertEqual(sub["name"], "First draft")
        XCTAssertEqual(sub["assignees"], [2])
    }

    func testDuplicateCopiesSubtasksWithoutTime() {
        var d = SampleWorkspace.make()
        Edits.addTask(&d, list: 111, name: "Sub", assignees: [], parent: 201, me: 1, id: 900, ts: 5)
        Edits.addTimeEntry(&d, task: 201, minutes: 30, yesterday: false, me: 1, ts: 6)
        var next: UInt64 = 5000
        let nid = Edits.duplicateTask(&d, task: 201, me: 1, ts: 10) { next += 1; return next }
        XCTAssertEqual(nid, 5001)
        let copy = task(d, 5001)
        XCTAssertEqual(copy["name"], "Write the launch post (copy)")
        XCTAssertEqual(copy["time_entries"], [])
        XCTAssertEqual(copy["activity"]!.array!.first!["text"], "duplicated this task from Write the launch post")
        let sub = task(d, 5002)
        XCTAssertEqual(sub["parent"], 5001)
        XCTAssertEqual(sub["name"], "Sub")
        XCTAssertEqual(copy["mac_only_task_field"], ["kept": true])
    }

    func testArchiveHidesTheTask() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setArchived(&d, task: 201, true, me: 1, ts: 10))
        XCTAssertEqual(lastLine(d, 201), "archived this task")
        XCTAssertFalse(WorkspaceView(d).tasks.contains { $0.id == 201 })
    }

    func testNewFieldsSurviveAMergeWithTheServer() {
        var phone = SampleWorkspace.make()
        var meta = SyncMeta(of: phone)
        Sync.stamp(phone, &meta, now: 10)
        phone.set("extra.sync", meta.json)
        var server = phone
        Edits.addTag(&phone, task: 201, tag: "web", me: 1, ts: 20)
        Edits.addRelation(&phone, task: 201, .waitingOn, other: 202, me: 1, ts: 20)
        Edits.setCustom(&phone, task: 201, field: 500, to: "7", ts: 20)
        Edits.addTimeEntry(&phone, task: 201, minutes: 15, yesterday: false, me: 1, ts: 20)
        Sync.fillActivityIds(&phone)
        Sync.stamp(phone, &meta, now: 20)
        phone.set("extra.sync", meta.json)
        Sync.merge(&server, [phone])
        let t = task(server, 201)
        XCTAssertEqual(t["tags"], ["web"])
        XCTAssertEqual(t["waiting_on"], [202])
        XCTAssertEqual(t["custom"], ["500": "7"])
        XCTAssertEqual(t["time_entries"]!.array!.count, 1)
    }

    func testRenameAndDescriptionAreLoggedLikeTheMac() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.rename(&d, task: 201, to: "Launch post", me: 1, ts: 1000))
        XCTAssertEqual(lastLine(d, 201), "renamed this task to \"Launch post\"")
        let n = task(d, 201)["activity"]!.array!.count
        Edits.rename(&d, task: 201, to: "Launch post v2", me: 1, ts: 1060)
        XCTAssertEqual(task(d, 201)["activity"]!.array!.count, n, "my recent line is updated")
        XCTAssertEqual(lastLine(d, 201), "renamed this task to \"Launch post v2\"")
        Edits.setDescription(&d, task: 201, to: "new", me: 1, ts: 1100)
        XCTAssertEqual(lastLine(d, 201), "updated the description")
    }
}

final class ShiftDatesTests: XCTestCase {
    func testDraggingABarMovesBothDatesAndKeepsTheTime() {
        var d = SampleWorkspace.make()
        let today = ISO8601DateFormatter().date(from: "2026-11-17T12:00:00Z")!
        Edits.setStart(&d, task: 205, to: "2026-11-17", me: 1, ts: 1, today: today)
        Edits.setDue(&d, task: 205, to: "2026-11-19", me: 1, ts: 1, today: today)
        Edits.setDueTime(&d, task: 205, to: "15:00", me: 1, ts: 1, today: today)
        XCTAssertTrue(Edits.shiftDates(&d, task: 205, days: 3, me: 1, ts: 2, today: today))
        let t = d["tasks"]!.array!.first { $0["id"] == 205 }!
        XCTAssertEqual(t["start"], "2026-11-20")
        XCTAssertEqual(t["due"], "2026-11-22")
        XCTAssertEqual(t["due_time"], "15:00")
        XCTAssertFalse(Edits.shiftDates(&d, task: 205, days: 0, me: 1, ts: 3, today: today))
    }
}

final class TaskAttachmentTests: XCTestCase {
    func testAPictureJoinsTheTaskOnceAndIsLogged() {
        var d = SampleWorkspace.make()
        let sha = String(repeating: "a", count: 64)
        let att = FileAPI.attachment(sha: sha, name: "shot.jpg", isVideo: false, bytes: 10, w: 2, h: 2, secs: 0, thumb: nil)
        XCTAssertTrue(Edits.addTaskAttachment(&d, task: 201, attachment: att, me: 1, ts: 5))
        XCTAssertFalse(Edits.addTaskAttachment(&d, task: 201, attachment: att, me: 1, ts: 6))
        let t = WorkspaceView(d).tasks.first { $0.id == 201 }!
        XCTAssertEqual(t.attachments.map(\.name), ["shot.jpg"])
        XCTAssertEqual(t.activity.last?.text, "attached shot.jpg")
        XCTAssertTrue(Edits.removeTaskAttachment(&d, task: 201, file: sha, ts: 7))
        XCTAssertTrue(WorkspaceView(d).tasks.first { $0.id == 201 }!.attachments.isEmpty)
    }
}
