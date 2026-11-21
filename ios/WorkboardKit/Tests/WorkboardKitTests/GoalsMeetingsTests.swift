import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class GoalsMeetingsTests: XCTestCase {
    func testGoalProgressMixesTargetKindsLikeTheMac() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addGoal(&d, name: "Launch", me: 1, id: 800))
        Edits.addTarget(&d, goal: 800, GoalTargetView(name: "Signups", kind: .number(start: 0, target: 100, current: 50, unit: "")))
        Edits.addTarget(&d, goal: 800, GoalTargetView(name: "Press", kind: .trueFalse(done: true)))
        Edits.addTarget(&d, goal: 800, GoalTargetView(name: "Work", kind: .tasks(ids: [201, 206, 99999])))
        let v = WorkspaceView(d)
        let g = v.goals(d).first!
        XCTAssertEqual(g.targets.count, 3)
        // 0.5, 1, and one of two live tasks closed (206 complete): 0.5
        XCTAssertEqual(v.progress(of: g), (0.5 + 1 + 0.5) / 3, accuracy: 0.0001)
        // written the way serde writes TargetKind
        XCTAssertNotNil(d.at("extra.goals")![0]!["targets"]![1]!["kind"]!["TrueFalse"])
        XCTAssertTrue(Edits.setTarget(&d, goal: 800, index: 1, to: GoalTargetView(name: "Press", kind: .trueFalse(done: false))))
        XCTAssertTrue(Edits.removeTarget(&d, goal: 800, index: 2))
        XCTAssertTrue(Edits.setGoalDue(&d, goal: 800, to: "2026-12-01"))
        XCTAssertTrue(Edits.renameGoal(&d, goal: 800, to: "Ship"))
        let g2 = WorkspaceView(d).goals(d).first!
        XCTAssertEqual(g2.name, "Ship")
        XCTAssertEqual(g2.due, "2026-12-01")
        XCTAssertEqual(WorkspaceView(d).progress(of: g2), 0.25, accuracy: 0.0001)
        XCTAssertTrue(Edits.deleteGoal(&d, goal: 800))
        XCTAssertEqual(d.at("extra.goals"), [])
    }

    func testMeetingActionItemsBecomeTasksForTheAttendees() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addMeeting(&d, title: "Sync", date: "2026-11-18", me: 1, id: 810))
        Edits.toggleAttendee(&d, meeting: 810, member: 2)
        Edits.updateMeeting(&d, meeting: 810, notes: "Talked.\n[ ] Write recap\n[ ]   \n[x] old\n[ ] Book room")
        var next: UInt64 = 9000
        let n = Edits.meetingActionsToTasks(&d, meeting: 810, list: 120, me: 1, ts: 5) { next += 1; return next }
        XCTAssertEqual(n, 2)
        let made = d["tasks"]!.array!.filter { ($0["id"]?.uint ?? 0) > 9000 }
        XCTAssertEqual(made.map { $0["name"]! }, ["Write recap", "Book room"])
        XCTAssertEqual(made[0]["assignees"], [1, 2])
        let m = WorkspaceView(d).meetings(d).first!
        XCTAssertTrue(m.actionItems.isEmpty, "ticked off")
        XCTAssertTrue(Edits.deleteMeeting(&d, meeting: 810))
    }
}

final class ReminderTests: XCTestCase {
    func testRemindersArePersonalAndSorted() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addReminder(&d, text: "Later", dueTs: 200, me: 1, id: 1))
        XCTAssertTrue(Edits.addReminder(&d, text: "Soon", dueTs: 100, task: 201, me: 1, id: 2))
        XCTAssertTrue(Edits.addReminder(&d, text: "Alex's", dueTs: 50, me: 2, id: 3))
        XCTAssertFalse(Edits.addReminder(&d, text: " ", dueTs: 50, me: 1, id: 4))
        let mine = WorkspaceView(d).reminders(d, me: 1)
        XCTAssertEqual(mine.map(\.text), ["Soon", "Later"])
        XCTAssertEqual(mine[0].task, 201)
        Edits.setReminderDone(&d, reminder: 2, true)
        XCTAssertTrue(WorkspaceView(d).reminders(d, me: 1)[0].done)
        Edits.deleteReminder(&d, reminder: 2)
        XCTAssertEqual(WorkspaceView(d).reminders(d, me: 1).count, 1)
        XCTAssertFalse(Sync.shared.contains("extra.reminders"), "each person's own")
        Edits.setNotepad(&d, to: "jot")
        XCTAssertEqual(d.at("extra.notepad"), "jot")
    }
}

final class PersonalDataMergeTests: XCTestCase {
    func testMyRemindersNotepadAndInboxSurviveAMergeWithTheServer() {
        var phone = SampleWorkspace.make()
        var meta = SyncMeta(of: phone)
        Sync.stamp(phone, &meta, now: 10)
        phone.set("extra.sync", meta.json)
        var server = phone
        Edits.addReminder(&phone, text: "Call", dueTs: 100, me: 1, id: 5)
        Edits.setNotepad(&phone, to: "jot")
        Inbox.move(&phone, keys: ["x"], to: .later)
        Edits.sendMessage(&server, channel: 301, text: "from a Mac", me: 2, id: 9100, ts: 20)
        var sm = SyncMeta(of: server)
        Sync.stamp(server, &sm, now: 20)
        server.set("extra.sync", sm.json)
        Sync.merge(&phone, [server])
        XCTAssertEqual(WorkspaceView(phone).reminders(phone, me: 1).map(\.text), ["Call"])
        XCTAssertEqual(phone.at("extra.notepad"), "jot")
        XCTAssertEqual(phone.at("extra.inbox_later"), ["x"])
        XCTAssertTrue(WorkspaceView(phone).channels[0].messages.contains { $0.text == "from a Mac" })
    }
}

final class TimesheetTests: XCTestCase {
    func testMyWeekAndEveryonesTotals() {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        var d = SampleWorkspace.make()
        // Tuesday 2026-11-17 12:00 UTC; the week starts Sunday the 15th
        let tue: Int64 = 1_794_916_800
        let start = Timesheet.weekStart(of: Date(timeIntervalSince1970: TimeInterval(tue)), calendar: cal)
        XCTAssertEqual(cal.component(.weekday, from: start), 1)
        Edits.addTimeEntry(&d, task: 201, minutes: 60, yesterday: false, me: 1, ts: tue)
        Edits.addTimeEntry(&d, task: 201, minutes: 30, yesterday: true, me: 1, ts: tue)
        Edits.addTimeEntry(&d, task: 203, minutes: 45, yesterday: false, me: 2, ts: tue)
        Edits.addTimeEntry(&d, task: 204, minutes: 15, yesterday: false, me: 1, ts: tue - 86400 * 7) // last week
        let v = WorkspaceView(d)
        let rows = Timesheet.mine(v, me: 1, weekOf: start, now: tue, calendar: cal)
        XCTAssertEqual(rows.map(\.task.id), [201])
        XCTAssertEqual(rows[0].days[2], 3600)
        XCTAssertEqual(rows[0].days[1], 1800)
        XCTAssertEqual(rows[0].total, 5400)
        let all = Timesheet.everyone(v, me: 1, weekOf: start, calendar: cal)
        XCTAssertEqual(all[1], 5400)
        XCTAssertEqual(all[2], 2700)
    }
}

final class DashboardTests: XCTestCase {
    func testTemplatesPickTheirCardsAndNumbersCountTheSpace() {
        var d = SampleWorkspace.make()
        Edits.addDashboard(&d, name: "Team", kind: "simple", space: nil, me: 1, id: 820)
        Edits.addDashboard(&d, name: "Time", kind: "time", space: 100, me: 1, id: 821)
        let v = WorkspaceView(d)
        let ds = v.dashboards(d, me: 1)
        XCTAssertEqual(ds[0].cards.map(\.kind), ["stats", "status", "priority", "overdue"])
        XCTAssertEqual(ds[1].cards.map(\.kind), ["time", "time_people"])
        let n = v.dashNumbers(space: 100, me: 1, now: nowTs())
        XCTAssertEqual(n.total, 6)
        XCTAssertEqual(n.open, 5)
        XCTAssertEqual(n.overdue.map(\.id), [202])
        XCTAssertEqual(n.byStatus.first { $0.0.name == "review" }?.1, 1)
        XCTAssertEqual(n.workload.first { $0.0.id == 1 }?.1, 3)
        XCTAssertTrue(Edits.renameDashboard(&d, dashboard: 820, to: "Ours"))
        XCTAssertTrue(Edits.deleteDashboard(&d, dashboard: 821))
        XCTAssertEqual(WorkspaceView(d).dashboards(d, me: 1).map(\.name), ["Ours"])
    }
}

final class FormTests: XCTestCase {
    func testSubmittingMakesATaskLikeTheMac() {
        var d = SampleWorkspace.make()
        Edits.addForm(&d, list: 120, me: 1, id: 830)
        Edits.setFormField(&d, form: 830, index: 0, to: FormFieldView(label: "Title", map: "name", required: true))
        Edits.setFormField(&d, form: 830, index: nil, to: FormFieldView(label: "Seats", map: "number"))
        Edits.setFormField(&d, form: 830, index: nil, to: FormFieldView(label: "Urgency", map: "priority", options: Edits.priorityLabels))
        XCTAssertEqual(Edits.submitForm(&d, form: 830, answers: [1: "Need it"], me: 2, id: 9000, ts: 5), .failure(.answer("Please answer \"Title\".")))
        XCTAssertEqual(Edits.submitForm(&d, form: 830, answers: [0: "Laptop", 2: "many"], me: 2, id: 9000, ts: 5), .failure(.answer("\"Seats\" needs a number.")))
        XCTAssertEqual(Edits.submitForm(&d, form: 830, answers: [0: "Laptop", 1: "Fast please", 2: "3", 3: "High"], me: 2, id: 9000, ts: 5), .success(9000))
        let t = d["tasks"]!.array!.first { $0["id"] == 9000 }!
        XCTAssertEqual(t["name"], "Laptop")
        XCTAssertEqual(t["list_id"], 120)
        XCTAssertEqual(t["priority"], 2)
        XCTAssertEqual(t["description"], "Fast please\nSeats: 3")
        XCTAssertEqual(t["activity"]!.array!.last!["text"], "submitted the form New Form")
        XCTAssertTrue(Edits.removeFormField(&d, form: 830, index: 2))
        XCTAssertTrue(Edits.deleteForm(&d, form: 830))
    }
}

final class WhiteboardTests: XCTestCase {
    func testStrokesAndNotesGetIdsAndMergeOneByOne() {
        var d = SampleWorkspace.make()
        Edits.addWhiteboard(&d, me: 1, id: 840)
        Edits.addStroke(&d, board: 840, points: [[1, 2], [3, 4]], color: [32, 32, 32], width: 2.5)
        Edits.addNote(&d, board: 840, at: [200, 200], color: [32, 32, 32])
        Sync.fillBoardIds(&d)
        var b = WorkspaceView(d).whiteboards(d)[0]
        XCTAssertEqual(b.name, "Whiteboard 1")
        XCTAssertNotEqual(b.strokes[0].id, 0)
        XCTAssertEqual(b.notes[0].pos, [120, 140])
        XCTAssertEqual(b.notes[0].color, [0xf8, 0xae, 0x00], "yellow when the pen is black")
        XCTAssertTrue(Edits.updateNote(&d, board: 840, note: b.notes[0].id, text: "idea", pos: [10, 10]))
        XCTAssertTrue(Edits.eraseStroke(&d, board: 840, stroke: b.strokes[0].id))
        b = WorkspaceView(d).whiteboards(d)[0]
        XCTAssertEqual(b.notes[0].text, "idea")
        XCTAssertTrue(b.strokes.isEmpty)
        XCTAssertTrue(Edits.deleteNote(&d, board: 840, note: b.notes[0].id))
        XCTAssertTrue(Edits.renameWhiteboard(&d, board: 840, to: "Plan"))
        XCTAssertTrue(Edits.deleteWhiteboard(&d, board: 840))
    }

    /// A board with the shapes and a line the Mac app lays out (a template).
    private func boardWithShapes() -> JSON {
        var d = SampleWorkspace.make()
        Edits.addWhiteboard(&d, me: 1, id: 840)
        Edits.addNote(&d, board: 840, at: [200, 200], color: [32, 32, 32])
        Sync.fillBoardIds(&d)
        var bs = d.at("extra.whiteboards")!.array!
        let box = { (id: UInt64, y: Double, text: String) -> JSON in
            ["id": .id(id), "kind": "rect", "pos": [.double(0), .double(y)], "size": [.double(160), .double(64)],
             "text": .string(text), "color": [.int(182), .int(96), .int(224)]]
        }
        bs[0]["shapes"] = [box(501, 0, "CEO"), box(502, 124, "CTO")]
        bs[0]["lines"] = [["id": .id(503), "from": .id(501), "to": .id(502), "a": [.double(0), .double(0)],
                           "b": [.double(0), .double(0)], "color": [.int(182), .int(96), .int(224)], "elbow": true]]
        d.set("extra.whiteboards", .array(bs))
        return d
    }

    func testNoteEditsKeepTheShapesAndClearTakesThemToo() {
        var d = boardWithShapes()
        let board = { (d: JSON) in d.at("extra.whiteboards")!.array![0] }
        XCTAssertEqual(WorkspaceView(d).whiteboards(d)[0].shapeCount, 2, "counted in the list")
        // editing and deleting a note here leaves the Mac app's shapes alone
        let note = WorkspaceView(d).whiteboards(d)[0].notes[0].id
        XCTAssertTrue(Edits.updateNote(&d, board: 840, note: note, text: "idea", pos: [10, 10]))
        XCTAssertTrue(Edits.deleteNote(&d, board: 840, note: note))
        XCTAssertEqual(board(d)["shapes"]?.array?.count, 2)
        XCTAssertEqual(board(d)["lines"]?.array?.count, 1)
        // Clear takes everything, as on the Mac: the shapes and lines stayed
        var meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: 1000)
        d.set("extra.sync", meta.json)
        let mac = d
        Edits.addNote(&d, board: 840, at: [50, 50], color: [8, 128, 234])
        XCTAssertTrue(Edits.clearWhiteboard(&d, board: 840))
        XCTAssertEqual(board(d)["notes"]?.array?.count, 0)
        XCTAssertNil(board(d)["shapes"], "left out of an empty board, like the Mac app does")
        XCTAssertNil(board(d)["lines"])
        XCTAssertEqual(WorkspaceView(d).whiteboards(d)[0].shapeCount, 0)
        meta = SyncMeta(of: d)
        Sync.stamp(d, &meta, now: 2000)
        d.set("extra.sync", meta.json)
        for id: UInt64 in [501, 502, 503] {
            XCTAssertNotNil(SyncMeta(of: d).tombs[id], "\(id) is marked deleted for the others")
        }
        // the Mac's copy from before the Clear doesn't bring them back
        Sync.merge(&d, [mac])
        XCTAssertNil(board(d)["shapes"]?.array?.first, "\(board(d))")
        XCTAssertNil(board(d)["lines"]?.array?.first)
    }
}

final class TemplateTests: XCTestCase {
    let templateRequired = ["id", "name", "task_name", "description", "checklist", "tags", "priority"]

    func testAListTemplateMakesASpaceWithItsStatusesAndTasks() {
        var d = SampleWorkspace.make()
        let bug = TemplateView.builtins.first { $0.id == "builtin:bug-tracker" }!
        var next: UInt64 = 7000
        let lid = Edits.useListTemplate(&d, bug, name: "", space: nil, me: 1, ts: 5) { next += 1; return next }!
        let v = WorkspaceView(d)
        XCTAssertEqual(v.list(lid)?.name, "Bug tracker")
        XCTAssertEqual(v.statuses(forList: lid).map(\.name), ["reported", "triaged", "in progress", "fixed", "closed"])
        let tasks = v.tasks(inList: lid)
        XCTAssertEqual(tasks.map(\.status), ["reported", "triaged"])
        XCTAssertEqual(tasks[0].checklist.map(\.text), ["Reproduce", "Find the cause", "Fix and test"])
        XCTAssertEqual(tasks[0].tags, ["bug"])
        XCTAssertEqual(tasks[0].priority, 2)
    }

    func testIntoAnExistingSpaceTheStatusesAreMerged() {
        var d = SampleWorkspace.make()
        let content = TemplateView.builtins.first { $0.id == "builtin:content-calendar" }!
        var next: UInt64 = 7000
        let lid = Edits.useListTemplate(&d, content, name: "Posts", space: 100, me: 1, ts: 5) { next += 1; return next }!
        let names = WorkspaceView(d).statuses(forList: lid).map(\.name)
        XCTAssertEqual(names.first, "to do", "the space keeps its own first status")
        XCTAssertTrue(names.contains("writing") && names.contains("published"))
        XCTAssertEqual(names.last, "complete")
        XCTAssertEqual(WorkspaceView(d).list(lid)?.name, "Posts")
    }

    func testTaskAndDocTemplatesAndSavingOnes() {
        var d = SampleWorkspace.make()
        let report = TemplateView.builtins.first { $0.id == "builtin:bug-report" }!
        XCTAssertTrue(Edits.useTaskTemplate(&d, report, name: "", list: 120, me: 1, id: 7100, ts: 5))
        let t = d["tasks"]!.array!.first { $0["id"] == 7100 }!
        XCTAssertEqual(t["name"], "New bug")
        XCTAssertEqual(t["checklist"]!.array!.count, 4)
        let notes = TemplateView.builtins.first { $0.id == "builtin:meeting-notes" }!
        XCTAssertTrue(Edits.useDocTemplate(&d, notes, title: "", space: 100, me: 1, id: 7200, ts: 5))
        let doc = d["docs"]!.array!.first { $0["id"] == 7200 }!
        XCTAssertEqual(doc["meeting_note"], true)
        XCTAssertTrue((doc["body"]?.string ?? "").contains("Action items"))
        XCTAssertTrue(Edits.saveListAsTemplate(&d, list: 112, id: 7300))
        XCTAssertTrue(Edits.saveDocAsTemplate(&d, doc: 401, id: 7301))
        let saved = d.at("extra.templates")!.array!
        for k in templateRequired { XCTAssertNotNil(saved[0][k], k); XCTAssertNotNil(saved[1][k], k) }
        let team = WorkspaceView(d).templates(d).filter { $0.teamId != nil }
        XCTAssertEqual(team.map(\.name), ["App", "Launch plan"])
        XCTAssertEqual(team[0].tasks.count, 2)
        XCTAssertTrue(Edits.deleteTemplate(&d, template: 7300))
    }
}

final class TeamTests: XCTestCase {
    func testTeamsAreMadeNamedAndFilled() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addTeam(&d, me: 1, id: 850))
        XCTAssertTrue(Edits.toggleTeamMember(&d, team: 850, member: 2))
        XCTAssertTrue(Edits.renameTeam(&d, team: 850, to: "Design"))
        let t = WorkspaceView(d).teams(d)[0]
        XCTAssertEqual(t.name, "Design")
        XCTAssertEqual(t.members, [1, 2])
        XCTAssertTrue(Sync.shared.contains("extra.teams"))
        XCTAssertTrue(Edits.deleteTeam(&d, team: 850))
    }
}
