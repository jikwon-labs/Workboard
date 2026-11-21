// Notifications: permission, a banner for a message that arrives while
// another screen is open, and tapping the banner to open the conversation.
// Needs: swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class NotificationUITests: WorkboardUITestCase {
    func testBannerForNewMessageOpensConversation() {
        let app = launchFresh(["-wbOpen", "chats"], server: adminServer)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

        // turn on notifications from the card in the chat list (once per install)
        let card = app.buttons["notifyCardButton"]
        if card.waitForExistence(timeout: 8) {
            card.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        // look at another screen, then someone mentions me in #general (channels
        // notify about mentions unless set to all messages, as on the Mac)
        goHome(app)
        let text = "Banner check \(Int(Date().timeIntervalSince1970) % 100000)"
        say("@Jamie Park " + text, channel: 301)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "banner shown")
        banner.tap()
        XCTAssertTrue(app.navigationBars["#general"].waitForExistence(timeout: 8), "tapping opens the conversation")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch.exists)
    }

    /// A banner tapped while a doc is being written opens the conversation
    /// and keeps what was written. With one Home stack the doc's screen is
    /// replaced, and its unsaved words used to go with it.
    func testABannerTappedWhileWritingADocKeepsTheWords() {
        let first = launchFresh(["-wbOpen", "chats"], server: adminServer)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let card = first.buttons["notifyCardButton"]
        if card.waitForExistence(timeout: 8) {
            card.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        first.terminate()

        let app = launchApp(["-wbServer", adminServer, "-wbMe", "1", "-wbOpen", "doc:402"])
        let edit = app.buttons["docEdit"]
        waitEnabled(edit)
        edit.tap()
        let editor = app.textViews["docEditor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        let tag = "Kept words \(Int.random(in: 100000...999999))"
        editor.typeText("\n\n" + tag)
        let text = "Draft check \(Int(Date().timeIntervalSince1970) % 100000)"
        say("@Jamie Park " + text, channel: 301)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "banner shown")
        banner.tap()
        XCTAssertTrue(app.navigationBars["#general"].waitForExistence(timeout: 8), "tapping opens the conversation")
        sleep(2) // the edit reaches the server
        app.terminate()

        let again = launchApp(["-wbServer", adminServer, "-wbMe", "1", "-wbOpen", "doc:402"])
        let body = again.staticTexts["docBody"]
        XCTAssertTrue(body.waitForExistence(timeout: 8))
        waitLabel(body, contains: tag)
    }

    /// Tapping a notification while the app is in the background brings it
    /// forward on the conversation (this used to stop the app).
    func testTappingABannerFromTheBackgroundOpensConversation() {
        let app = launchFresh(["-wbOpen", "chats"], server: adminServer)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let card = app.buttons["notifyCardButton"]
        if card.waitForExistence(timeout: 8) {
            card.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        XCTAssertTrue(app.navigationBars.firstMatch.waitForExistence(timeout: 5))
        XCUIDevice.shared.press(.home)
        XCTAssertTrue(app.wait(for: .runningBackground, timeout: 5))
        let text = "Away check \(Int(Date().timeIntervalSince1970) % 100000)"
        say("@Jamie Park " + text, channel: 301)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "banner shown while away")
        banner.tap()
        XCTAssertTrue(app.navigationBars["#general"].waitForExistence(timeout: 8), "tapping opens the conversation")
        sleep(3)
        XCTAssertEqual(app.state, .runningForeground, "the app is still running after the tap")
    }

    func testAChannelSetToAllMessagesNotifiesWithoutAMention() {
        let app = launchFresh(["-wbOpen", "settings"], server: adminServer)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let turnOn = app.buttons["notifyButton"]
        if turnOn.waitForExistence(timeout: 3) {
            turnOn.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        let link = app.buttons["notificationSettingsLink"]
        for _ in 0..<6 where !(link.exists && link.isHittable) { app.swipeUp() }
        link.tap()
        let mode = app.buttons["channelMode-301"]
        for _ in 0..<8 where !(mode.exists && mode.isHittable) { app.swipeUp() }
        mode.tap()
        app.buttons["All messages"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["Done"].firstMatch.tap()

        let text = "Plain note \(Int(Date().timeIntervalSince1970) % 100000)"
        say(text, channel: 301)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "a channel set to all messages notifies without a mention")
    }
}
