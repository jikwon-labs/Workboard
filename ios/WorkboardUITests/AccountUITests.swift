// Signing in on a server that wants it, the way the real server does it:
// invite code with a new password, email and password on another device,
// a wrong password, "Forgot password?" with a reset code, setting a password
// from settings, invite links (workboard://join?...), and the code a computer
// shows to sign a phone in (typed, or its workboard://link?... QR code,
// with a spinner until the workspace opens).
// Needs: swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev
// (from ios/WorkboardKit). The server keeps accounts between tests, so each
// test starts by joining with a fresh invite code, which replaces the account.

import Network
import XCTest

final class AccountUITests: WorkboardUITestCase {
    func testInviteCodeWithPasswordThenEmailSignInOnAnotherDevice() {
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
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
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in as the invited member; no member picker")
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
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in with email and password")
    }

    func testForgotPasswordWithResetCode() {
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
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
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in with the new password")

        // and the new password is the one that works now
        signOut(a)
        type(a.textFields["emailField"], "sam@example.com")
        type(a.secureTextFields["passwordField"], "remembered-pass-2")
        a.buttons["signInButton"].tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))
    }

    func testSetAndChangePasswordFromSettings() {
        var a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 2, email: "alex@example.test", password: "alex-password-1")
        // as if this device had been signed in before passwords existed
        admin("drop-account?member=2")
        a.terminate()
        a = launchApp(["-wbServer", signInServer])
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in with the saved token")
        a.buttons["settingsButton"].tap()
        // the account rows are below "Show on Home", off a small screen
        XCTAssertTrue(reveal(a.staticTexts["accountEmail"], in: a))
        waitLabel(a.staticTexts["accountEmail"], contains: "No password yet")
        let link = a.buttons["passwordLink"]
        XCTAssertTrue(reveal(link, in: a))
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
        let a = launchApp(["-wbReset", "YES"])
        XCTAssertTrue(a.textFields["serverField"].waitForExistence(timeout: 10), "first launch, no server yet")
        a.open(URL(string: "workboard://join?server=127.0.0.1%3A18788&code=\(code)&email=alex%40example.test")!)
        let field = a.textFields["inviteField"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "the link set the server and opened the invite form")
        XCTAssertEqual(field.value as? String, code)
        XCTAssertEqual(a.textFields["inviteEmailField"].value as? String, "alex@example.test")
        type(a.secureTextFields["newPasswordField"], "alex-password-1")
        type(a.secureTextFields["confirmPasswordField"], "alex-password-1")
        a.buttons["joinButton"].tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10))
    }

    func testCodeFromTheComputerSignsInWithoutAPassword() {
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
        let toCode = a.buttons["codeModeButton"]
        XCTAssertTrue(toCode.waitForExistence(timeout: 10))
        toCode.tap()
        let field = a.textFields["linkCodeField"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        XCTAssertTrue(a.buttons["scanButton"].exists, "the QR code can be scanned from here too")
        type(field, "zzzzzz")
        XCTAssertEqual(field.value as? String, "ZZZZZZ", "typed in capitals")
        a.buttons["linkSignInButton"].tap()
        waitLabel(a.staticTexts["signInError"], contains: "That code doesn't work")

        let code = admin("link-code?member=3")["code"] as! String
        type(field, code.lowercased())
        a.buttons["linkSignInButton"].tap()
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "signed in as the computer's person")
        a.buttons["settingsButton"].tap()
        waitLabel(a.staticTexts["signedInAs"], contains: "Signed in as Sam Lee")
    }

    func testScannedComputerLinkSignsInAtOnce() {
        let a = launchApp(["-wbReset", "YES"])
        let scan = a.buttons["setupScanButton"]
        XCTAssertTrue(scan.waitForExistence(timeout: 10), "first launch offers to scan the computer's QR code")
        scan.tap()
        // the simulator has no camera: it says to type the code instead
        waitLabel(a.staticTexts["scanMessage"], contains: "six-character code")
        a.buttons["scanCancelButton"].tap()
        // what the Camera app opens after reading the QR code
        let code = admin("link-code?member=2")["code"] as! String
        a.open(URL(string: "workboard://link?server=ws%3A%2F%2F127.0.0.1%3A18788&code=\(code)")!)
        XCTAssertTrue(a.buttons["quickFind"].waitForExistence(timeout: 10), "the link set the server and signed in with no form")
    }

    func testComputerLinkShowsASpinnerUntilTheServerAnswers() throws {
        // a server that takes the connection and never answers, so the
        // spinner stays up long enough to see
        let silent = try SilentServer()
        let a = launchApp(["-wbReset", "YES"])
        XCTAssertTrue(a.buttons["setupScanButton"].waitForExistence(timeout: 10))
        a.open(URL(string: "workboard://link?server=ws%3A%2F%2F127.0.0.1%3A\(silent.port)&code=ABCDEF")!)
        let spinner = a.descendants(matching: .any)["openingWorkspace"]
        XCTAssertTrue(spinner.waitForExistence(timeout: 5), "a spinner while the sign-in goes out")
        waitLabel(spinner, contains: "Signing in")
        XCTAssertFalse(a.buttons["setupScanButton"].exists, "not the setup form standing still")
        // no answer: once the connection gives up, the sign-in screen says why
        XCTAssertTrue(a.textFields["linkCodeField"].waitForExistence(timeout: 40), "back to the code form")
        XCTAssertTrue(a.staticTexts["signInError"].waitForExistence(timeout: 5))
        XCTAssertFalse(spinner.exists)
    }

    func testPastedInviteEmailFillsTheForm() {
        let code = admin("invite?member=2")["code"] as! String
        let a = launchApp(["-wbReset", "YES", "-wbServer", signInServer])
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

/// Accepts connections on a free local port and never says anything back.
final class SilentServer {
    private let listener: NWListener
    let port: UInt16

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        // started and left alone: the app waits for an answer that never comes
        listener.newConnectionHandler = { $0.start(queue: .global()) }
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.start(queue: .global())
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }

    deinit { listener.cancel() }
}
