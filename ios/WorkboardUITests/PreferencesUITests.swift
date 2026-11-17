// UI tests for Settings > What to notify about and Settings > Theme. Needs
// the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class PreferencesUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func settings() -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "settings"] + testArgs
        a.launch()
        return a
    }

    func find(_ e: XCUIElement, in a: XCUIApplication) {
        for _ in 0..<8 where !(e.exists && e.isHittable) { a.swipeUp() }
        XCTAssertTrue(e.waitForExistence(timeout: 5))
    }

    func testMuteAndChooseWhatToHearAbout() {
        let a = settings()
        let link = a.buttons["notificationSettingsLink"]
        find(link, in: a)
        link.tap()
        let direct = a.switches["notifyDirect"]
        XCTAssertTrue(direct.waitForExistence(timeout: 5))
        let before = direct.value as? String
        direct.switches.firstMatch.tap()
        let flipped = expectation(for: NSPredicate(format: "value != %@", before ?? ""), evaluatedWith: direct)
        wait(for: [flipped], timeout: 5)
        direct.switches.firstMatch.tap() // back as it was

        let mute = a.buttons["muteMenu"]
        if !mute.exists { a.buttons["unmuteButton"].tap() }
        XCTAssertTrue(mute.waitForExistence(timeout: 5))
        mute.tap()
        a.buttons["For 1 hour"].tap()
        XCTAssertTrue(a.descendants(matching: .any)["mutedUntil"].firstMatch.waitForExistence(timeout: 5))
        a.buttons["unmuteButton"].tap()
        XCTAssertTrue(mute.waitForExistence(timeout: 5))

        // a channel set apart from the default
        let mode = a.buttons["channelMode-302"]
        find(mode, in: a)
        mode.tap()
        a.buttons["Nothing"].tap()
        let set = expectation(for: NSPredicate(format: "label CONTAINS 'Nothing'"), evaluatedWith: mode)
        wait(for: [set], timeout: 5)
        mode.tap()
        a.buttons["Only @mentions"].tap()
    }

    func testDarkModeAndAnAccentColor() {
        let a = settings()
        let link = a.buttons["themeLink"]
        find(link, in: a)
        link.tap()
        let dark = a.buttons["Dark"]
        XCTAssertTrue(dark.waitForExistence(timeout: 5))
        dark.tap()
        XCTAssertTrue(dark.isSelected)
        let teal = a.buttons["accent-teal"]
        teal.tap()
        XCTAssertTrue(teal.isSelected)
        // back as it was
        a.buttons["As the system"].tap()
        a.buttons["accent-workboard"].tap()
        XCTAssertTrue(a.buttons["accent-workboard"].isSelected)
    }
}
