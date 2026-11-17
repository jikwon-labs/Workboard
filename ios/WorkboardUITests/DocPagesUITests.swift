// UI tests for a doc's sub pages and Markdown. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class DocPagesUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func testAddASubPageAndFindItUnderItsDoc() {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "doc:402"] + testArgs
        a.launch()
        let more = a.buttons["docMoreMenu"]
        XCTAssertTrue(more.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: more)
        wait(for: [enabled], timeout: 10)
        more.tap()
        a.buttons["addPageButton"].tap()
        // the new page opens for writing
        let title = a.textFields["docTitleField"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        let tag = "\(Int.random(in: 100000...999999))"
        let editor = a.textViews["docEditor"]
        editor.tap()
        editor.typeText("# Heading \(tag)\n- [x] done item")
        a.buttons["docDone"].tap()
        let body = a.staticTexts["docBody"]
        XCTAssertTrue(body.waitForExistence(timeout: 5))
        XCTAssertTrue(body.label.contains("Heading \(tag)"))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        // listed under its doc, not on its own in Docs
        XCTAssertTrue(a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'page-'")).firstMatch.waitForExistence(timeout: 5))
    }
}
