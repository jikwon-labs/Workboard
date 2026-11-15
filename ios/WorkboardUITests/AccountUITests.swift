// Signing in on a server that wants it, the way the real server does it:
// invite code with a new password, email and password on another device,
// a wrong password, "Forgot password?" with a reset code, setting a password
// from settings, and invite links (workboard://join?...).
// Needs: swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
// (from ios/WorkboardKit). The server keeps accounts between tests, so each
// test starts by joining with a fresh invite code, which replaces the account.

import XCTest

let signInServer = "ws://127.0.0.1:18788"
/// Added to every launch: no saved or strong passwords offered, so iOS's
/// "Use Strong Password" sheet never takes over the keyboard mid-typing.
let testArgs = ["-wbNoPasswordAutoFill", "YES"]

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
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10), "signed in")
    }

    func signOut(_ a: XCUIApplication) {
        a.buttons["settingsButton"].tap()
        let out = a.buttons["signOutButton"]
        XCTAssertTrue(out.waitForExistence(timeout: 5))
        out.tap()
        XCTAssertTrue(a.textFields["emailField"].waitForExistence(timeout: 10), "back on the sign-in screen")
    }
}

final class AccountUITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func app(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args + testArgs
        a.launch()
        return a
    }

    func testInviteCodeWithPasswordThenEmailSignInOnAnotherDevice() {
        let a = app(["-wbReset", "YES", "-wbServer", signInServer])
        XCTAssertTrue(a.textFields["emailField"].waitForExistence(timeout: 10), "a new device is asked to sign in")
        a.buttons["inviteModeButton"].tap()

        // a wrong code first
        type(a.textFields["inviteField"], "NOPE-NOPE-NOPE")
        type(a.secureTextFields["newPasswordField"], "sam-password-1")
        type(a.secureTextFields["confirmPasswordField"], "sam-password-1")
        a.buttons["joinButton"].tap()
        waitLabel(a.staticTexts["signInError"], contains: "invite code doesn't work")

        // the right one; Sam's email comes from the workspace
        let code = admin("invite?member=3")["code"] as! String
        type(a.textFields["inviteField"], code)
        a.buttons["joinButton"].tap()
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10), "signed in as the invited member; no member picker")
        a.buttons["settingsButton"].tap()
        waitLabel(a.staticTexts["signedInAs"], contains: "Signed in as Sam Lee")
        waitLabel(a.staticTexts["accountEmail"], contains: "sam@example.com")
        a.buttons["Done"].tap()

        // signing out forgets the token: like another device from here on
        signOut(a)
        type(a.textFields["emailField"], "sam@example.com")
        type(a.secureTextFields["passwordField"], "not-the-password")
        a.buttons["signInButton"].tap()
        waitLabel(a.staticTexts["signInError"], contains: "That email and password don't match.")
        type(a.secureTextFields["passwordField"], "sam-password-1")
        a.buttons["signInButton"].tap()
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10), "signed in with email and password")
    }

    func testForgotPasswordWithResetCode() {
        let a = app(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 3, password: "forgotten-pass-1")
        signOut(a)

        a.buttons["forgotButton"].tap()
        type(a.textFields["forgotEmailField"], "sam@example.com")
        a.buttons["sendCodeButton"].tap()
        let codeField = a.textFields["resetCodeField"]
        XCTAssertTrue(codeField.waitForExistence(timeout: 10), "code form after asking")
        waitLabel(a.staticTexts["forgotInfo"], contains: "a code is on its way")
        let code = admin("reset-code?email=sam@example.com")["code"] as! String
        type(codeField, code)
        type(a.secureTextFields["resetPasswordField"], "remembered-pass-2")
        type(a.secureTextFields["resetConfirmField"], "remembered-pass-2")
        a.buttons["resetButton"].tap()
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10), "signed in with the new password")

        // and the new password is the one that works now
        signOut(a)
        type(a.textFields["emailField"], "sam@example.com")
        type(a.secureTextFields["passwordField"], "remembered-pass-2")
        a.buttons["signInButton"].tap()
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10))
    }

    func testSetAndChangePasswordFromSettings() {
        var a = app(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 2, email: "alex@example.test", password: "alex-password-1")
        // as if this device had been signed in before passwords existed
        admin("drop-account?member=2")
        a.terminate()
        a = app(["-wbServer", signInServer])
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10), "signed in with the saved token")
        a.buttons["settingsButton"].tap()
        waitLabel(a.staticTexts["accountEmail"], contains: "No password yet")
        let link = a.buttons["passwordLink"]
        waitLabel(link, contains: "Set a password")
        link.tap()
        type(a.textFields["accountEmailField"], "alex@example.test")
        type(a.secureTextFields["accountPasswordField"], "alex-password-2")
        type(a.secureTextFields["accountConfirmField"], "alex-password-2")
        a.buttons["savePasswordButton"].tap()
        XCTAssertTrue(a.staticTexts["passwordSaved"].waitForExistence(timeout: 10))
        a.navigationBars.buttons.element(boundBy: 0).tap()
        waitLabel(a.staticTexts["accountEmail"], contains: "alex@example.test")

        // changing it needs the current password
        waitLabel(link, contains: "Change password")
        link.tap()
        type(a.secureTextFields["oldPasswordField"], "wrong-password")
        type(a.secureTextFields["accountPasswordField"], "alex-password-3")
        type(a.secureTextFields["accountConfirmField"], "alex-password-3")
        a.buttons["savePasswordButton"].tap()
        waitLabel(a.staticTexts["passwordError"], contains: "current password isn't right")
        type(a.secureTextFields["oldPasswordField"], "alex-password-2")
        a.buttons["savePasswordButton"].tap()
        XCTAssertTrue(a.staticTexts["passwordSaved"].waitForExistence(timeout: 10))
    }

    func testInviteLinkOpensTheJoinForm() {
        let code = admin("invite?member=2")["code"] as! String
        let a = app(["-wbReset", "YES"])
        XCTAssertTrue(a.textFields["serverField"].waitForExistence(timeout: 10), "first launch, no server yet")
        a.open(URL(string: "workboard://join?server=127.0.0.1%3A18788&code=\(code)&email=alex%40example.test")!)
        let field = a.textFields["inviteField"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "the link set the server and opened the invite form")
        XCTAssertEqual(field.value as? String, code)
        XCTAssertEqual(a.textFields["inviteEmailField"].value as? String, "alex@example.test")
        type(a.secureTextFields["newPasswordField"], "alex-password-1")
        type(a.secureTextFields["confirmPasswordField"], "alex-password-1")
        a.buttons["joinButton"].tap()
        XCTAssertTrue(a.navigationBars["My Tasks"].waitForExistence(timeout: 10))
    }

    func testPastedInviteEmailFillsTheForm() {
        let code = admin("invite?member=2")["code"] as! String
        let a = app(["-wbReset", "YES", "-wbServer", signInServer])
        let toInvite = a.buttons["inviteModeButton"]
        XCTAssertTrue(toInvite.waitForExistence(timeout: 10))
        toInvite.tap()
        let field = a.textFields["inviteField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        UIPasteboard.general.string = "Jamie invited you. Open workboard://join?server=127.0.0.1:18788&code=\(code)&email=alex%40example.test on your iPhone."
        field.tap()
        field.press(forDuration: 1.2)
        let paste = a.menuItems["Paste"].exists ? a.menuItems["Paste"] : a.buttons["Paste"]
        XCTAssertTrue(paste.waitForExistence(timeout: 5), "edit menu")
        paste.tap()
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow Paste"]
        if allow.waitForExistence(timeout: 2) { allow.tap() }
        let filled = XCTWaiter().wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", code), object: field)], timeout: 5)
        XCTAssertEqual(filled, .completed, "code taken out of the pasted text, got \(field.value ?? "nil")")
        XCTAssertEqual(a.textFields["inviteEmailField"].value as? String, "alex@example.test")
    }
}
