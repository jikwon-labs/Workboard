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
        openPlace(app, "chats")
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
        // the marks go in on the next update: typing before then went in
        // ahead of them
        let marks = expectation(for: NSPredicate(format: "value == '****'"), evaluatedWith: box)
        wait(for: [marks], timeout: 5)
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

final class ChatCommandsUITests: WorkboardUITestCase {
    /// "/" lists the commands; /shrug posts with the shrug added.
    func testSlashListAndShrug() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        box.tap()
        box.typeText("/sh")
        let chip = a.buttons["command-shrug"]
        XCTAssertTrue(chip.waitForExistence(timeout: 5), "typing / lists the commands")
        chip.tap()
        XCTAssertEqual(box.value as? String, "/shrug ")
        let tag = "\(Int.random(in: 1000...9999))"
        box.typeText("fine \(tag)")
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts["fine \(tag) ¯\\_(ツ)_/¯"].waitForExistence(timeout: 5))
    }

    /// A workflow set up in the channel's settings replies in the thread of
    /// the next message.
    func testAWorkflowRepliesToTheNextMessage() {
        let a = launchFresh(["-wbOpen", "channel:302"])
        let box = a.textFields["composer"]
        waitEnabled(box)
        a.buttons["channelSettingsButton"].tap()
        let link = a.buttons["workflowsLink"]
        XCTAssertTrue(link.waitForExistence(timeout: 5))
        link.tap()
        let tag = "\(Int.random(in: 1000...9999))"
        let kw = a.textFields["workflowKeyword"]
        kw.tap()
        kw.typeText("ping\(tag)")
        a.buttons["workflowAction"].tap()
        a.buttons["Reply in its thread"].tap()
        let text = a.textFields["workflowText"]
        text.tap()
        text.typeText("pong")
        a.buttons["addWorkflow"].tap()
        XCTAssertTrue(a.staticTexts["Reply in its thread: pong"].waitForExistence(timeout: 5))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        a.buttons["channelSettingsDone"].tap()
        box.tap()
        box.typeText("ping\(tag) are you there")
        a.buttons["sendButton"].tap()
        XCTAssertTrue(a.staticTexts["ping\(tag) are you there"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.staticTexts["1 reply"].firstMatch.waitForExistence(timeout: 5), "the workflow replied")
    }

    /// A message that comes in while the list is scrolled up leaves it
    /// where it is: a grey note over the message box says so and goes down
    /// to it, and a red line marks where the new ones start.
    func testNewMessagesWhileScrolledUpShowAQuietNoteAndARedLine() {
        let tag = Int.random(in: 1000...9999)
        let a = launchFresh(["-wbOpen", "channel:302"], server: adminServer)
        waitEnabled(a.textFields["composer"])
        for i in 0..<14 {
            say("Earlier note \(i) \(tag): a longer line so the list scrolls well past one screen on a phone.", channel: 302)
        }
        XCTAssertTrue(a.staticTexts["Earlier note 13 \(tag): a longer line so the list scrolls well past one screen on a phone."].waitForExistence(timeout: 5), "down at the newest while at the bottom")
        let note = a.buttons["newMessagesButton"]
        let line = a.descendants(matching: .any)["readLine"].firstMatch
        XCTAssertFalse(note.exists, "nothing came in below")
        XCTAssertFalse(line.exists, "read as they came in")

        let list = a.scrollViews.firstMatch
        list.swipeDown()
        list.swipeDown()
        say("Fresh one \(tag)", channel: 302)
        XCTAssertTrue(note.waitForExistence(timeout: 5), "a note for the message below")
        waitLabel(note, contains: "1 new message")
        XCTAssertFalse(a.staticTexts["Fresh one \(tag)"].isHittable, "the list stayed where I was reading")
        note.tap()
        XCTAssertTrue(a.staticTexts["Fresh one \(tag)"].waitForExistence(timeout: 5))
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: note)
        wait(for: [gone], timeout: 5)
        XCTAssertTrue(line.exists, "a red line above the new one")
    }
}
