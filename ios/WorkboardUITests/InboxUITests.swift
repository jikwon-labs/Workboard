// UI tests for the Home tab's Inbox and search. Needs the fake server with
// test-only HTTP:
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class InboxUITests: WorkboardUITestCase {
    func home() -> XCUIApplication {
        let a = launchFresh(["-wbTab", "home"], server: adminServer)
        XCTAssertTrue(a.navigationBars["Home"].waitForExistence(timeout: 10))
        return a
    }

    func testAMentionShowsInTheInboxAndIsCleared() {
        let a = home()
        let tag = "\(Int.random(in: 100000...999999))"
        say("@Jamie Park please check \(tag)", channel: 302)
        let row = a.buttons.matching(NSPredicate(format: "label CONTAINS %@", tag)).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10), "the mention is in Primary")
        XCTAssertTrue(row.label.contains("mentioned you"))
        row.swipeLeft()
        a.buttons["Clear"].tap()
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: row)
        wait(for: [gone], timeout: 5)
        a.segmentedControls["inboxTabs"].buttons["Cleared"].tap()
        XCTAssertTrue(a.buttons.matching(NSPredicate(format: "label CONTAINS %@", tag)).firstMatch.waitForExistence(timeout: 5), "it's under Cleared")
        a.segmentedControls["inboxTabs"].buttons.element(boundBy: 0).tap()
    }

    func testSearchFindsTasksDocsAndMessages() {
        let a = home()
        let field = a.searchFields.firstMatch
        if !field.waitForExistence(timeout: 3) { a.swipeDown() }
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("launch")
        XCTAssertTrue(a.buttons["result-task-201"].waitForExistence(timeout: 5), "Write the launch post")
        XCTAssertTrue(a.buttons["result-doc-401"].waitForExistence(timeout: 5), "Launch plan")
        a.buttons["result-doc-401"].tap()
        XCTAssertTrue(a.navigationBars.element.waitForExistence(timeout: 5))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 6) + "zzqqxx")
        XCTAssertTrue(a.staticTexts["noResults"].waitForExistence(timeout: 5))
    }
}
