// UI tests for a list's Board and Calendar views. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class ListModesUITests: WorkboardUITestCase {
    func openList() -> XCUIApplication {
        let a = launchFresh(["-wbOpen", "list:100:112"])
        let menu = a.buttons["viewModeMenu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        return a
    }

    func pick(_ mode: String, in a: XCUIApplication) {
        a.buttons["viewModeMenu"].tap()
        let item = a.buttons[mode]
        XCTAssertTrue(item.waitForExistence(timeout: 5))
        item.tap()
    }

    func day(_ offset: Int) -> String {
        let f = DateFormatter()
        f.calendar = Calendar(identifier: .gregorian)
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        return f.string(from: Date().addingTimeInterval(Double(offset) * 86400))
    }

    func testBoardMovesACardAndAddsToAColumn() {
        let a = openList()
        pick("Board", in: a)
        defer { pick("List", in: a) }
        // a card of its own in the first column (other tests move the sample tasks)
        let tag = "\(Int.random(in: 100000...999999))"
        let add = a.buttons["boardAdd-to do"]
        XCTAssertTrue(add.waitForExistence(timeout: 10))
        add.tap()
        let field = a.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("Board \(tag)")
        a.alerts.buttons["Add"].tap()
        let card = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'card-' AND label CONTAINS %@", "Board \(tag)")).firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 5))
        let id = card.identifier.replacingOccurrences(of: "card-", with: "")

        a.buttons["cardMenu-\(id)"].tap()
        let inProgress = a.buttons["IN PROGRESS"]
        XCTAssertTrue(inProgress.waitForExistence(timeout: 5))
        inProgress.tap()
        card.tap()
        let status = a.buttons["statusMenu"]
        XCTAssertTrue(status.waitForExistence(timeout: 5))
        let moved = expectation(for: NSPredicate(format: "label CONTAINS 'IN PROGRESS'"), evaluatedWith: status)
        wait(for: [moved], timeout: 5)
        a.navigationBars.buttons.element(boundBy: 0).tap()
    }

    func testCalendarShowsADaysTasksAndAddsOne() {
        let a = openList()
        pick("Calendar", in: a)
        defer { pick("List", in: a) }
        let tomorrow = a.descendants(matching: .any)["day-\(day(1))"].firstMatch
        if !tomorrow.waitForExistence(timeout: 5) {
            a.buttons["nextMonth"].tap() // today is the month's last day
        }
        XCTAssertTrue(tomorrow.waitForExistence(timeout: 5))
        tomorrow.tap()
        XCTAssertTrue(a.staticTexts["Tomorrow"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.buttons["task-203"].waitForExistence(timeout: 5), "Fix login crash is due tomorrow")

        let tag = "\(Int.random(in: 100000...999999))"
        let field = a.textFields["calendarNewTaskField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("Cal \(tag)\n")
        let row = a.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Cal \(tag)")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5), "the new task is on the chosen day")
    }

    func testTableGanttAndTimelineShowTheListsTasks() {
        let a = openList()
        defer { pick("List", in: a) }
        pick("Table", in: a)
        XCTAssertTrue(a.buttons["tableRow-203"].waitForExistence(timeout: 10), "Fix login crash is a row")
        pick("Gantt", in: a)
        let bar = a.buttons["bar-203"]
        XCTAssertTrue(bar.waitForExistence(timeout: 10), "a dated task is a bar")
        pick("Timeline", in: a)
        XCTAssertTrue(a.staticTexts["By assignee"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.buttons["bar-203"].waitForExistence(timeout: 5))
        a.buttons["bar-203"].tap()
        XCTAssertTrue(a.buttons["statusMenu"].waitForExistence(timeout: 5), "a bar opens its task")
        a.navigationBars.buttons.element(boundBy: 0).tap()
    }
}
