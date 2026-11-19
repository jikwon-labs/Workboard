// "Someone is typing..." above the message box. Needs the fake server with
// test-only HTTP:
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class TypingUITests: WorkboardUITestCase {
    func testWhoElseIsTypingShowsAboveTheMessageBoxForAMoment() {
        let a = launchFresh(["-wbOpen", "channel:302"], server: adminServer)
        waitEnabled(a.textFields["composer"])
        let line = a.staticTexts["typingLine"]
        XCTAssertFalse(line.exists, "nobody is typing yet")

        // Alex starts typing in #design
        admin(port: 18789, "typing?channel=302&member=2")
        XCTAssertTrue(line.waitForExistence(timeout: 5))
        XCTAssertEqual(line.label, "Alex Kim is typing...")

        // and Sam too
        admin(port: 18789, "typing?channel=302&member=3")
        waitLabel(line, contains: "Alex Kim and Sam Lee are typing...")

        // nothing more from them: it goes away after a few seconds
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: line)
        wait(for: [gone], timeout: 10)
    }
}
