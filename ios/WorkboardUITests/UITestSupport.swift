// What every UI test shares: the fake servers' addresses, the launch
// arguments, a base class that launches the app and waits for controls, and
// helpers for the test-only admin HTTP and the sign-in screens.

import XCTest

/// The fake servers from docs/ios-design.md (start them from ios/WorkboardKit).
/// swift run wb-fake-server --port 18787
let openServer = "ws://127.0.0.1:18787"
/// swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
let signInServer = "ws://127.0.0.1:18788"
/// swift run wb-fake-server --port 18789 --admin-http
let adminServer = "ws://127.0.0.1:18789"

/// Added to every launch: no saved or strong passwords offered, so iOS's
/// "Use Strong Password" sheet never takes over the keyboard mid-typing.
/// Every UI test runs the app in English, whatever language the simulator
/// is set to (KoreanUITests checks the Korean screens).
let testArgs = ["-wbNoPasswordAutoFill", "YES", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]

/// Waits for a row of a long form (Settings), scrolling down to it: rows
/// below the screen aren't made until they come into view.
func reveal(_ e: XCUIElement, in a: XCUIApplication, timeout: TimeInterval = 5) -> Bool {
    // clear of the bars at the top and the bottom
    let top = a.frame.height * 0.15, bottom = a.frame.height * 0.9
    func shown() -> Bool {
        e.exists && e.isHittable && e.frame.minY > top && e.frame.maxY < bottom
    }
    if e.waitForExistence(timeout: timeout) && shown() { return true }
    // short drags: a whole swipe flung rows past the screen and the next
    // one past it the other way (Settings' Sign out button)
    func drag(_ from: CGFloat, _ to: CGFloat) {
        a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
            .press(forDuration: 0.05, thenDragTo: a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)))
    }
    for i in 0..<30 where !shown() {
        if e.exists && e.frame.minY <= top {
            drag(0.4, 0.6) // above: back up
        } else if e.exists || i < 15 {
            drag(0.75, 0.45) // below, or not made yet: further down
        } else {
            drag(0.4, 0.7) // not found going down: further up
        }
    }
    // let the scroll come to rest, or a tap lands where the row was
    usleep(800_000)
    return shown()
}

extension XCTestCase {
    /// Test-only admin call on the sign-in server (blocking; UI tests run on the main thread).
    @discardableResult
    func admin(_ path: String) -> [String: Any] { admin(port: 18788, path) }

    /// Test-only admin call on a fake server started with --admin-http.
    @discardableResult
    func admin(port: Int, _ path: String) -> [String: Any] {
        var req = URLRequest(url: URL(string: "http://127.0.0.1:\(port)/admin/\(path)")!)
        req.httpMethod = "POST"
        var out: [String: Any] = [:]
        let done = expectation(description: "admin \(path)")
        URLSession.shared.dataTask(with: req) { data, _, _ in
            out = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 10)
        return out
    }

    /// A message from someone else (test-only HTTP on the 18789 server).
    func say(_ text: String, channel: Int, author: Int = 2) {
        var c = URLComponents(string: "http://127.0.0.1:18789/admin/say")!
        c.queryItems = [.init(name: "channel", value: "\(channel)"), .init(name: "author", value: "\(author)"), .init(name: "text", value: text)]
        var req = URLRequest(url: c.url!)
        req.httpMethod = "POST"
        let done = expectation(description: "say")
        URLSession.shared.dataTask(with: req) { _, _, _ in done.fulfill() }.resume()
        wait(for: [done], timeout: 10)
    }

    func waitLabel(_ e: XCUIElement, contains s: String, timeout: TimeInterval = 8, file: StaticString = #filePath, line: UInt = #line) {
        let ok = XCTWaiter().wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", s), object: e)], timeout: timeout)
        XCTAssertEqual(ok, .completed, "\"\(s)\" not in \"\(e.exists ? e.label : "(missing)")\"", file: file, line: line)
    }

    func type(_ field: XCUIElement, _ text: String) {
        XCTAssertTrue(field.waitForExistence(timeout: 5), "\(field) missing")
        field.tap()
        if let old = field.value as? String, !old.isEmpty, old != field.placeholderValue {
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count))
        }
        guard field.elementType == .secureTextField, !text.isEmpty else {
            field.typeText(text)
            return
        }
        // launched with -wbNoPasswordAutoFill (testArgs), the app asks for
        // no strong password, so the typing lands whole; wait until the
        // field shows every character. Should iOS still offer one, close it
        // once it is up and type again.
        field.typeText(text)
        let app = XCUIApplication()
        func count() -> Int { ((field.value as? String).flatMap { $0 == field.placeholderValue ? nil : $0 } ?? "").count }
        let full = NSPredicate { _, _ in count() == text.count }
        for _ in 0..<3 {
            if XCTWaiter().wait(for: [XCTNSPredicateExpectation(predicate: full, object: field)], timeout: 3) == .completed { return }
            let offer = app.buttons["GenerateStrongPasswordButton"]
            if offer.exists {
                app.buttons["xmark"].tap()
                let closed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: offer)
                _ = XCTWaiter().wait(for: [closed], timeout: 3)
            }
            if !((field.value(forKey: "hasKeyboardFocus") as? Bool) ?? false) { field.tap() }
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: max(count(), 1)) + text)
        }
        XCTFail("could not type into \(field)")
    }

    /// Join with a fresh invite code from the sign-in screen, choosing a password.
    func join(_ a: XCUIApplication, member: Int, email: String? = nil, password: String) {
        let code = admin("invite?member=\(member)")["code"] as! String
        let toInvite = a.buttons["inviteModeButton"]
        XCTAssertTrue(toInvite.waitForExistence(timeout: 10), "sign-in screen")
        toInvite.tap()
        type(a.textFields["inviteField"], code)
        if let email { type(a.textFields["inviteEmailField"], email) }
        type(a.secureTextFields["newPasswordField"], password)
        type(a.secureTextFields["confirmPasswordField"], password)
        a.buttons["joinButton"].tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in")
    }

    func signOut(_ a: XCUIApplication) {
        a.buttons["settingsButton"].tap()
        let out = a.buttons["signOutButton"]
        XCTAssertTrue(reveal(out, in: a))
        out.tap()
        XCTAssertTrue(a.textFields["emailField"].waitForExistence(timeout: 10), "back on the sign-in screen")
    }
}

/// A test that stops at its first failure, with helpers to launch the app.
class WorkboardUITestCase: XCTestCase {
    /// Back to Home, one "<" at a time.
    func goHome(_ a: XCUIApplication) {
        let home = a.buttons["quickFind"]
        for _ in 0..<6 where !home.waitForExistence(timeout: 1) {
            a.navigationBars.buttons.element(boundBy: 0).tap()
        }
        XCTAssertTrue(home.waitForExistence(timeout: 5), "on Home")
    }

    /// Opens a place (inbox, today, upcoming, chats, docs, tools) from Home.
    func openPlace(_ a: XCUIApplication, _ place: String) {
        goHome(a)
        let row = a.buttons["place-\(place)"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
    }

    /// Unfolds a task's More details (tags, dates, fields, links, files, time).
    func showTaskDetails(_ a: XCUIApplication) {
        let toggle = a.buttons["taskDetailsToggle"]
        scrollTo(toggle, in: a)
        if toggle.value as? String != "Shown" { toggle.tap() }
        XCTAssertTrue(wait(for: toggle, value: "Shown"))
    }

    private func wait(for e: XCUIElement, value: String) -> Bool {
        let done = expectation(for: NSPredicate(format: "value == %@", value), evaluatedWith: e)
        return XCTWaiter().wait(for: [done], timeout: 5) == .completed
    }

    override func setUp() { continueAfterFailure = false }

    /// Launches the app with these arguments (and `testArgs`).
    func launchApp(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args + testArgs
        a.launch()
        return a
    }

    /// Launches the app from a fresh start on `server`, as member 1.
    func launchFresh(_ extra: [String] = [], server: String = openServer) -> XCUIApplication {
        launchApp(["-wbReset", "YES", "-wbServer", server, "-wbMe", "1"] + extra)
    }

    /// Waits for `e` to be there and enabled (buttons wait for the first sync).
    func waitEnabled(_ e: XCUIElement) {
        XCTAssertTrue(e.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: e)
        wait(for: [enabled], timeout: 10)
    }

    /// Scrolls the form up until `e` is there (rows below the fold aren't made yet).
    func scrollTo(_ e: XCUIElement, in a: XCUIApplication) {
        var swiped = false
        // a row scrolled under the navigation bar still counts as hittable,
        // but a tap on it lands on the bar (the Repeat menu never opened)
        let bar = a.navigationBars.firstMatch
        let top = bar.exists ? bar.frame.maxY : 0
        // until it is below the bar and well clear of the comment box at the
        // bottom; once it is on screen, small drags, so a whole swipe doesn't
        // carry it past the top
        for i in 0..<24 where !(e.exists && e.isHittable && e.frame.minY >= top && e.frame.maxY < a.frame.height * 0.7) {
            if e.exists && (e.frame.minY < top || e.frame.maxY < a.frame.height * 0.15) {
                // above: back down a little
                let from = a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4))
                from.press(forDuration: 0.05, thenDragTo: a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.6)))
            } else if !e.exists && i >= 10 {
                // not found going down: it is further up
                a.swipeDown()
            } else if e.exists && e.frame.minY > 0 && e.frame.minY < a.frame.height {
                let from = a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.75))
                from.press(forDuration: 0.05, thenDragTo: a.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.55)))
            } else {
                a.swipeUp()
            }
            swiped = true
        }
        XCTAssertTrue(e.waitForExistence(timeout: 5))
        // let the scroll come to rest, or a tap lands where the row was
        if swiped { usleep(800_000) }
    }
}
