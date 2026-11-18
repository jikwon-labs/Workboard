// UI tests for starting a direct message, adding a subtask and using a task's
// checklist. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class NewItemsUITests: WorkboardUITestCase {
    func testStartADirectMessageAndOpenTheOneThereIs() {
        let a = launchFresh(["-wbTab", "chat"])
        let new = a.buttons["newDMButton"]
        waitEnabled(new)
        new.tap()
        let sam = a.buttons["dmPerson-3"]
        XCTAssertTrue(sam.waitForExistence(timeout: 5))
        XCTAssertFalse(a.buttons["dmPerson-1"].exists, "I'm not in the list")
        sam.tap()
        XCTAssertTrue(a.navigationBars["Sam Lee"].waitForExistence(timeout: 5), "the conversation with Sam opens")
        let box = a.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 5))
        let tag = "\(Int.random(in: 100000...999999))"
        box.tap()
        box.typeText("Hi Sam \(tag)")
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", tag)).firstMatch.waitForExistence(timeout: 5))

        a.navigationBars.buttons.element(boundBy: 0).tap()
        waitEnabled(new)
        new.tap()
        a.buttons["dmPerson-3"].tap()
        XCTAssertTrue(a.navigationBars["Sam Lee"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", tag)).firstMatch.waitForExistence(timeout: 5),
                      "the same conversation again, not a new empty one")
    }

    func testAddASubtaskAndUseTheChecklist() {
        let a = launchFresh(["-wbOpen", "task:204"])
        let name = a.descendants(matching: .any)["taskName"].firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 10))
        let tag = "\(Int.random(in: 100000...999999))"

        let sub = a.textFields["newSubtaskField"]
        scrollTo(sub, in: a)
        waitEnabled(sub)
        sub.tap()
        sub.typeText("Sub \(tag)\n")
        let row = a.staticTexts["Sub \(tag)"]
        XCTAssertTrue(row.waitForExistence(timeout: 5), "the subtask shows under the task")
        XCTAssertEqual((sub.value as? String) ?? "", "Add subtask", "the field is empty again for the next one")

        let check = a.textFields["newCheckItemField"]
        scrollTo(check, in: a)
        check.tap()
        check.typeText("Item \(tag)\n")
        let item = a.buttons["Item \(tag)"]
        scrollTo(item, in: a)
        XCTAssertEqual(item.value as? String, "Not done")
        item.tap()
        let done = expectation(for: NSPredicate(format: "value == 'Done'"), evaluatedWith: item)
        wait(for: [done], timeout: 5)
        XCTAssertTrue(a.staticTexts["checklistCount"].exists)

        item.swipeLeft()
        let del = a.buttons["Delete"]
        XCTAssertTrue(del.waitForExistence(timeout: 5))
        del.tap()
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: item)
        wait(for: [gone], timeout: 5)

        // the subtask opens as a task of its own
        scrollTo(row, in: a)
        row.tap()
        let subName = a.descendants(matching: .any)["taskName"].firstMatch
        let named = expectation(for: NSPredicate(format: "value == %@", "Sub \(tag)"), evaluatedWith: subName)
        wait(for: [named], timeout: 5)
    }
}
