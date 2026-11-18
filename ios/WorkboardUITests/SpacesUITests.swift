// UI tests for making a space, a folder and a list, editing statuses, and
// archiving. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class SpacesUITests: WorkboardUITestCase {
    func name(_ text: String, in a: XCUIApplication, button: String) {
        let field = a.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        a.alerts.buttons[button].tap()
    }

    func testMakeASpaceWithAFolderAndListThenEditStatusesAndArchive() {
        let a = launchFresh(["-wbTab", "tasks"])
        let tag = "\(Int.random(in: 100000...999999))"

        let new = a.buttons["newSpaceButton"]
        waitEnabled(new)
        new.tap()
        name("Space \(tag)", in: a, button: "Create")
        let row = a.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Space \(tag)")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        // a new space has one list called List
        XCTAssertTrue(a.buttons.matching(NSPredicate(format: "label BEGINSWITH 'List'")).firstMatch.waitForExistence(timeout: 5))

        let menu = a.buttons["spaceMenu"]
        waitEnabled(menu)
        menu.tap()
        a.buttons["newFolderButton"].tap()
        name("Folder \(tag)", in: a, button: "Create")
        XCTAssertTrue(a.staticTexts["Folder \(tag)"].waitForExistence(timeout: 5))

        menu.tap()
        a.buttons["newListButton"].tap()
        name("Plans \(tag)", in: a, button: "Create")
        XCTAssertTrue(a.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Plans \(tag)")).firstMatch.waitForExistence(timeout: 5))

        // statuses: add one, rename it, delete it
        menu.tap()
        a.buttons["editStatusesButton"].tap()
        let add = a.buttons["addStatusButton"]
        waitEnabled(add)
        add.tap()
        let field = a.textFields["statusName-2"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        XCTAssertEqual(field.value as? String, "new status")
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 10) + "qa\n")
        let renamed = expectation(for: NSPredicate(format: "value == 'qa'"), evaluatedWith: field)
        wait(for: [renamed], timeout: 5)
        field.swipeLeft()
        a.buttons["Delete"].tap()
        let gone = expectation(for: NSPredicate(format: "value == 'complete'"), evaluatedWith: a.textFields["statusName-2"])
        wait(for: [gone], timeout: 5)

        // archive the space: it leaves the Tasks tab
        a.navigationBars.buttons.element(boundBy: 0).tap()
        waitEnabled(menu)
        menu.tap()
        a.buttons["Archive space"].tap()
        let left = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: row)
        wait(for: [left], timeout: 5)
    }
}
