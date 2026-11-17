// UI tests for round 4: picking someone from the @ list, and Trash leaving
// with the person who signs out.
// Needs the fake servers from docs/ios-design.md:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev

import XCTest

final class Round4UITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func launch(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args + testArgs
        a.launch()
        return a
    }

    func testMentionPickedFromTheList() {
        let a = launch(["-wbReset", "YES", "-wbServer", "ws://127.0.0.1:18787", "-wbMe", "1", "-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: box)
        wait(for: [enabled], timeout: 10)
        box.tap()
        box.typeText("Look @Al")
        let alex = a.buttons["mention-2"]
        XCTAssertTrue(alex.waitForExistence(timeout: 5), "typing @ offers the people")
        XCTAssertFalse(a.buttons["mention-1"].exists, "not myself")
        XCTAssertFalse(a.buttons["mention-3"].exists, "only names starting with what's typed")
        alex.tap()
        XCTAssertEqual(box.value as? String, "Look @Alex Kim ")
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: a.descendants(matching: .any)["mentionList"])
        wait(for: [gone], timeout: 5)
        let tail = "here \(Int.random(in: 1000...9999))"
        box.typeText(tail)
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts["Look @Alex Kim " + tail].waitForExistence(timeout: 5))
    }

    func testTrashIsEmptyForTheNextPersonAfterSignOut() {
        let a = launch(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 3, password: "sam-password-r4")
        a.terminate()
        let b = launch(["-wbServer", signInServer, "-wbOpen", "list:100:120"])
        let add = b.buttons["addTaskButton"]
        XCTAssertTrue(add.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: add)
        wait(for: [enabled], timeout: 10)
        let name = "Sam's secret \(Int.random(in: 1000...9999))"
        add.tap()
        let tf = b.textFields["newTaskField"]
        XCTAssertTrue(tf.waitForExistence(timeout: 5))
        tf.typeText(name + "\n")
        let row = b.staticTexts[name].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        let more = b.buttons["taskMoreMenu"]
        XCTAssertTrue(more.waitForExistence(timeout: 5))
        more.tap()
        b.buttons["deleteTaskButton"].tap()
        b.buttons["confirmDeleteTask"].firstMatch.tap()
        XCTAssertTrue(b.navigationBars["Backlog"].waitForExistence(timeout: 5))
        b.terminate()

        let c = launch(["-wbServer", signInServer, "-wbOpen", "settings"])
        let trash = c.buttons["trashLink"]
        XCTAssertTrue(reveal(trash, in: c, timeout: 10))
        trash.tap()
        XCTAssertTrue(c.buttons["restoreTrash-\(name)"].waitForExistence(timeout: 5), "in Sam's Trash")
        // the Trash screen's own back button (Settings' bar sits behind it)
        c.navigationBars["Trash"].buttons.element(boundBy: 0).tap()
        XCTAssertTrue(c.navigationBars["Settings"].waitForExistence(timeout: 5))
        let out = c.buttons["signOutButton"]
        XCTAssertTrue(reveal(out, in: c))
        out.tap()
        XCTAssertTrue(c.textFields["emailField"].waitForExistence(timeout: 10))

        // Alex signs in on this iPhone next: nothing of Sam's Trash
        join(c, member: 2, email: "alex@example.test", password: "alex-password-r4")
        c.buttons["settingsButton"].tap()
        XCTAssertTrue(reveal(c.buttons["trashLink"], in: c))
        c.buttons["trashLink"].tap()
        XCTAssertTrue(c.navigationBars["Trash"].waitForExistence(timeout: 5))
        XCTAssertFalse(c.buttons["restoreTrash-\(name)"].exists)
    }
}
