// UI tests for the task fields the Mac app has: tags, the time estimate, the
// timer and time entries, relationships, repeat, and the task menu's
// Duplicate and Archive. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class TaskFieldsUITests: WorkboardUITestCase {
    func openTask(_ id: Int) -> XCUIApplication {
        let a = launchFresh(["-wbOpen", "task:\(id)"])
        XCTAssertTrue(a.descendants(matching: .any)["taskName"].firstMatch.waitForExistence(timeout: 10))
        waitEnabled(a.buttons["taskMoreMenu"])
        return a
    }

    func testTagEstimateAndRepeat() {
        let a = openTask(205)
        let tag = "t\(Int.random(in: 100000...999999))"

        let addTag = a.buttons["addTagButton"]
        scrollTo(addTag, in: a)
        addTag.tap()
        let field = a.textFields["newTagField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("\(tag)\n")
        let chip = a.buttons["tag-\(tag)"]
        scrollTo(chip, in: a)

        let est = a.textFields["estimateField"]
        scrollTo(est, in: a)
        // tap at its end, so deleting takes what an earlier run left there
        est.coordinate(withNormalizedOffset: CGVector(dx: 0.97, dy: 0.5)).tap()
        if let old = est.value as? String, !old.isEmpty, old != "e.g. 1h 30m" {
            est.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count + 2))
        }
        est.typeText("90m\n")
        let shown = expectation(for: NSPredicate(format: "value == '1h 30m'"), evaluatedWith: est)
        wait(for: [shown], timeout: 5)

        let rep = a.buttons["repeatMenu"]
        scrollTo(rep, in: a)
        rep.tap()
        XCTAssertTrue(a.buttons["Weekly"].waitForExistence(timeout: 5))
        a.buttons["Weekly"].tap()
        let weekly = expectation(for: NSPredicate(format: "label CONTAINS 'Weekly'"), evaluatedWith: rep)
        wait(for: [weekly], timeout: 5)
        // a repeating task gets a due date, so its time can be set
        XCTAssertTrue(a.buttons["dueTimeButton"].waitForExistence(timeout: 5))
        rep.tap()
        XCTAssertTrue(a.buttons["Never"].waitForExistence(timeout: 5))
        a.buttons["Never"].tap()
    }

    func testTimerAndAddedTime() {
        let a = openTask(204)
        let timer = a.buttons["timerButton"]
        scrollTo(timer, in: a)
        if timer.label.contains("Stop") { timer.tap() } // left running by an earlier run
        let total = a.staticTexts["trackedTotal"]
        let before = total.label
        timer.tap()
        XCTAssertTrue(a.buttons["Stop timer"].waitForExistence(timeout: 5))
        sleep(2)
        a.buttons["Stop timer"].tap()
        XCTAssertTrue(a.buttons["Start timer"].waitForExistence(timeout: 5))
        XCTAssertNotEqual(total.label, before, "the timer's time is kept")

        let add = a.buttons["addTimeButton"]
        scrollTo(add, in: a)
        add.tap()
        let field = a.textFields["addTimeField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("45m")
        a.buttons["addTimeConfirm"].tap()
        XCTAssertTrue(a.staticTexts["45m"].firstMatch.waitForExistence(timeout: 5), "the entry shows")
    }

    func testLinkATaskAndOpenIt() {
        let a = openTask(203)
        let menu = a.buttons["addRelationMenu"]
        scrollTo(menu, in: a)
        menu.tap()
        let linked = a.buttons["Linked"]
        XCTAssertTrue(linked.waitForExistence(timeout: 5))
        linked.tap()
        let pick = a.buttons["pick-206"]
        XCTAssertTrue(pick.waitForExistence(timeout: 5))
        pick.tap()
        let row = a.buttons["relation-Linked-206"]
        scrollTo(row, in: a)
        row.tap()
        let name = a.descendants(matching: .any)["taskName"].firstMatch
        let other = expectation(for: NSPredicate(format: "value == 'Old onboarding flow'"), evaluatedWith: name)
        wait(for: [other], timeout: 5)
        // the other side shows the link too
        let back = a.buttons["relation-Linked-203"]
        scrollTo(back, in: a)
        back.swipeLeft()
        a.buttons["Remove"].tap()
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: back)
        wait(for: [gone], timeout: 5)
    }

    func testDuplicateThenArchiveTheCopy() {
        let a = launchFresh(["-wbOpen", "list:100:120"])
        let original = a.buttons["task-205"]
        waitEnabled(a.buttons["addTaskButton"])
        XCTAssertTrue(original.waitForExistence(timeout: 10))
        original.tap()
        waitEnabled(a.buttons["taskMoreMenu"])
        a.buttons["taskMoreMenu"].tap()
        a.buttons["duplicateTaskButton"].tap()
        a.navigationBars.buttons.element(boundBy: 0).tap()

        let copy = a.buttons.matching(NSPredicate(format: "label CONTAINS 'Dark mode for settings (copy)'")).firstMatch
        XCTAssertTrue(copy.waitForExistence(timeout: 5), "the copy is in the list")
        let copies = a.buttons.matching(NSPredicate(format: "label CONTAINS 'Dark mode for settings (copy)'")).count
        copy.tap()
        waitEnabled(a.buttons["taskMoreMenu"])
        a.buttons["taskMoreMenu"].tap()
        a.buttons["archiveTaskButton"].tap()
        XCTAssertTrue(a.buttons["addTaskButton"].waitForExistence(timeout: 5), "back in the list")
        let fewer = expectation(for: NSPredicate(format: "count == %d", copies - 1),
                                evaluatedWith: a.buttons.matching(NSPredicate(format: "label CONTAINS 'Dark mode for settings (copy)'")))
        wait(for: [fewer], timeout: 5)
    }
}
