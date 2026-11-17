// UI tests for round 5: Slack history brought in by a Mac doesn't notify,
// and an access request the server answers late shows a gentle note until
// its answer comes (round 6: the late answer is taken as its own).
// Needs the fake servers from docs/ios-design.md:
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class Round5UITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func launch(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args + testArgs
        a.launch()
        return a
    }

    func testImportedSlackMessageDoesNotNotify() {
        let a = launch(["-wbReset", "YES", "-wbServer", "ws://127.0.0.1:18789", "-wbMe", "1", "-wbTab", "chat"])
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let card = a.buttons["notifyCardButton"]
        if card.waitForExistence(timeout: 8) {
            card.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        a.tabBars.buttons["Home"].tap()
        XCTAssertTrue(a.navigationBars["Home"].waitForExistence(timeout: 5))
        func banner(_ text: String) -> XCUIElement {
            springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        }
        let n = Int(Date().timeIntervalSince1970) % 100000
        let old = "From Slack \(n)"
        admin(port: 18789, "say?channel=301&author=2&text=\(old.replacingOccurrences(of: " ", with: "%20"))&imported=slack:C1:\(n).000100")
        XCTAssertFalse(banner(old).waitForExistence(timeout: 4), "imported history is not news")
        let fresh = "Written now \(n)"
        admin(port: 18789, "say?channel=301&author=2&text=\(fresh.replacingOccurrences(of: " ", with: "%20"))")
        XCTAssertTrue(banner(fresh).waitForExistence(timeout: 10), "a new message still notifies")
    }

    func testLateRoleChangeShowsANoteUntilItsAnswerComes() {
        let a = launch(["-wbReset", "YES", "-wbServer", signInServer, "-wbAskTimeout", "2"])
        join(a, member: 1, email: "jamie@example.com", password: "jamie-password-5")
        a.buttons["settingsButton"].tap()
        let people = a.buttons["peopleLink"]
        XCTAssertTrue(reveal(people, in: a))
        people.tap()
        let alex = a.buttons["personMenu-2"]
        XCTAssertTrue(alex.waitForExistence(timeout: 8))
        admin("late-answer?secs=7")
        alex.tap()
        let makeAdmin = a.buttons["Make admin"].exists ? a.buttons["Make admin"] : a.buttons["Make member"]
        XCTAssertTrue(makeAdmin.waitForExistence(timeout: 5))
        makeAdmin.tap()
        let note = a.staticTexts["adminUnconfirmed"].firstMatch
        waitLabel(note, contains: "didn't confirm", timeout: 8)
        XCTAssertFalse(a.staticTexts["adminError"].exists, "not an error")
        // the answer comes after all: the note goes, nothing shows as refused
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: note)
        wait(for: [gone], timeout: 12)
        XCTAssertFalse(a.staticTexts["adminError"].exists)
    }
}
