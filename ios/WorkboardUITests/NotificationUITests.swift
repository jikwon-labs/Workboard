// Notifications: permission, a banner for a message that arrives while
// another screen is open, and tapping the banner to open the conversation.
// Needs: swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class NotificationUITests: XCTestCase {
    let server = "ws://127.0.0.1:18789"

    override func setUp() { continueAfterFailure = false }

    func say(_ text: String, channel: Int = 301, author: Int = 2) {
        var c = URLComponents(string: "http://127.0.0.1:18789/admin/say")!
        c.queryItems = [.init(name: "channel", value: "\(channel)"), .init(name: "author", value: "\(author)"), .init(name: "text", value: text)]
        var req = URLRequest(url: c.url!)
        req.httpMethod = "POST"
        let done = expectation(description: "say")
        URLSession.shared.dataTask(with: req) { _, _, _ in done.fulfill() }.resume()
        wait(for: [done], timeout: 10)
    }

    func testBannerForNewMessageOpensConversation() {
        let app = XCUIApplication()
        app.launchArguments = ["-wbReset", "YES", "-wbServer", server, "-wbMe", "1", "-wbTab", "chat"]
        app.launch()
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

        // turn on notifications from the card in the chat list (once per install)
        let card = app.buttons["notifyCardButton"]
        if card.waitForExistence(timeout: 8) {
            card.tap()
            let allow = springboard.buttons["Allow"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
        }
        // look at another tab, then someone writes in #general
        app.tabBars.buttons["Home"].tap()
        XCTAssertTrue(app.navigationBars["My Tasks"].waitForExistence(timeout: 5))
        let text = "Banner check \(Int(Date().timeIntervalSince1970) % 100000)"
        say(text)
        let banner = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10), "banner shown")
        banner.tap()
        XCTAssertTrue(app.navigationBars["#general"].waitForExistence(timeout: 8), "tapping opens the conversation")
        XCTAssertTrue(app.staticTexts[text].exists)
    }
}
