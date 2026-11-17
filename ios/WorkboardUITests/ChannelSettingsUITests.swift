// UI tests for a group conversation (a private channel with chosen people)
// and Workspace > Automations. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class ChannelSettingsUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func waitEnabled(_ e: XCUIElement) {
        XCTAssertTrue(e.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: e)
        wait(for: [enabled], timeout: 10)
    }

    func testMakeAGroupConversationAndRenameIt() {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbTab", "chat"] + testArgs
        a.launch()
        let tag = "\(Int.random(in: 100000...999999))"
        let newChannel = a.buttons["newChannelButton"]
        waitEnabled(newChannel)
        newChannel.tap()
        let nameField = a.textFields["channelNameField"]
        XCTAssertTrue(nameField.waitForExistence(timeout: 5))
        nameField.tap()
        nameField.typeText("crew \(tag)")
        let priv = a.switches["channelPrivateToggle"]
        if priv.exists, (priv.value as? String) == "0" { priv.switches.firstMatch.tap() }
        a.buttons["createChannelButton"].tap()

        let settings = a.buttons["channelSettingsButton"]
        waitEnabled(settings)
        settings.tap()
        let alex = a.switches["channelMember-2"]
        XCTAssertTrue(alex.waitForExistence(timeout: 5))
        XCTAssertEqual(alex.value as? String, "0")
        alex.switches.firstMatch.tap()
        let on = expectation(for: NSPredicate(format: "value == '1'"), evaluatedWith: alex)
        wait(for: [on], timeout: 5)
        let name = a.textFields["channelSettingsName"]
        name.tap()
        name.typeText(" team")
        a.buttons["channelSettingsDone"].tap()
        XCTAssertTrue(a.navigationBars["#crew-\(tag)-team"].waitForExistence(timeout: 5), "the new name is written like a channel name")
    }

    func testAddAnAutomationAndTurnItOff() {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "settings"] + testArgs
        a.launch()
        let link = a.buttons["automationsLink"]
        for _ in 0..<5 where !link.isHittable { a.swipeUp() }
        XCTAssertTrue(link.waitForExistence(timeout: 10))
        link.tap()
        let add = a.buttons["addAutomationButton"]
        waitEnabled(add)
        let before = a.switches.matching(NSPredicate(format: "identifier BEGINSWITH 'automation-'")).count
        add.tap()
        a.buttons["automationAction"].tap()
        a.buttons["Comment"].tap()
        let comment = a.textFields["automationComment"]
        XCTAssertTrue(comment.waitForExistence(timeout: 5))
        comment.tap()
        comment.typeText("Ready for a look")
        a.buttons["automationAdd"].tap()
        let rows = a.switches.matching(NSPredicate(format: "identifier BEGINSWITH 'automation-'"))
        let more = expectation(for: NSPredicate(format: "count == %d", before + 1), evaluatedWith: rows)
        wait(for: [more], timeout: 5)
        let row = rows.element(boundBy: before)
        XCTAssertEqual(row.value as? String, "1")
        row.switches.firstMatch.tap()
        let off = expectation(for: NSPredicate(format: "value == '0'"), evaluatedWith: row)
        wait(for: [off], timeout: 5)
        row.swipeLeft()
        a.buttons["Delete"].tap()
        let back = expectation(for: NSPredicate(format: "count == %d", before), evaluatedWith: rows)
        wait(for: [back], timeout: 5)
    }

    func testWriteAPostSaveItAndMakeATaskFromIt() {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "channel:302"] + testArgs
        a.launch()
        let write = a.buttons["writePostButton"]
        waitEnabled(write)
        write.tap()
        let tag = "\(Int.random(in: 100000...999999))"
        let title = a.textFields["postTitle"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        title.tap()
        title.typeText("Launch \(tag)")
        let text = a.textFields["postText"].exists ? a.textFields["postText"] : a.textViews["postText"]
        text.tap()
        text.typeText("Ship it Friday \(tag)")
        a.buttons["postSend"].tap()
        let body = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Ship it Friday \(tag)")).firstMatch
        XCTAssertTrue(body.waitForExistence(timeout: 5))
        XCTAssertTrue(a.staticTexts["Launch \(tag)"].exists, "the post's title shows")

        body.press(forDuration: 1.0)
        a.buttons["Save for later"].tap()
        body.press(forDuration: 1.0)
        a.buttons["Create task"].tap()
        let chip = a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'messageTask-'")).firstMatch
        XCTAssertTrue(chip.waitForExistence(timeout: 5), "the message links to its task")
    }
}
