// UI tests for the steps after phase 1: offline edits, being signed out by
// an admin, huddles, reactions, comments, task fields and new docs. Signing
// in itself is in AccountUITests.
// Needs three fake servers (from ios/WorkboardKit):
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
//   swift run wb-fake-server --port 18789 --admin-http
// (no LiveKit server is running, so joining a huddle on 18788 fails on purpose)

import XCTest

final class NextStepsUITests: WorkboardUITestCase {
    func testOfflineEditGoesUpAfterReconnect() {
        // the server goes away and comes back at the same address (a new
        // address would rightly make the app start from a fresh copy)
        let a = launchFresh(["-wbOpen", "channel:302"], server: adminServer)
        addTeardownBlock { self.admin(port: 18789, "online") }
        let box = a.textFields["composer"]
        XCTAssertTrue(box.waitForExistence(timeout: 10))
        let banner = a.descendants(matching: .any)["offlineBanner"]
        let connected = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: banner)
        wait(for: [connected], timeout: 10)

        admin(port: 18789, "offline?secs=90")
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "the app notices the server is gone")
        XCTAssertTrue(box.isEnabled, "the message box works offline")
        let text = "Written on the train \(Int(Date().timeIntervalSince1970))"
        box.tap()
        box.typeText(text)
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts[text].waitForExistence(timeout: 5))
        waitLabel(banner, contains: "1 change waiting")
        XCTAssertEqual(admin(port: 18789, "find?text=\(text.replacingOccurrences(of: " ", with: "%20"))")["found"] as? Bool, false)

        // back in reach: the app reconnects on its own, the waiting edit goes
        // up and the banner goes away once the server shows it back
        admin(port: 18789, "online")
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: banner)
        wait(for: [gone], timeout: 45)
        XCTAssertEqual(admin(port: 18789, "find?text=\(text.replacingOccurrences(of: " ", with: "%20"))")["found"] as? Bool, true, "the edit is on the server")
    }

    func testRevokedDeviceIsSignedOut() {
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
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
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 2, email: "alex@example.test", password: "alex-password-1")
        openPlace(a, "chats")
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
        let a = launchFresh(["-wbOpen", "channel:302"])
        let huddle = a.buttons["huddleButton"]
        waitEnabled(huddle)
        huddle.tap()
        // Safari opens the link; come back and find the call message
        sleep(2)
        a.activate()
        XCTAssertTrue(a.staticTexts["Started a call"].waitForExistence(timeout: 10))
        XCTAssertTrue(a.links["Join call"].exists || a.buttons["Join call"].exists)
    }

    func testReactCommentPriorityAndNewDoc() {
        let a = launchFresh(["-wbOpen", "channel:301"])
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
        let b = launchApp(["-wbServer", openServer, "-wbOpen", "task:205"])
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
        let c = launchApp(["-wbServer", openServer, "-wbOpen", "docs"])
        let newDoc = c.buttons["newDocButton"]
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
