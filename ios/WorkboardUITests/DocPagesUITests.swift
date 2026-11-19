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

    func testFormattingBarAddsMarkdownAndReturnContinuesTheList() {
        let a = launchFresh(["-wbOpen", "doc:402"])
        let edit = a.buttons["docEdit"]
        waitEnabled(edit)
        edit.tap()
        let editor = a.textViews["docEditor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        // the cursor starts at the end; two Returns leave any list there
        // (the second ends an empty item) and start a plain line
        editor.typeText("\n\n")
        let check = a.buttons["fmt-checklist"]
        XCTAssertTrue(check.waitForExistence(timeout: 5), "the formatting bar is above the keyboard")
        check.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        editor.typeText("first \(tag)\nsecond")
        a.buttons["fmt-bold"].tap()
        editor.typeText("loud")
        let text = editor.value as? String ?? ""
        XCTAssertTrue(text.contains("- [ ] first \(tag)\n- [ ] second**loud**"), text)
        a.buttons["docDone"].tap()
        let body = a.staticTexts["docBody"]
        XCTAssertTrue(body.waitForExistence(timeout: 5))
        XCTAssertTrue(body.label.contains("first \(tag)"))
    }
}
