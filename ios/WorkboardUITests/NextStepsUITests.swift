// UI tests for the steps after phase 1: offline edits, being signed out by
// an admin, huddles, reactions, comments, task fields and new docs. Signing
// in itself is in AccountUITests.
// Needs two fake servers (from ios/WorkboardKit):
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
// (no LiveKit server is running, so joining a huddle on 18788 fails on purpose)

import XCTest

final class NextStepsUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"
    let locked = "ws://127.0.0.1:18788"
    let dead = "ws://127.0.0.1:18799"

    override func setUp() { continueAfterFailure = false }

    func app(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args
        a.launch()
        return a
    }

    func testOfflineEditGoesUpAfterReconnect() {
        // load the workspace once, then come back with the server unreachable
        var a = app(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1"])
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10))
        a.terminate()
        a = app(["-wbServer", dead, "-wbOpen", "channel:302"])
        let banner = a.descendants(matching: .any)["offlineBanner"]
        XCTAssertTrue(banner.waitForExistence(timeout: 10))
        let box = a.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 5))
        XCTAssertTrue(box.isEnabled, "the message box works offline")
        box.tap()
        box.typeText("Written on the train")
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts["Written on the train"].waitForExistence(timeout: 5))
        waitLabel(banner, contains: "1 change waiting")
        a.terminate()

        // back in reach: the banner goes away and nothing is left waiting
        a = app(["-wbServer", open, "-wbOpen", "channel:302"])
        XCTAssertTrue(a.staticTexts["Written on the train"].waitForExistence(timeout: 10))
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: a.descendants(matching: .any)["offlineBanner"])
        wait(for: [gone], timeout: 10)
    }

    func testRevokedDeviceIsSignedOut() {
        let a = app(["-wbReset", "YES", "-wbServer", locked])
        join(a, member: 3, password: "sam-password-1")
        a.buttons["settingsButton"].tap()
        waitLabel(a.staticTexts["signedInAs"], contains: "Sam Lee")
        a.buttons["Done"].tap()

        _ = admin("revoke?member=3")
        let out = a.staticTexts["Signed out"].waitForExistence(timeout: 10)
        XCTAssertTrue(out, "revoked device goes back to the sign-in screen")
        waitLabel(a.staticTexts["signInNote"], contains: "Your access to this server was removed.")
    }

    func testHuddleOnServerWithCallsTriesToJoin() {
        let a = app(["-wbReset", "YES", "-wbServer", locked])
        join(a, member: 2, email: "alex@example.test", password: "alex-password-1")
        a.tabBars.buttons["Chat"].tap()
        a.buttons["channel-301"].tap()
        let huddle = a.buttons["huddleButton"]
        _ = huddle.waitForExistence(timeout: 5)
        huddle.tap()
        // the server gave a token; there is no LiveKit server, so joining fails with a message
        let failed = a.descendants(matching: .any)["callFailed"].waitForExistence(timeout: 30)
        XCTAssertTrue(failed)
        a.buttons["Close"].tap()
    }

    func testHuddleWithoutCallServerPostsLinkCall() {
        let a = app(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "channel:302"])
        let huddle = a.buttons["huddleButton"]
        XCTAssertTrue(huddle.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: huddle)
        wait(for: [enabled], timeout: 10)
        huddle.tap()
        // Safari opens the link; come back and find the call message
        sleep(2)
        a.activate()
        XCTAssertTrue(a.staticTexts["Started a call"].waitForExistence(timeout: 10))
        XCTAssertTrue(a.links["Join call"].exists || a.buttons["Join call"].exists)
    }

    func testReactCommentPriorityAndNewDoc() {
        let a = app(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "channel:301"])
        let msg = a.descendants(matching: .any)["message-313"]
        XCTAssertTrue(msg.waitForExistence(timeout: 10))
        // the server keeps state between runs, so check that the tap flips it
        let chip = a.buttons.matching(NSPredicate(format: "label BEGINSWITH '👍 1'")).firstMatch
        let hadIt = chip.exists
        msg.press(forDuration: 1.0)
        let thumbs = a.buttons["React 👍"]
        XCTAssertTrue(thumbs.waitForExistence(timeout: 5))
        thumbs.tap()
        wait(for: [expectation(for: NSPredicate(format: "exists == %@", NSNumber(value: !hadIt)), evaluatedWith: chip)], timeout: 5)

        a.terminate()
        let b = app(["-wbServer", open, "-wbOpen", "task:205"])
        let pr = b.buttons["priorityMenu"]
        XCTAssertTrue(pr.waitForExistence(timeout: 10))
        pr.tap()
        b.buttons["Urgent"].tap()
        waitLabel(pr, contains: "Urgent")
        let box = b.textFields["composer"]
        box.tap()
        box.typeText("Picking this up")
        b.buttons["sendButton"].tap()
        XCTAssertTrue(b.staticTexts["Picking this up"].waitForExistence(timeout: 5))

        b.terminate()
        let c = app(["-wbServer", open, "-wbTab", "docs"])
        let newDoc = c.navigationBars.buttons["New doc"]
        XCTAssertTrue(newDoc.waitForExistence(timeout: 5))
        newDoc.tap()
        let editor = c.textViews["docEditor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        editor.typeText("Notes from my phone")
        c.buttons["docDone"].tap()
        XCTAssertTrue(c.staticTexts["docBody"].waitForExistence(timeout: 5))
    }
}
