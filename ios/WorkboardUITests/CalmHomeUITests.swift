// UI tests for the screens without a tab bar: everything opens from Home and
// "<" goes one step back. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class CalmHomeUITests: WorkboardUITestCase {
    func home() -> XCUIApplication {
        let a = launchFresh()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))
        XCTAssertFalse(a.tabBars.firstMatch.exists, "no tab bar")
        return a
    }

    func back(_ a: XCUIApplication) {
        a.navigationBars.buttons.element(boundBy: 0).tap()
    }

    func testHomeTodayTaskAndBack() {
        let a = home()
        a.buttons["place-today"].tap()
        let title = a.descendants(matching: .any)["screenTitle"].firstMatch
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        XCTAssertTrue(title.label.contains("Today"))
        let task = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'task-'")).firstMatch
        XCTAssertTrue(task.waitForExistence(timeout: 5), "Jamie has tasks due by today")
        task.tap()
        XCTAssertTrue(a.descendants(matching: .any)["taskName"].firstMatch.waitForExistence(timeout: 5))
        back(a)
        XCTAssertTrue(a.buttons["newTaskButton"].waitForExistence(timeout: 5), "back in Today")
        back(a)
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 5), "back on Home")
    }

    func testHomeChatConversation() {
        let a = home()
        a.buttons["place-chats"].tap()
        let general = a.buttons["channel-301"]
        XCTAssertTrue(general.waitForExistence(timeout: 5))
        general.tap()
        XCTAssertTrue(a.navigationBars["#general"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.textFields["composer"].waitForExistence(timeout: 5))
        back(a)
        XCTAssertTrue(a.buttons["newDMButton"].waitForExistence(timeout: 5), "back in Chat")
    }

    func testPlusMakesATaskDueToday() {
        let a = home()
        let plus = a.buttons["newItemButton"]
        waitEnabled(plus)
        plus.tap()
        let name = a.textFields["newItemName"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        let text = "From the plus \(Int.random(in: 1000...9999))"
        name.tap()
        name.typeText(text)
        a.buttons["newItemDue"].tap()
        a.buttons["Today"].firstMatch.tap()
        let save = a.buttons["newItemSave"]
        waitEnabled(save)
        save.tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 5), "the sheet closed")
        a.buttons["place-today"].tap()
        XCTAssertTrue(a.staticTexts[text].waitForExistence(timeout: 5), "it's in Today")
    }

    func testHidingToolsTakesItOffHome() {
        let a = home()
        XCTAssertTrue(a.buttons["place-tools"].exists)
        a.buttons["settingsButton"].tap()
        let tools = a.switches["showOnHome-tools"]
        XCTAssertTrue(tools.waitForExistence(timeout: 5))
        func flip() { tools.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap() }
        if (tools.value as? String) != "1" { flip() } // left off by an earlier run
        flip()
        wait(for: [expectation(for: NSPredicate(format: "value == '0'"), evaluatedWith: tools)], timeout: 5)
        a.buttons["Done"].tap()
        XCTAssertTrue(a.buttons["place-inbox"].waitForExistence(timeout: 5))
        XCTAssertFalse(a.buttons["place-tools"].exists, "Tools is off Home")
        // and back on
        a.buttons["settingsButton"].tap()
        XCTAssertTrue(tools.waitForExistence(timeout: 5))
        flip()
        wait(for: [expectation(for: NSPredicate(format: "value == '1'"), evaluatedWith: tools)], timeout: 5)
        a.buttons["Done"].tap()
        XCTAssertTrue(a.buttons["place-tools"].waitForExistence(timeout: 5))
    }

    func testMoreDetailsFoldAndUnfold() {
        let a = launchFresh(["-wbOpen", "task:203"])
        XCTAssertTrue(a.descendants(matching: .any)["taskName"].firstMatch.waitForExistence(timeout: 10))
        let toggle = a.buttons["taskDetailsToggle"]
        scrollTo(toggle, in: a)
        if toggle.value as? String == "Shown" { toggle.tap() }
        XCTAssertFalse(a.buttons["addTagButton"].exists, "folded")
        toggle.tap()
        // rows below the screen aren't made until they come into view
        let tag = a.buttons["addTagButton"]
        scrollTo(tag, in: a)
        XCTAssertTrue(tag.exists, "tags are in More details")
    }
}
