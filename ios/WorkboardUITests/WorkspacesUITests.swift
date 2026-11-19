// Settings > Workspaces: the workspace left for another server stays on the
// iPhone and opens again with one tap. Needs two open fake servers:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class WorkspacesUITests: WorkboardUITestCase {
    func openSettings(_ a: XCUIApplication) {
        let gear = a.buttons["settingsButton"]
        XCTAssertTrue(gear.waitForExistence(timeout: 10))
        gear.tap()
        XCTAssertTrue(a.descendants(matching: .any)["currentWorkspace"].firstMatch.waitForExistence(timeout: 5), "Settings open")
    }

    func testSwitchToAnotherServerAndBack() {
        let a = launchFresh(["-wbTab", "home"])
        XCTAssertTrue(a.navigationBars["Home"].waitForExistence(timeout: 10))

        // another server's address: its own workspace, the first one kept
        openSettings(a)
        let field = a.textFields["settingsServerField"]
        type(field, adminServer)
        field.typeText("\n")
        a.buttons["Done"].tap()
        let me = a.buttons["member-1"]
        XCTAssertTrue(me.waitForExistence(timeout: 10), "a server not opened here before: pick who you are")
        me.tap()
        XCTAssertTrue(a.navigationBars["Home"].waitForExistence(timeout: 10))

        openSettings(a)
        let current = a.descendants(matching: .any)["currentWorkspace"].firstMatch
        waitLabel(current, contains: "18789")
        let kept = a.buttons["savedWorkspace-\(openServer)"]
        XCTAssertTrue(kept.waitForExistence(timeout: 5), "the first workspace is listed")

        // back with one tap: no picking again, straight to Home
        kept.tap()
        XCTAssertTrue(a.navigationBars["Home"].waitForExistence(timeout: 10))
        XCTAssertFalse(a.buttons["member-1"].exists)
        openSettings(a)
        waitLabel(a.descendants(matching: .any)["currentWorkspace"].firstMatch, contains: "18787")
        XCTAssertTrue(a.buttons["savedWorkspace-\(adminServer)"].exists, "and the second one is kept in turn")

        // swipe to remove it from this iPhone
        a.buttons["savedWorkspace-\(adminServer)"].swipeLeft()
        let remove = a.buttons["forgetWorkspace"]
        XCTAssertTrue(remove.waitForExistence(timeout: 5))
        remove.tap()
        let confirm = a.buttons["confirmForgetWorkspace"].firstMatch
        XCTAssertTrue(confirm.waitForExistence(timeout: 5))
        confirm.tap()
        let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: a.buttons["savedWorkspace-\(adminServer)"])
        XCTAssertEqual(XCTWaiter().wait(for: [gone], timeout: 5), .completed)
    }
}
