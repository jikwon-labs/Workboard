// UI tests for a doc's sub pages and Markdown. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class DocPagesUITests: WorkboardUITestCase {
    func testAddASubPageAndFindItUnderItsDoc() {
        let a = launchFresh(["-wbOpen", "doc:402"])
        let more = a.buttons["docMoreMenu"]
        waitEnabled(more)
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
