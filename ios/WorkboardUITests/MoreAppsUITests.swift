// UI tests for the More tab's apps: Goals and Meetings. Needs the open fake
// server:
//   swift run wb-fake-server --port 18787

import XCTest

final class MoreAppsUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func launch(_ app: String) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "app:\(app)"] + testArgs
        a.launch()
        return a
    }

    func waitEnabled(_ e: XCUIElement) {
        XCTAssertTrue(e.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: e)
        wait(for: [enabled], timeout: 10)
    }

    func testAGoalWithAYesNoTargetGoesToAHundred() {
        let a = launch("goals")
        let new = a.buttons["newGoalButton"]
        waitEnabled(new)
        new.tap()
        let progress = a.staticTexts["goalProgress"]
        XCTAssertTrue(progress.waitForExistence(timeout: 5))
        XCTAssertTrue(progress.label.hasSuffix(" 0%") || progress.label == "0%", progress.label)
        a.buttons["addTargetButton"].tap()
        let name = a.textFields["targetName"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Press release")
        a.buttons["Yes or no"].tap()
        a.buttons["targetAdd"].tap()
        let done = a.switches["targetDone-0"]
        XCTAssertTrue(done.waitForExistence(timeout: 5))
        done.switches.firstMatch.tap()
        let full = expectation(for: NSPredicate(format: "label ENDSWITH '100%'"), evaluatedWith: progress)
        wait(for: [full], timeout: 5)
        // clean up
        a.buttons["goalMoreMenu"].tap()
        a.buttons["Delete goal"].tap()
    }

    func testAMeetingsActionItemsBecomeTasks() {
        let a = launch("meetings")
        let new = a.buttons["newMeetingButton"]
        waitEnabled(new)
        new.tap()
        let notes = a.textViews["meetingNotes"].exists ? a.textViews["meetingNotes"] : a.textFields["meetingNotes"]
        XCTAssertTrue(notes.waitForExistence(timeout: 5))
        notes.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        notes.typeText("Agreed on it\n[ ] Recap \(tag)")
        let make = a.buttons["meetingMakeTasks"]
        XCTAssertTrue(make.waitForExistence(timeout: 5))
        make.tap()
        let list = a.buttons["pickList-120"]
        XCTAssertTrue(list.waitForExistence(timeout: 5))
        list.tap()
        XCTAssertTrue(a.staticTexts["Made 1 task in Backlog."].waitForExistence(timeout: 5))
        XCTAssertFalse(make.exists, "the action item was ticked off")
    }

    func testAReminderIsAddedTickedAndTheNotepadKeepsText() {
        let a = launch("reminders")
        let new = a.buttons["newReminderButton"]
        waitEnabled(new)
        new.tap()
        let text = a.textFields["reminderText"]
        XCTAssertTrue(text.waitForExistence(timeout: 5))
        text.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        text.typeText("Call back \(tag)")
        a.buttons["reminderAdd"].tap()
        // the permission prompt may come up once
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]
        if allow.waitForExistence(timeout: 3) { allow.tap() }
        let row = a.staticTexts["Call back \(tag)"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        let done = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'reminderDone-'")).firstMatch
        done.tap()
        XCTAssertTrue(a.staticTexts["Done"].waitForExistence(timeout: 5), "it moves under Done")

        a.navigationBars.buttons.element(boundBy: 0).tap()
        a.buttons["app-notepad"].tap()
        let pad = a.textViews["notepad"]
        XCTAssertTrue(pad.waitForExistence(timeout: 5))
        pad.tap()
        pad.typeText(" jot \(tag)")
        a.navigationBars.buttons.element(boundBy: 0).tap()
        a.buttons["app-notepad"].tap()
        XCTAssertTrue((a.textViews["notepad"].value as? String ?? "").contains("jot \(tag)"))
    }

    func testTimeAddedFromTheTimesheetShowsInMyWeekAndEveryones() {
        let a = launch("timesheets")
        let add = a.buttons["sheetAddTime"]
        waitEnabled(add)
        add.tap()
        let pick = a.buttons["sheetPick-205"]
        XCTAssertTrue(pick.waitForExistence(timeout: 5))
        pick.tap()
        let field = a.textFields["addTimeField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("30m")
        a.buttons["addTimeConfirm"].tap()
        XCTAssertTrue(a.buttons["sheetRow-205"].waitForExistence(timeout: 5))
        a.segmentedControls["timesheetTabs"].buttons["All timesheets"].tap()
        let me = a.descendants(matching: .any)["sheetPerson-1"].firstMatch
        XCTAssertTrue(me.waitForExistence(timeout: 5))
        XCTAssertFalse(me.label.hasSuffix("-"), "my time is counted")
    }

    func testTheWorkspaceOverviewAndANewDashboardShowCards() {
        let a = launch("dashboards")
        let overview = a.buttons["dashboard-overview"]
        XCTAssertTrue(overview.waitForExistence(timeout: 10))
        overview.tap()
        XCTAssertTrue(a.staticTexts["Task totals"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.staticTexts["Overdue tasks"].exists || a.swipeUpAndFind(a.staticTexts["Overdue tasks"]))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        let new = a.buttons["newDashboardButton"]
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: new)
        wait(for: [enabled], timeout: 10)
        new.tap()
        a.buttons["Time Tracking"].tap()
        let made = a.buttons.matching(NSPredicate(format: "label CONTAINS 'Time Tracking'")).firstMatch
        XCTAssertTrue(made.waitForExistence(timeout: 5))
        made.tap()
        XCTAssertTrue(a.staticTexts["Time tracked this week"].waitForExistence(timeout: 5))
        XCTAssertFalse(a.staticTexts["Task totals"].exists, "the time template has only time cards")
    }

    func testFillingAFormMakesATask() {
        let a = launch("forms")
        let new = a.buttons["newFormButton"]
        waitEnabled(new)
        let before = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'form-'")).count
        new.tap()
        let rows = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'form-'"))
        let more = expectation(for: NSPredicate(format: "count == %d", before + 1), evaluatedWith: rows)
        wait(for: [more], timeout: 5)
        rows.element(boundBy: before).tap()
        let title = a.textFields["formAnswer-0"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        title.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        title.typeText("From the form \(tag)")
        a.buttons["formSubmit"].tap()
        let msg = a.staticTexts["formMessage"]
        XCTAssertTrue(msg.waitForExistence(timeout: 5))
        XCTAssertTrue(msg.label.hasPrefix("Submitted."), msg.label)
    }

    func testDrawAStrokeAndAddANoteOnAWhiteboard() {
        let a = launch("whiteboards")
        let new = a.buttons["newBoardButton"]
        waitEnabled(new)
        let rows = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'board-'"))
        let before = rows.count
        new.tap()
        let more = expectation(for: NSPredicate(format: "count == %d", before + 1), evaluatedWith: rows)
        wait(for: [more], timeout: 5)
        let board = rows.element(boundBy: before)
        board.tap()
        let canvas = a.windows.firstMatch
        let from = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.4))
        let to = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5))
        from.press(forDuration: 0.1, thenDragTo: to)
        a.segmentedControls["boardTool"].buttons["Sticky note"].tap()
        canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.7)).tap()
        let note = a.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'note-'")).firstMatch
        XCTAssertTrue(note.waitForExistence(timeout: 5))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(board.waitForExistence(timeout: 5))
        XCTAssertTrue(board.label.contains("1 strokes") && board.label.contains("1 notes"), board.label)
    }

    func testATaskTemplateMakesATaskWithItsChecklist() {
        let a = launch("templates")
        let tp = a.buttons["template-builtin:bug-report"]
        waitEnabled(tp)
        tp.tap()
        let name = a.textFields["templateName"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        name.typeText("Crash \(tag)")
        a.buttons["templateUse"].tap()
        let title = a.descendants(matching: .any)["taskName"].firstMatch
        let named = expectation(for: NSPredicate(format: "value == %@", "Crash \(tag)"), evaluatedWith: title)
        wait(for: [named], timeout: 10)
        let count = a.staticTexts["checklistCount"]
        for _ in 0..<8 where !count.exists { a.swipeUp() }
        XCTAssertEqual(count.label, "0/4")
    }
}

extension XCUIApplication {
    func swipeUpAndFind(_ e: XCUIElement) -> Bool {
        for _ in 0..<6 where !e.exists { swipeUp() }
        return e.exists
    }
}
