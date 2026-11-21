// Settings > Workspaces: the workspace left for another server stays on the
// iPhone and opens again with one tap. Needs two open fake servers:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18789 --admin-http
// and, for the workspaces an account has on one server:
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev

import XCTest

final class WorkspacesUITests: WorkboardUITestCase {
    func openSettings(_ a: XCUIApplication) {
        let gear = a.buttons["settingsButton"]
        XCTAssertTrue(gear.waitForExistence(timeout: 10))
        gear.tap()
        XCTAssertTrue(a.descendants(matching: .any)["currentWorkspace"].firstMatch.waitForExistence(timeout: 5), "Settings open")
    }

    func testSwitchToAnotherServerAndBack() {
        let a = launchFresh([])
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))

        // another server's address: its own workspace, the first one kept
        openSettings(a)
        let field = a.textFields["settingsServerField"]
        type(field, adminServer)
        field.typeText("\n")
        a.buttons["Done"].tap()
        let me = a.buttons["member-1"]
        XCTAssertTrue(me.waitForExistence(timeout: 10), "a server not opened here before: pick who you are")
        me.tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))

        openSettings(a)
        let current = a.descendants(matching: .any)["currentWorkspace"].firstMatch
        waitLabel(current, contains: "18789")
        let kept = a.buttons["savedWorkspace-\(openServer)"]
        XCTAssertTrue(kept.waitForExistence(timeout: 5), "the first workspace is listed")

        // back with one tap: no picking again, straight to Home
        kept.tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))
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

    func testFindAnotherWorkspaceOnTheSameServerAndOpenIt() {
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 3, password: "find-pass-1")
        admin("workspaces?list=:Acme,k3x9:Design")
        defer { admin("workspaces") }

        openSettings(a)
        let link = a.buttons["findWorkspacesLink"]
        XCTAssertTrue(reveal(link, in: a))
        link.tap()
        let email = a.textFields["findEmailField"]
        XCTAssertTrue(email.waitForExistence(timeout: 5))
        XCTAssertEqual(email.value as? String, "sam@example.com", "the account's email is filled in")

        // a wrong password: the server's words, nothing listed
        type(a.secureTextFields["findPasswordField"], "not-the-password")
        a.buttons["findWorkspacesButton"].tap()
        waitLabel(a.staticTexts["findWorkspacesError"], contains: "That email and password don't match.")
        XCTAssertFalse(a.buttons["foundWorkspace-k3x9"].exists)

        type(a.secureTextFields["findPasswordField"], "find-pass-1")
        a.buttons["findWorkspacesButton"].tap()
        let other = a.buttons["foundWorkspace-k3x9"]
        XCTAssertTrue(other.waitForExistence(timeout: 10), "the account's other workspace is listed")
        XCTAssertFalse(a.buttons["foundWorkspace-first"].isEnabled, "the open one is marked, not tappable")

        // open it: signed in there with the same password, the first one kept
        other.tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "opened without the sign-in screen")
        XCTAssertFalse(a.textFields["emailField"].exists)
        openSettings(a)
        waitLabel(a.descendants(matching: .any)["currentWorkspace"].firstMatch, contains: "/w/k3x9")
        XCTAssertTrue(a.buttons["savedWorkspace-\(signInServer)"].waitForExistence(timeout: 5), "the first workspace is kept to switch back to")
    }
}
