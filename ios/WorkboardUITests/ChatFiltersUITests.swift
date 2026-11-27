// UI tests for a conversation's filters (Replies, Files, Links...), its
// files gallery, and has:/from: in quick find. Needs the fake servers:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class ChatFiltersUITests: WorkboardUITestCase {
    func pick(_ a: XCUIApplication, _ filter: String) {
        let menu = a.buttons["filterMessagesButton"]
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        menu.tap()
        let item = a.buttons[filter]
        XCTAssertTrue(item.waitForExistence(timeout: 5), "\(filter) in the filter menu")
        item.tap()
    }

    /// A message from Alex on the 18789 server, with a file (&kind=Image:
    /// a picture) or as a reply in a thread.
    func send(_ text: String, channel: Int, file: String? = nil, picture: Bool = false, reply: Int? = nil, post: String? = nil) {
        var q = "channel=\(channel)&author=2&text=\(text.addingPercentEncoding(withAllowedCharacters: .alphanumerics)!)"
        if let file { q += "&file=\(file)" + (picture ? "&kind=Image" : "") }
        if let reply { q += "&reply=\(reply)" }
        if let post { q += "&post=\(post.addingPercentEncoding(withAllowedCharacters: .alphanumerics)!)" }
        admin(port: 18789, "say?" + q)
    }

    /// Replies keeps only messages with a thread; the banner names the
    /// filter and its x goes back to all. Links and Files say when the
    /// conversation has none.
    func testFiltersNarrowAndClear() {
        let a = launchFresh(["-wbOpen", "channel:301"])
        XCTAssertTrue(a.descendants(matching: .any)["message-313"].waitForExistence(timeout: 10))
        pick(a, "Replies")
        let banner = a.staticTexts["filterBanner"]
        XCTAssertTrue(banner.waitForExistence(timeout: 5))
        XCTAssertEqual(banner.label, "Messages with replies")
        XCTAssertTrue(a.descendants(matching: .any)["message-311"].waitForExistence(timeout: 5), "the message with replies stays")
        XCTAssertFalse(a.descendants(matching: .any)["message-313"].exists, "one without replies goes")
        a.buttons["clearFilterButton"].tap()
        XCTAssertTrue(a.descendants(matching: .any)["message-313"].waitForExistence(timeout: 5), "all messages again")
        XCTAssertFalse(banner.exists)

        // a direct message with no links or files (no test writes there)
        let b = launchFresh(["-wbOpen", "channel:303"])
        pick(b, "Links")
        XCTAssertTrue(b.descendants(matching: .any)["noLinksNote"].waitForExistence(timeout: 5))
        XCTAssertTrue(b.staticTexts["No links in this conversation yet"].exists)
        XCTAssertEqual(b.staticTexts["filterBanner"].label, "Messages with links")
        pick(b, "Files")
        XCTAssertTrue(b.staticTexts["No files in this conversation yet"].waitForExistence(timeout: 5))
        XCTAssertFalse(b.descendants(matching: .any)["message-331"].exists, "the gallery, not the messages")
    }

    /// The newest announcement shows over the messages: Show all lists only
    /// the announcements, tapping it goes to the post, the x hides it.
    func testNewestAnnouncementOverTheMessages() {
        let tag = "\(Int.random(in: 100000...999999))"
        send("plain \(tag)", channel: 303)
        send("Read before Friday", channel: 303, post: "Office move \(tag)")
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)
        let banner = a.buttons["announcementBanner"]
        XCTAssertTrue(banner.waitForExistence(timeout: 10))
        XCTAssertTrue(banner.label.contains("Office move \(tag)"), banner.label)
        let plain = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "plain \(tag)")).firstMatch
        XCTAssertTrue(plain.waitForExistence(timeout: 5))

        a.buttons["allAnnouncementsButton"].tap()
        let filterBanner = a.staticTexts["filterBanner"]
        XCTAssertTrue(filterBanner.waitForExistence(timeout: 5))
        XCTAssertEqual(filterBanner.label, "Announcements")
        XCTAssertFalse(plain.exists, "only the announcements")
        XCTAssertFalse(banner.exists, "the filter's banner in its place")
        a.buttons["clearFilterButton"].tap()
        XCTAssertTrue(banner.waitForExistence(timeout: 5))
        banner.tap()
        a.buttons["hideAnnouncementButton"].tap()
        XCTAssertFalse(banner.waitForExistence(timeout: 2), "hidden")
        // the filter menu has it too
        pick(a, "Announcements")
        XCTAssertEqual(a.staticTexts["filterBanner"].label, "Announcements")
    }

    /// Files shows pictures in a grid and other files in a list, with those
    /// sent in thread replies; each goes to its message. Links keeps
    /// messages with a web address.
    func testFilesGalleryAndLinks() {
        let tag = "\(Int.random(in: 100000...999999))"
        send("plain \(tag)", channel: 303)
        send("see https://example.com/\(tag)", channel: 303)
        send("notes \(tag)", channel: 303, file: "notes-\(tag).pdf")
        send("", channel: 303, file: "cat-\(tag).png", picture: true, reply: 331)
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)
        let plain = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "plain \(tag)")).firstMatch
        XCTAssertTrue(plain.waitForExistence(timeout: 10))

        pick(a, "Links")
        let link = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "example.com/\(tag)")).firstMatch
        XCTAssertTrue(link.waitForExistence(timeout: 5), "the message with a link stays")
        XCTAssertFalse(plain.exists, "a message without one goes")

        pick(a, "Files")
        let picture = a.buttons["Picture cat-\(tag).png"]
        XCTAssertTrue(picture.waitForExistence(timeout: 5), "a picture sent in a thread reply is in the grid")
        a.segmentedControls["filesPart"].buttons["Other files"].tap()
        let file = a.buttons["notes-\(tag).pdf"]
        XCTAssertTrue(file.waitForExistence(timeout: 5), "the PDF is under Other files")
        file.press(forDuration: 1.0)
        let show = a.buttons["Show in conversation"]
        XCTAssertTrue(show.waitForExistence(timeout: 5))
        show.tap()
        XCTAssertFalse(a.staticTexts["filterBanner"].waitForExistence(timeout: 2), "back to all messages")
        let sent = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "notes \(tag)")).firstMatch
        XCTAssertTrue(sent.waitForExistence(timeout: 5))

        pick(a, "Files")
        XCTAssertTrue(picture.waitForExistence(timeout: 5))
        picture.press(forDuration: 1.0)
        XCTAssertTrue(show.waitForExistence(timeout: 5))
        show.tap()
        XCTAssertTrue(a.navigationBars["Thread"].waitForExistence(timeout: 5), "a reply's file goes to its thread")
    }

    /// has:file, has:link and from:name narrow the messages found, and
    /// only messages show while one is typed.
    func testSearchOperators() {
        let tag = "\(Int.random(in: 100000...999999))"
        send("report \(tag)", channel: 303, file: "report-\(tag).pdf")
        send("site \(tag) https://example.com/s\(tag)", channel: 303)
        let a = launchFresh(["-wbOpen", "search"], server: adminServer)
        let field = a.searchFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        XCTAssertTrue(a.staticTexts["searchHint"].waitForExistence(timeout: 5), "the hint while the search is empty")
        field.tap()
        func find(_ s: String) {
            field.tap()
            if let old = field.value as? String, !old.isEmpty, old != field.placeholderValue {
                field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count))
            }
            field.typeText(s)
        }
        func result(_ text: String) -> XCUIElement {
            a.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'result-message-' AND label CONTAINS %@", text)).firstMatch
        }
        find("has:file \(tag)")
        XCTAssertTrue(result("report \(tag)").waitForExistence(timeout: 5))
        XCTAssertFalse(result("site \(tag)").exists)
        find("HAS:LINK \(tag)")
        XCTAssertTrue(result("site \(tag)").waitForExistence(timeout: 5))
        XCTAssertFalse(result("report \(tag)").exists)
        find("from:alex \(tag)")
        XCTAssertTrue(result("report \(tag)").waitForExistence(timeout: 5))
        XCTAssertTrue(result("site \(tag)").exists)
        // the launch task matches "launch", but with an operator only messages show
        find("launch has:file")
        XCTAssertTrue(a.staticTexts["noResults"].waitForExistence(timeout: 5))
        XCTAssertFalse(a.buttons["result-task-201"].exists)
        XCTAssertTrue(a.staticTexts["searchHint"].exists, "the hint with no results")
        find("from:sam \(tag)")
        XCTAssertTrue(a.staticTexts["noResults"].waitForExistence(timeout: 5))
    }
}
