// UI tests for polls in chat: someone else's poll takes my vote, moves it
// and takes it back, with the counts and who voted; "/poll" opens New poll
// and posts one I can close and reopen, with no Edit. Needs the fake server:
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class PollsUITests: WorkboardUITestCase {
    /// The message id in a choice's identifier ("pollOption-<id>-<i>").
    func messageId(_ option: XCUIElement) -> String {
        option.identifier.split(separator: "-").dropFirst().first.map(String.init) ?? ""
    }

    func value(_ e: XCUIElement) -> String { e.value as? String ?? "" }

    /// A one-choice poll from Alex that Sam already voted in: my vote goes
    /// in, moves to the other choice, and is taken back; Show votes says who.
    func testVoteMoveAndTakeBack() {
        let tag = "\(Int.random(in: 100000...999999))"
        let pizza = "Pizza \(tag)", noodles = "Noodles \(tag)"
        let spec = "Lunch \(tag)?|\(pizza)|\(noodles)".addingPercentEncoding(withAllowedCharacters: .alphanumerics)!
        admin(port: 18789, "say?channel=303&author=2&poll=\(spec)&votes=3:0")
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)

        let first = a.buttons["Vote \(pizza)"]
        let second = a.buttons["Vote \(noodles)"]
        XCTAssertTrue(first.waitForExistence(timeout: 10), "the poll shows as a card with its choices")
        XCTAssertTrue(a.staticTexts["Lunch \(tag)?"].exists)
        XCTAssertTrue(a.staticTexts["Pick one"].exists)
        XCTAssertFalse(a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "1. \(pizza)")).firstMatch.exists, "the card, not the text")
        let id = messageId(first)
        let status = a.staticTexts["pollStatus-\(id)"]
        XCTAssertEqual(value(first), "1")
        XCTAssertEqual(value(second), "0")
        XCTAssertEqual(status.label, "1 person voted")
        XCTAssertFalse(a.buttons["closePoll-\(id)"].exists, "only its author closes it")

        second.tap()
        XCTAssertTrue(waitFor(second, value: "1"))
        XCTAssertTrue(second.isSelected, "my pick is marked")
        XCTAssertEqual(status.label, "2 people voted")

        first.tap()
        XCTAssertTrue(waitFor(first, value: "2"), "one choice: my vote moves")
        XCTAssertEqual(value(second), "0")
        XCTAssertTrue(first.isSelected)
        XCTAssertFalse(second.isSelected)

        first.tap()
        XCTAssertTrue(waitFor(first, value: "1"), "tapping my pick again takes it back")
        XCTAssertEqual(status.label, "1 person voted")

        a.buttons["showVotes-\(id)"].tap()
        XCTAssertTrue(a.staticTexts["Sam Lee"].waitForExistence(timeout: 5), "who voted, as the votes are shown")
        a.buttons["Done"].tap()
    }

    /// "/poll question" opens New poll with the question; Create poll is on
    /// with two choices; the poll takes several picks, and its author closes
    /// it (no more votes) and opens it again. A poll has no Edit.
    func testCreateCloseAndReopenMyPoll() {
        let tag = "\(Int.random(in: 100000...999999))"
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)
        let box = a.textFields["composer"]
        waitEnabled(box)
        box.tap()
        box.typeText("/poll Where \(tag)?")
        a.buttons["sendButton"].tap()

        XCTAssertTrue(a.navigationBars["New poll"].waitForExistence(timeout: 5), "/poll opens New poll")
        XCTAssertEqual(value(a.textFields["pollQuestion"]), "Where \(tag)?", "with the question typed after it")
        let create = a.buttons["createPoll"]
        XCTAssertFalse(create.isEnabled, "two choices first")
        let park = "Park \(tag)", beach = "Beach \(tag)"
        type(a.textFields["pollChoice-0"], park)
        type(a.textFields["pollChoice-1"], beach)
        a.buttons["addPollChoice"].tap()
        XCTAssertTrue(a.textFields["pollChoice-2"].waitForExistence(timeout: 5))
        a.buttons["removePollChoice-2"].tap()
        XCTAssertFalse(a.textFields["pollChoice-2"].waitForExistence(timeout: 1))
        let multi = a.switches["pollMulti"]
        multi.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertEqual(value(multi), "1")
        XCTAssertTrue(create.isEnabled)
        create.tap()

        let first = a.buttons["Vote \(park)"]
        let second = a.buttons["Vote \(beach)"]
        XCTAssertTrue(first.waitForExistence(timeout: 5), "the poll is in the conversation")
        XCTAssertTrue(a.staticTexts["Pick one or more"].exists)
        let id = messageId(first)
        first.tap()
        XCTAssertTrue(waitFor(first, value: "1"))
        second.tap()
        XCTAssertTrue(waitFor(second, value: "1"), "several picks")
        XCTAssertTrue(first.isSelected && second.isSelected)
        let status = a.staticTexts["pollStatus-\(id)"]
        XCTAssertEqual(status.label, "1 person voted")

        a.buttons["closePoll-\(id)"].tap()
        XCTAssertTrue(a.buttons["reopenPoll-\(id)"].waitForExistence(timeout: 5))
        XCTAssertEqual(status.label, "1 person voted · Poll closed")
        XCTAssertFalse(first.isEnabled, "a closed poll takes no votes")
        a.buttons["reopenPoll-\(id)"].tap()
        XCTAssertTrue(a.buttons["closePoll-\(id)"].waitForExistence(timeout: 5))
        XCTAssertEqual(status.label, "1 person voted")
        XCTAssertTrue(first.isEnabled)

        a.staticTexts["pollQuestion-\(id)"].press(forDuration: 1.0)
        XCTAssertTrue(a.buttons["Copy text"].waitForExistence(timeout: 5), "the message menu")
        XCTAssertFalse(a.buttons["Edit message"].exists, "a poll has no Edit")
    }

    func waitFor(_ e: XCUIElement, value v: String, timeout: TimeInterval = 5) -> Bool {
        let done = expectation(for: NSPredicate(format: "value == %@", v), evaluatedWith: e)
        return XCTWaiter().wait(for: [done], timeout: timeout) == .completed
    }
}
