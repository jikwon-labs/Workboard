// UI tests for voice messages: a sound someone sent shows as a card that
// plays, pauses and shows how far it got; both message boxes have the
// microphone button; recording in the simulator (with the Mac's microphone)
// goes into the box, plays there, and sends. Needs the fake server:
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class VoiceMessagesUITests: WorkboardUITestCase {
    /// A voice message from Alex (4 seconds of silence as a WAV) plays from
    /// its card: fetched on the first press, Pause while it plays, and the
    /// time shows where it stopped.
    func testVoiceMessageCardPlaysAndPauses() {
        let tag = "\(Int.random(in: 100000...999999))"
        let name = "voice-\(tag).wav"
        admin(port: 18789, "say?channel=303&author=2&text=&file=\(name)&kind=Audio")
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)

        let play = a.buttons["Play \(name)"]
        XCTAssertTrue(play.waitForExistence(timeout: 10), "the voice message shows as a card with a play button")
        // the bar says where it is, as the time under it does
        let bar = a.otherElements["Progress of \(name)"]
        XCTAssertTrue(bar.exists)
        XCTAssertEqual(bar.value as? String, "0:04", "its length before it plays")
        XCTAssertTrue(a.buttons["recordButton"].exists, "the message box has the microphone button")
        XCTAssertEqual(a.buttons["recordButton"].label, "Record a voice message")

        play.tap()
        let pause = a.buttons["Pause \(name)"]
        XCTAssertTrue(pause.waitForExistence(timeout: 10), "it plays: the button pauses now")
        sleep(1)
        pause.tap()
        XCTAssertTrue(play.waitForExistence(timeout: 5), "paused: play again")
        let at = bar.value as? String ?? ""
        XCTAssertTrue(at.hasSuffix(" / 0:04") && at != "0:00 / 0:04", "the time shows where it stopped: \(at)")

        // played to the end, it is ready to play again from the start
        play.tap()
        XCTAssertTrue(pause.waitForExistence(timeout: 5))
        XCTAssertTrue(play.waitForExistence(timeout: 10), "back to play at the end")
    }

    /// Recording in the simulator (it takes the Mac's microphone): the bar
    /// shows the time out of 5:00, Stop puts the recording in the box where
    /// it plays before sending, Send posts it as a voice message card; the
    /// bin throws one away. Run `xcrun simctl privacy <device> grant
    /// microphone com.seojigwon.workboard` once first, or tap Allow.
    func testRecordPlayInTheBoxAndSend() {
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)
        let mic = a.buttons["recordButton"]
        waitEnabled(mic)
        mic.tap()
        // the first time, the system asks for the microphone
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.alerts.buttons.matching(NSPredicate(format: "label IN {'Allow', 'OK'}")).firstMatch
        if allow.waitForExistence(timeout: 3) { allow.tap() }
        let stop = a.buttons["stopRecording"]
        XCTAssertTrue(stop.waitForExistence(timeout: 5), "recording: the bar with Stop shows")
        XCTAssertTrue(a.buttons["discardRecording"].exists)
        XCTAssertEqual(mic.label, "Stop recording")
        sleep(2)
        let time = a.staticTexts["recordingTime"]
        XCTAssertTrue(time.label.hasSuffix(" / 5:00") && time.label != "0:00 / 5:00", "the time so far: \(time.label)")
        stop.tap()
        XCTAssertFalse(stop.waitForExistence(timeout: 1), "the bar goes")

        let preview = a.buttons["Play Voice message"]
        XCTAssertTrue(preview.waitForExistence(timeout: 5), "the recording waits in the box")
        let send = a.buttons["sendButton"]
        let uploaded = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: send)
        wait(for: [uploaded], timeout: 10)
        preview.tap()
        let pause = a.buttons["Pause Voice message"]
        XCTAssertTrue(pause.waitForExistence(timeout: 5), "it plays before sending")
        pause.tap()
        send.tap()
        XCTAssertFalse(preview.waitForExistence(timeout: 2), "the box is empty again")
        let sent = a.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Play Voice message 20' AND label ENDSWITH '.m4a'")).firstMatch
        XCTAssertTrue(sent.waitForExistence(timeout: 5), "the message shows a voice message card")

        // the bin throws a recording away (too short to send is
        // VoiceTests: a tap here takes longer than half a second)
        mic.tap()
        XCTAssertTrue(stop.waitForExistence(timeout: 5))
        a.buttons["discardRecording"].tap()
        XCTAssertFalse(stop.waitForExistence(timeout: 1), "the bar goes")
        XCTAssertFalse(preview.exists, "nothing in the box")
        XCTAssertEqual(mic.label, "Record a voice message")
    }

    /// The thread's reply box records too.
    func testThreadReplyBoxHasTheMicrophone() {
        let a = launchFresh(["-wbOpen", "thread:301:311"])
        XCTAssertTrue(a.navigationBars["Thread"].waitForExistence(timeout: 10))
        let mic = a.buttons["recordButton"]
        XCTAssertTrue(mic.waitForExistence(timeout: 5))
        XCTAssertEqual(mic.label, "Record a voice message")
    }
}
