import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Polls in chat, as the Mac makes and merges them (src/app/polls.rs,
/// Message::toggle_vote, poll_text).
final class PollTests: XCTestCase {
    func message(_ d: JSON, _ id: UInt64, channel: UInt64 = 301) -> JSON? {
        let msgs = d["channels"]?.array?.first { $0["id"]?.uint == channel }?["messages"]?.array ?? []
        if let m = msgs.first(where: { $0["id"]?.uint == id }) { return m }
        return msgs.flatMap { $0["replies"]?.array ?? [] }.first { $0["id"]?.uint == id }
    }

    func view(_ d: JSON, _ id: UInt64) -> MessageView? {
        let c = WorkspaceView(d).channels.first { $0.id == 301 }!
        return c.messages.first { $0.id == id } ?? c.messages.flatMap(\.replies).first { $0.id == id }
    }

    func poll(multi: Bool = false, closed: Int64? = nil, n: Int = 3) -> PollView {
        PollView(Polls.object(question: "Q", options: (0..<n).map { "o\($0)" }, multi: multi, anonymous: false, closed: closed))!
    }

    func testPollTextNumbersTrimmedChoices() {
        XCTAssertEqual(Polls.text(question: "  Lunch? ", options: [" Pizza ", "Noodles"]), "Lunch?\n1. Pizza\n2. Noodles")
        XCTAssertEqual(Polls.filled(["a ", "  ", "", " b"]), ["a", "b"])
        XCTAssertTrue(Polls.ready(question: "Q", options: ["a", "b", ""]))
        XCTAssertFalse(Polls.ready(question: "Q", options: ["a", " "]), "two choices with words")
        XCTAssertFalse(Polls.ready(question: "  ", options: ["a", "b"]), "a question")
    }

    func testToggleRules() {
        let single = poll()
        XCTAssertEqual(Polls.toggled([], option: 1, poll: single), [1])
        XCTAssertEqual(Polls.toggled([1], option: 2, poll: single), [2], "one choice: the vote moves")
        XCTAssertEqual(Polls.toggled([2], option: 2, poll: single), [], "my pick again takes it back")
        let multi = poll(multi: true)
        XCTAssertEqual(Polls.toggled([2], option: 0, poll: multi), [0, 2], "added in order")
        XCTAssertEqual(Polls.toggled([0, 2], option: 2, poll: multi), [0])
        XCTAssertNil(Polls.toggled([], option: 3, poll: single), "no such choice")
        XCTAssertNil(Polls.toggled([], option: -1, poll: single))
        XCTAssertNil(Polls.toggled([0], option: 1, poll: poll(closed: 99)), "closed polls take no votes")
        XCTAssertEqual(Polls.counts([2: [0, 1], 10: [1], 11: [7]], options: 2), [1, 2], "picks past the choices don't count")
    }

    func testSendPollWritesTheMacsShape() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.sendPoll(&d, channel: 301, question: " Lunch? ", options: ["Pizza", " ", "Noodles "], multi: false, anonymous: false, me: 1, id: 9600, ts: 50))
        let m = message(d, 9600)!
        XCTAssertEqual(m["text"]?.string, "Lunch?\n1. Pizza\n2. Noodles", "what old apps show")
        XCTAssertEqual(m["poll"], ["question": "Lunch?", "options": ["Pizza", "Noodles"]], "multi, anonymous and closed left out")
        XCTAssertNil(m["votes"], "no votes key until someone votes")
        XCTAssertEqual(m["author"]?.uint, 1)

        XCTAssertTrue(Edits.sendPoll(&d, channel: 301, question: "Where?", options: ["A", "B"], multi: true, anonymous: true, me: 1, id: 9601, ts: 51))
        XCTAssertEqual(message(d, 9601)?["poll"], ["question": "Where?", "options": ["A", "B"], "multi": true, "anonymous": true])

        // in a thread
        XCTAssertTrue(Edits.sendPoll(&d, channel: 301, parent: 311, question: "When?", options: ["Now", "Later"], multi: false, anonymous: false, me: 1, id: 9602, ts: 52))
        let v = view(d, 9602)!
        XCTAssertEqual(v.poll?.question, "When?")
        XCTAssertTrue(WorkspaceView(d).channels.first { $0.id == 301 }!.messages.first { $0.id == 311 }!.replies.contains { $0.id == 9602 })

        // not without a question and two choices, not more than ten
        XCTAssertFalse(Edits.sendPoll(&d, channel: 301, question: "", options: ["A", "B"], multi: false, anonymous: false, me: 1, id: 9603, ts: 53))
        XCTAssertFalse(Edits.sendPoll(&d, channel: 301, question: "Q", options: ["A", ""], multi: false, anonymous: false, me: 1, id: 9603, ts: 53))
        XCTAssertFalse(Edits.sendPoll(&d, channel: 301, question: "Q", options: (0..<11).map(String.init), multi: false, anonymous: false, me: 1, id: 9603, ts: 53))
        XCTAssertNil(message(d, 9603))
    }

    func testVotesWriteOnlyMyEntry() {
        var d = SampleWorkspace.make()
        Edits.sendPoll(&d, channel: 301, question: "Lunch?", options: ["Pizza", "Noodles", "Salad"], multi: false, anonymous: false, me: 1, id: 9610, ts: 50)
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9610, option: 0, me: 2))
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9610, option: 1, me: 1))
        XCTAssertEqual(message(d, 9610)?["votes"], ["1": [1], "2": [0]])
        let before = message(d, 9610)?["votes"]?["2"]

        // my vote moves; Sam's stays exactly as it was
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9610, option: 2, me: 1))
        XCTAssertEqual(message(d, 9610)?["votes"], ["1": [2], "2": [0]])
        XCTAssertEqual(message(d, 9610)?["votes"]?["2"], before)

        // taken back: my key goes, only mine
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9610, option: 2, me: 1))
        XCTAssertEqual(message(d, 9610)?["votes"], ["2": [0]])
        // the last one taken back: no votes key at all
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9610, option: 0, me: 2))
        XCTAssertNil(message(d, 9610)?["votes"])

        // the view
        Edits.vote(&d, channel: 301, message: 9610, option: 1, me: 1)
        Edits.vote(&d, channel: 301, message: 9610, option: 1, me: 3)
        let v = view(d, 9610)!
        XCTAssertEqual(v.pollCounts, [0, 2, 0])
        XCTAssertEqual(v.voters, 2)
        XCTAssertEqual(v.picks(of: 1), [1])
        XCTAssertEqual(v.voters(for: 1), [1, 3])
        XCTAssertFalse(v.isEditable(by: 1), "a poll has no Edit")
        XCTAssertFalse(Edits.editMessage(&d, channel: 301, message: 9610, text: "changed", me: 1, ts: 60))

        // not a poll, no such choice
        XCTAssertFalse(Edits.vote(&d, channel: 301, message: 313, option: 0, me: 1))
        XCTAssertFalse(Edits.vote(&d, channel: 301, message: 9610, option: 3, me: 1))
    }

    func testSeveralChoicesAndVotesInAThread() {
        var d = SampleWorkspace.make()
        Edits.sendPoll(&d, channel: 301, parent: 311, question: "Days?", options: ["Mon", "Tue", "Wed"], multi: true, anonymous: false, me: 2, id: 9620, ts: 50)
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9620, option: 2, me: 1))
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9620, option: 0, me: 1))
        XCTAssertEqual(message(d, 9620)?["votes"], ["1": [0, 2]], "kept in order")
    }

    func testOnlyItsAuthorClosesAndClosedTakesNoVotes() {
        var d = SampleWorkspace.make()
        Edits.sendPoll(&d, channel: 301, question: "Lunch?", options: ["Pizza", "Noodles"], multi: false, anonymous: true, me: 1, id: 9630, ts: 50)
        XCTAssertFalse(Edits.closePoll(&d, channel: 301, message: 9630, close: true, me: 2, ts: 70), "not someone else's")
        XCTAssertTrue(Edits.closePoll(&d, channel: 301, message: 9630, close: true, me: 1, ts: 70))
        XCTAssertEqual(message(d, 9630)?["poll"], ["question": "Lunch?", "options": ["Pizza", "Noodles"], "anonymous": true, "closed": 70])
        XCTAssertTrue(view(d, 9630)!.poll!.isClosed)
        XCTAssertFalse(Edits.vote(&d, channel: 301, message: 9630, option: 0, me: 2), "closed")
        XCTAssertFalse(Edits.closePoll(&d, channel: 301, message: 9630, close: true, me: 1, ts: 80), "already closed")
        XCTAssertTrue(Edits.closePoll(&d, channel: 301, message: 9630, close: false, me: 1, ts: 90))
        XCTAssertEqual(message(d, 9630)?["poll"], ["question": "Lunch?", "options": ["Pizza", "Noodles"], "anonymous": true], "reopened: closed goes")
        XCTAssertTrue(Edits.vote(&d, channel: 301, message: 9630, option: 0, me: 2))
    }

    func testSlashPoll() {
        XCTAssertEqual(SlashCommands.parse("/poll Lunch today?", commands: []), .poll("Lunch today?"))
        XCTAssertEqual(SlashCommands.parse("/poll", commands: []), .poll(""))
        XCTAssertEqual(SlashCommands.choices("/po", commands: []).map(\.0), ["poll"])
    }

    /// The poll message of the shared golden workspace (the Mac and the
    /// server read the same file) reads as a poll here.
    func testGoldenWorkspacePollReads() throws {
        let golden = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("crates/wb-sync/tests/golden/workspace.json")
        let d = try JSON(parsing: Data(contentsOf: golden))
        let m = try XCTUnwrap(WorkspaceView(d).channels.flatMap(\.messages).first { $0.id == 3004 })
        let p = try XCTUnwrap(m.poll)
        XCTAssertEqual(p.question, "점심?")
        XCTAssertEqual(p.options, ["피자", "국수"])
        XCTAssertTrue(p.multi)
        XCTAssertFalse(p.anonymous)
        XCTAssertFalse(p.isClosed)
        XCTAssertEqual(m.votes, [2: [0, 1], 10: [1]])
        XCTAssertEqual(m.pollCounts, [1, 2])
    }
}
