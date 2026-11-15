// UI tests for round 2: editing, pinning and deleting my messages, making a
// channel, and deleting a task. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class MessageActionsUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func app(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args
        a.launch()
        return a
    }

    func testEditPinAndDeleteMyMessage() {
        let a = app(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "channel:302"])
        let box = a.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: box)
        wait(for: [enabled], timeout: 10)
        let tag = "\(Int.random(in: 100000...999999))"
        let text = "Typo heer \(tag)"
        box.tap()
        box.typeText(text)
        a.buttons["sendButton"].tap()
        let sent = a.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", text)).firstMatch
        XCTAssertTrue(sent.waitForExistence(timeout: 5))

        sent.press(forDuration: 1.0)
        let edit = a.buttons["Edit message"]
        XCTAssertTrue(edit.waitForExistence(timeout: 5))
        edit.tap()
        let field = a.textFields["editMessageField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(" fixed")
        a.buttons["saveEditButton"].tap()
        let edited = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@ AND label CONTAINS 'fixed' AND label CONTAINS '(edited)'", tag)).firstMatch
        XCTAssertTrue(edited.waitForExistence(timeout: 5), "the message shows (edited)")
        // press the row, not the selectable text
        let row = a.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'message-'")).containing(NSPredicate(format: "label CONTAINS %@", tag)).element(boundBy: 0)

        row.press(forDuration: 1.0)
        let pin = a.buttons["Pin to channel"]
        XCTAssertTrue(pin.waitForExistence(timeout: 5))
        pin.tap()
        XCTAssertTrue(a.images["Pinned"].firstMatch.waitForExistence(timeout: 5))

        row.press(forDuration: 1.0)
        let del = a.buttons["Delete message"]
        XCTAssertTrue(del.waitForExistence(timeout: 5))
        del.tap()
        a.buttons["confirmDeleteMessage"].firstMatch.tap()
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: edited)
        wait(for: [gone], timeout: 5)
    }

    func testCreateChannelAndDeleteTask() {
        let a = app(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbTab", "chat"])
        let new = a.buttons["newChannelButton"]
        XCTAssertTrue(new.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: new)
        wait(for: [enabled], timeout: 10)
        new.tap()
        let name = "ui test \(Int.random(in: 1000...9999))"
        let field = a.textFields["channelNameField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(name)
        a.buttons["createChannelButton"].tap()
        XCTAssertTrue(a.navigationBars["#" + name.replacingOccurrences(of: " ", with: "-")].waitForExistence(timeout: 5))
        a.terminate()

        // a task made here, then deleted
        let b = app(["-wbServer", open, "-wbOpen", "list:100:120"])
        let add = b.buttons["addTaskButton"]
        if add.waitForExistence(timeout: 10) {
            add.tap()
            let tf = b.textFields["newTaskField"]
            XCTAssertTrue(tf.waitForExistence(timeout: 5))
            tf.typeText("Delete me\n")
            let row = b.staticTexts["Delete me"].firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 5))
            row.tap()
            let more = b.buttons["taskMoreMenu"]
            XCTAssertTrue(more.waitForExistence(timeout: 5))
            more.tap()
            b.buttons["deleteTaskButton"].tap()
            b.buttons["confirmDeleteTask"].firstMatch.tap()
            XCTAssertTrue(b.navigationBars["Backlog"].waitForExistence(timeout: 5))
        }
    }
}
