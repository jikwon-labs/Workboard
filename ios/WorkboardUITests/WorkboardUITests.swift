// Drives the app in the simulator against a running server.
// Start one first (from ios/WorkboardKit):
//   swift run wb-fake-server --port 18787
// The address can be changed with the WB_SERVER environment variable
// (TEST_RUNNER_WB_SERVER when passed through xcodebuild).

import XCTest

final class WorkboardUITests: WorkboardUITestCase {
    var server: String { ProcessInfo.processInfo.environment["WB_SERVER"] ?? openServer }

    func launch(_ extra: [String] = [], connect: Bool = true) -> XCUIApplication {
        connect ? launchFresh(extra, server: server) : launchApp(["-wbReset", "YES"] + extra)
    }

    func testFirstRunConnectsAndChoosesMember() {
        let app = launch(connect: false)
        let field = app.textFields["serverField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(server)
        app.buttons["connectButton"].tap()
        let me = app.buttons["member-1"]
        XCTAssertTrue(me.waitForExistence(timeout: 10), "member list after connecting")
        me.tap()
        XCTAssertTrue(app.buttons["quickFind"].waitForExistence(timeout: 5))
    }

    func testSendMessageAndReply() {
        let app = launch()
        openPlace(app, "chats")
        let general = app.buttons["channel-301"]
        XCTAssertTrue(general.waitForExistence(timeout: 10))
        general.tap()
        let box = app.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 5))
        let text = "Hello from the simulator \(Int(Date().timeIntervalSince1970) % 10000)"
        box.tap()
        box.typeText(text)
        app.buttons["sendButton"].tap()
        XCTAssertTrue(app.staticTexts[text].waitForExistence(timeout: 5))

        // a thread reply on Alex's first message
        let replies = app.buttons["replies-311"]
        XCTAssertTrue(replies.waitForExistence(timeout: 5))
        replies.tap()
        let rbox = app.textFields["composer"]
        XCTAssertTrue(rbox.waitForExistence(timeout: 5))
        rbox.tap()
        rbox.typeText("Reply from the simulator")
        app.buttons["sendButton"].tap()
        XCTAssertTrue(app.staticTexts["Reply from the simulator"].waitForExistence(timeout: 5))
    }

    func testChangeStatusAndAddTask() {
        let app = launch(["-wbOpen", "task:204"])
        let menu = app.buttons["statusMenu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        XCTAssertTrue(menu.isEnabled)
        menu.tap()
        app.buttons["IN PROGRESS"].tap()
        wait(for: [expectation(for: NSPredicate(format: "label CONTAINS 'IN PROGRESS'"), evaluatedWith: menu)], timeout: 5)

        let complete = app.buttons["completeButton"]
        complete.tap()
        let reopened = expectation(for: NSPredicate(format: "label CONTAINS 'Reopen'"), evaluatedWith: complete)
        wait(for: [reopened], timeout: 5)

        // add a task in the same list
        goHome(app)
        app.buttons["space-100"].tap()
        app.buttons["list-112"].tap()
        app.buttons["addTaskButton"].tap()
        let field = app.textFields["newTaskField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("Added on iPhone\n")
        XCTAssertTrue(app.staticTexts["Added on iPhone"].waitForExistence(timeout: 5))
    }

    func testEditDoc() {
        let app = launch(["-wbOpen", "doc:402"])
        let edit = app.buttons["docEdit"]
        XCTAssertTrue(edit.waitForExistence(timeout: 10))
        edit.tap()
        let editor = app.textViews["docEditor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        editor.typeText("\nAdded from the iPhone.")
        app.buttons["docDone"].tap()
        let body = app.staticTexts["docBody"]
        XCTAssertTrue(body.waitForExistence(timeout: 5))
        XCTAssertTrue((body.label).contains("Added from the iPhone."))
    }
}
