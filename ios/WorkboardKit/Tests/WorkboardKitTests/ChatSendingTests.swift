import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Forwarding, "Also send to the channel" and scheduled messages, as the
/// Mac app does them.
final class ChatSendingTests: XCTestCase {
    func channel(_ d: JSON, _ id: UInt64) -> ChannelView { WorkspaceView(d).channels.first { $0.id == id }! }

    func testForwardCarriesTheOriginalAndAForwardedForwardCarriesTheFirst() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 301, text: "the numbers are in", me: 2, id: 900, ts: 10)
        XCTAssertTrue(Edits.forward(&d, from: 301, message: 900, to: 302, note: " FYI @Alex Kim ", me: 1, id: 901, ts: 11))
        let f = channel(d, 302).messages.last!
        XCTAssertEqual(f.text, "FYI @Alex Kim")
        XCTAssertEqual(f.forwarded?.text, "the numbers are in")
        XCTAssertEqual(f.forwarded?.author, 2)
        XCTAssertEqual(f.forwarded?.channel, 301)
        XCTAssertTrue(Edits.forward(&d, from: 302, message: 901, to: 303, note: "", me: 1, id: 902, ts: 12))
        XCTAssertEqual(channel(d, 303).messages.last?.forwarded?.message, 900, "the first message, not the forward")
    }

    func testAReplyAlsoInTheChannelIsMarkedAndNotToldTwice() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 301, text: "plan", me: 2, id: 900, ts: 10)
        Edits.sendReply(&d, channel: 301, to: 900, text: "@Alex Kim done", me: 1, id: 901, ts: 11)
        XCTAssertTrue(Edits.copyReplyToChannel(&d, channel: 301, thread: 900, reply: 901, me: 1, id: 902))
        let copy = channel(d, 301).messages.last!
        XCTAssertEqual(copy.text, "@Alex Kim done")
        XCTAssertEqual(copy.threadOf, 900)
        XCTAssertFalse(Edits.copyReplyToChannel(&d, channel: 301, thread: 900, reply: 901, me: 2, id: 903), "only my own reply")
        let inbox = Inbox.items(d, me: 2).filter { $0.text == "@Alex Kim done" }
        XCTAssertEqual(inbox.count, 1, "the reply, not its copy")
    }

    func testScheduledItemsPostAndTravel() {
        var d = SampleWorkspace.make()
        let item = ScheduledItem(id: 950, channel: 301, text: "good morning", at: 99, mentions: [2])
        XCTAssertEqual(ScheduledItem(item.json), item)
        XCTAssertEqual(ClientMessage(text: ClientMessage.schedule(item: item.json).text), .schedule(item: item.json))
        XCTAssertEqual(ClientMessage(text: ClientMessage.unschedule(id: 950).text), .unschedule(id: 950))
        XCTAssertEqual(ClientMessage(text: ClientMessage.scheduledList.text), .scheduledList)
        XCTAssertEqual(ServerMessage(text: ServerMessage.scheduled([item.json]).text), .scheduled([item.json]))
        XCTAssertEqual(ClientMessage.schedule(item: item.json).json,
                       ["type": "schedule", "item": ["id": 950, "channel": 301, "text": "good morning", "at": 99, "mentions": [2]]])
        Edits.postScheduled(&d, item, me: 1, ts: 100)
        let m = channel(d, 301).messages.last!
        XCTAssertEqual(m.id, 950)
        XCTAssertEqual(m.text, "good morning")
        let reply = ScheduledItem(id: 951, channel: 301, parent: 950, text: "and you", at: 99)
        Edits.postScheduled(&d, reply, me: 2, ts: 101)
        XCTAssertEqual(channel(d, 301).messages.last?.replies.last?.text, "and you")
    }

    func testTheClockOffersAnHourTomorrowAndMonday() {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "Asia/Seoul")!
        // Wednesday 2026-11-18 15:00 in Seoul
        let wed = cal.date(from: DateComponents(year: 2026, month: 11, day: 18, hour: 15))!
        let c = ScheduleTimes.choices(now: wed, calendar: cal)
        XCTAssertEqual(c.map(\.0), ["In 1 hour", "Tomorrow at 9:00 AM", "Monday at 9:00 AM"])
        XCTAssertEqual(c[0].1, Int64(wed.timeIntervalSince1970) + 3600)
        let mon = Date(timeIntervalSince1970: TimeInterval(c[2].1))
        XCTAssertEqual(cal.component(.weekday, from: mon), 2)
        XCTAssertEqual(cal.component(.day, from: mon), 23)
        XCTAssertEqual(cal.component(.hour, from: mon), 9)
    }
}
