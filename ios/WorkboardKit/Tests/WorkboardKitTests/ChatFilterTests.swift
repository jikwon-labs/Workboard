import XCTest
@testable import WorkboardKit

final class ChatFilterTests: XCTestCase {
    let file: JSON = ["file": .string(String(repeating: "a", count: 64)), "name": "notes.pdf", "kind": "File", "bytes": 10, "w": 0, "h": 0, "secs": 0.0]
    let picture: JSON = ["file": .string(String(repeating: "b", count: 64)), "name": "cat.png", "kind": "Image", "bytes": 10, "w": 4, "h": 3, "secs": 0.0]

    func msg(_ id: UInt64, _ text: String, author: UInt64 = 1, ts: Int64 = 0, extra: [String: JSON] = [:]) -> JSON {
        var o: [String: JSON] = ["id": .uint(id), "author": .uint(author), "text": .string(text), "ts": .int(ts), "replies": []]
        for (k, v) in extra { o[k] = v }
        return .object(o)
    }

    func testLinksLikeTheMac() {
        XCTAssertTrue(MessageView(msg(1, "see https://ex.example/a")).hasLink)
        XCTAssertTrue(MessageView(msg(1, "[doc](http://d.example)")).hasLink)
        XCTAssertFalse(MessageView(msg(1, "`https://code.example`")).hasLink, "not inside code")
        XCTAssertFalse(MessageView(msg(1, "ftp://x.example and example.com")).hasLink)
        XCTAssertTrue(MessageView(msg(1, "no address", extra: ["preview": ["url": "https://p.example"]])).hasLink, "a link preview counts")
    }

    func testFiltersKeepTheMessagesTheMacKeeps() {
        let plain = MessageView(msg(1, "hi"))
        let replied = MessageView(msg(2, "q", extra: ["replies": [msg(20, "a")]]))
        let pinned = MessageView(msg(3, "p", extra: ["pinned": true]))
        let saved = MessageView(msg(4, "s", extra: ["saved": true]))
        let savedReply = MessageView(msg(5, "q", extra: ["replies": [msg(50, "a", extra: ["saved": true])]]))
        let task = MessageView(msg(6, "t", extra: ["task": 201]))
        let withFile = MessageView(msg(7, "", extra: ["attachments": [file]]))
        let fileInReply = MessageView(msg(8, "q", extra: ["replies": [msg(80, "here", extra: ["attachments": [picture]])]]))
        let link = MessageView(msg(9, "https://x.example"))
        let linkInReply = MessageView(msg(10, "q", extra: ["replies": [msg(100, "https://y.example")]]))
        let all = [plain, replied, pinned, saved, savedReply, task, withFile, fileInReply, link, linkInReply]
        func ids(_ f: MessageFilter) -> [UInt64] { all.filter(f.keeps).map(\.id) }
        XCTAssertEqual(ids(.all).count, all.count)
        XCTAssertEqual(ids(.replies), [2, 5, 8, 10])
        XCTAssertEqual(ids(.pinned), [3])
        XCTAssertEqual(ids(.saved), [4, 5])
        XCTAssertEqual(ids(.tasks), [6])
        XCTAssertEqual(ids(.files), [7, 8], "a file in a thread reply shows its parent")
        XCTAssertEqual(ids(.links), [9, 10])
    }

    func testAnnouncementsAreTheMacsAnnouncementPosts() {
        let plain = msg(1, "hi", ts: 10)
        let announcement = msg(2, "Read this", ts: 20, extra: ["post_title": "Office move", "post_type": "Announcement"])
        let old = msg(3, "From before types", ts: 30, extra: ["post_title": "Party", "post_type": ""])
        let idea = msg(4, "Coffee?", ts: 40, extra: ["post_title": "New coffee", "post_type": "Idea"])
        let all = [plain, announcement, old, idea].map(MessageView.init)
        XCTAssertEqual(all.filter(MessageFilter.announcements.keeps).map(\.id), [2, 3])
        let c = ChannelView(["id": 301, "name": "general", "kind": "Channel", "members": [1], "messages": [plain, announcement, old, idea]])
        XCTAssertEqual(c.latestAnnouncement?.id, 3, "the newest announcement, not the idea after it")
        XCTAssertNil(ChannelView(["id": 302, "name": "x", "kind": "Channel", "members": [1], "messages": [plain]]).latestAnnouncement)
    }

    func testFilesNewestFirstWithThreadReplies() {
        let c = ChannelView(["id": 301, "name": "general", "kind": "Channel", "members": [1, 2], "messages": [
            msg(1, "", ts: 100, extra: ["attachments": [file, picture]]),
            msg(2, "q", ts: 200, extra: ["replies": [msg(21, "", ts: 400, extra: ["attachments": [picture]])]]),
            msg(3, "", ts: 300, extra: ["attachments": [file]]),
        ]])
        let files = c.files
        XCTAssertEqual(files.map(\.message.id), [21, 3, 1, 1])
        XCTAssertEqual(files.first?.parent, 2)
        XCTAssertNil(files[1].parent)
        XCTAssertEqual(files[2].attachment.name, "notes.pdf", "in the order sent within one message")
        XCTAssertEqual(files.filter { !$0.attachment.isFile && !$0.attachment.isVideo }.count, 2)
    }

    func testSearchOperatorsAreReadOut() {
        var q = SearchQuery("  launch HAS:FILE from:Al  has:links From:me plan ")
        XCTAssertEqual(q.text, "launch plan")
        XCTAssertTrue(q.hasFile)
        XCTAssertTrue(q.hasLink)
        XCTAssertEqual(q.from, ["al", "me"])
        XCTAssertTrue(q.hasOperators)
        q = SearchQuery("has:files")
        XCTAssertEqual(q.text, "")
        XCTAssertTrue(q.hasFile)
        q = SearchQuery("plain words from:")
        XCTAssertFalse(q.hasOperators)
        XCTAssertEqual(q.text, "plain words from:")
    }

    func testSearchMatchesOperatorsAndText() {
        let members = [
            MemberView(["id": 1, "name": "Jamie Park"]),
            MemberView(["id": 2, "name": "Alex Kim", "display_name": "Lexi"]),
            MemberView(["id": 3, "name": "Mary Ann Lee"]),
        ]
        let fromAlex = MessageView(msg(10, "Launch notes https://x.example", author: 2))
        let mine = MessageView(msg(11, "Launch file", author: 1, extra: ["attachments": [file]]))
        let mary = MessageView(msg(12, "hello", author: 3))
        let bot = MessageView(msg(13, "Build passed", author: 1, extra: ["bot": "Deploy Bot"]))
        func hits(_ s: String) -> [UInt64] {
            let q = SearchQuery(s)
            return [fromAlex, mine, mary, bot].filter { q.matches($0, members: members, me: 1) }.map(\.id)
        }
        XCTAssertEqual(hits("has:file"), [11])
        XCTAssertEqual(hits("has:link"), [10])
        XCTAssertEqual(hits("from:me"), [11], "a bot's message goes by the bot")
        XCTAssertEqual(hits("from:alex"), [10])
        XCTAssertEqual(hits("from:kim"), [10], "any word of the name")
        XCTAssertEqual(hits("from:alexk"), [10], "the name without spaces")
        XCTAssertEqual(hits("from:lexi"), [10], "the display name too")
        XCTAssertEqual(hits("from:maryannl"), [12])
        XCTAssertEqual(hits("from:ann"), [12])
        XCTAssertEqual(hits("from:deploy"), [13])
        XCTAssertEqual(hits("from:bot"), [13])
        XCTAssertEqual(hits("from:jamie"), [11], "the bot's message isn't Jamie's")
        XCTAssertEqual(hits("from:alex from:mary"), [10, 12], "any of them")
        XCTAssertEqual(hits("launch from:me"), [11])
        XCTAssertEqual(hits("LAUNCH"), [10, 11])
        XCTAssertEqual(hits("from:zed"), [])
    }
}
