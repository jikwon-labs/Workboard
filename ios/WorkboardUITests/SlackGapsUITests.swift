// UI tests for what Slack had that Workboard lacked: group mentions in the
// @ list, keywords in the notification choices, quiet channels.
// Needs the fake servers from docs/ios-design.md:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class SlackGapsUITests: WorkboardUITestCase {
    /// "@ch" offers "@channel" after the people; picking it fills the box.
    func testChannelMentionPickedFromTheList() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        box.tap()
        box.typeText("Standup @ch")
        let group = a.buttons["mention-group-channel"]
        XCTAssertTrue(group.waitForExistence(timeout: 5), "typing @ch offers @channel")
        XCTAssertFalse(a.buttons["mention-group-here"].exists, "only groups starting with what's typed")
        group.tap()
        XCTAssertEqual(box.value as? String, "Standup @channel ")
        let tail = "now \(Int.random(in: 1000...9999))"
        box.typeText(tail)
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts["Standup @channel " + tail].waitForExistence(timeout: 5))
    }

    /// A keyword typed in the notification choices notifies like a mention;
    /// a channel set to nothing is marked quiet in the list.
    func testKeywordNotifiesAndQuietChannelIsMarked() {
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
        let field = app.textFields["notifyKeywords"]
        for _ in 0..<8 where !(field.exists && field.isHittable) { app.swipeUp() }
        field.tap()
        let word = "zebra\(Int.random(in: 100...999))"
        field.typeText(word + "\n")
        // #design set to nothing (just below the keywords)
        let mode = app.buttons["channelMode-302"]
        for _ in 0..<8 where !(mode.exists && mode.isHittable) {
            app.swipeUp(velocity: .slow)
            if !mode.exists { app.swipeDown(velocity: .slow) }
        }
        mode.tap()
        app.buttons["Nothing"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["Done"].firstMatch.tap()

        let text = "The \(word) shipped"
        say(text, channel: 301)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "a keyword notifies like a mention")

        app.activate()
        app.tabBars.buttons["Chat"].tap()
        XCTAssertTrue(app.descendants(matching: .any)["Notifications off"].waitForExistence(timeout: 5), "#design is marked quiet")
    }
}

final class ChatFormatUITests: WorkboardUITestCase {
    /// The formatting buttons put marks in the box; the message shows
    /// without them.
    func testBoldButtonAndTheMessageShowsWithoutMarks() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        box.tap()
        a.buttons["formatButton"].tap()
        let bold = a.buttons["format-Bold"]
        XCTAssertTrue(bold.waitForExistence(timeout: 5), "the formatting buttons show")
        bold.tap()
        let word = "loud\(Int.random(in: 1000...9999))"
        box.typeText(word)
        XCTAssertEqual(box.value as? String, "**\(word)**", "typing goes between the marks")
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts[word].waitForExistence(timeout: 5), "shown without the marks")
        XCTAssertFalse(a.staticTexts["**\(word)**"].exists)
    }
}

final class ChatSendingUITests: WorkboardUITestCase {
    /// Long press > Forward message: the message arrives in the other
    /// channel with the note, carrying the original.
    func testForwardAMessageToAnotherChannel() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        let tag = "\(Int.random(in: 100000...999999))"
        box.tap()
        box.typeText("Numbers are in \(tag)")
        a.buttons["sendButton"].tap()
        let sent = a.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "Numbers are in \(tag)")).firstMatch
        XCTAssertTrue(sent.waitForExistence(timeout: 5))
        sent.press(forDuration: 1.0)
        let fwd = a.buttons["Forward message"]
        XCTAssertTrue(fwd.waitForExistence(timeout: 5))
        fwd.tap()
        a.buttons["forwardTo-302"].tap()
        let note = a.textFields["forwardNote"]
        note.tap()
        note.typeText("FYI \(tag)")
        a.buttons["forwardButton"].tap()
        // over in #design
        a.navigationBars.buttons.element(boundBy: 0).tap()
        a.buttons["channel-302"].firstMatch.tap()
        XCTAssertTrue(a.staticTexts["FYI \(tag)"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.staticTexts["Numbers are in \(tag)"].exists, "the original rides along")
        XCTAssertTrue(a.descendants(matching: .any)["forwardedFrom"].firstMatch.exists)
    }

    /// An unsent message stays in its box and shows in Drafts; the clock
    /// keeps a message for later and empties the box.
    func testDraftsAndTheScheduleClock() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        let tag = "\(Int.random(in: 100000...999999))"
        box.tap()
        box.typeText("Half a thought \(tag)")
        a.navigationBars.buttons.element(boundBy: 0).tap()
        a.buttons["draftsMenu"].tap()
        a.buttons["Drafts"].tap()
        let row = a.buttons["draft-301-0"]
        XCTAssertTrue(row.waitForExistence(timeout: 5), "listed in Drafts")
        row.tap()
        let back = a.textFields["composer"]
        waitEnabled(back)
        XCTAssertEqual(back.value as? String, "Half a thought \(tag)", "the box has it again")
        a.buttons["scheduleButton"].tap()
        a.buttons["In 1 hour"].tap()
        let emptied = expectation(for: NSPredicate(format: "value != %@", "Half a thought \(tag)"), evaluatedWith: back)
        wait(for: [emptied], timeout: 5)
        XCTAssertFalse(a.staticTexts["Half a thought \(tag)"].exists, "not posted now")
    }
}
