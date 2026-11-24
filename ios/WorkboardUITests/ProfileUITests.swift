// Edit profile and Set status in Settings, and others' photos and status
// where they show (as on the Mac: src/app/profile.rs, the profile menu).
// Needs the admin fake server:
//   swift run wb-fake-server --port 18789 --admin-http

import XCTest

final class ProfileUITests: WorkboardUITestCase {
    /// Someone's profile on the 18789 server: changed as if their app did
    /// it (`change`, like "status=Away&photo=1"), and read back.
    @discardableResult
    func member(_ id: Int, _ change: String = "") -> [String: Any] {
        let q = change.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? change
        return admin(port: 18789, "member?member=\(id)" + (q.isEmpty ? "" : "&" + q))["member"] as? [String: Any] ?? [:]
    }

    /// The member as the server has it once `ok` holds (or after 10 seconds).
    func serverMember(_ id: Int, until ok: ([String: Any]) -> Bool) -> [String: Any] {
        var m: [String: Any] = [:]
        for _ in 0..<20 {
            m = member(id)
            if ok(m) { break }
            usleep(500_000)
        }
        return m
    }

    func openEditProfile(_ a: XCUIApplication) {
        let edit = a.buttons["editProfileLink"]
        XCTAssertTrue(reveal(edit, in: a, timeout: 10))
        edit.tap()
        XCTAssertTrue(a.textFields["profileNameField"].waitForExistence(timeout: 5), "on Edit profile")
    }

    func testEditMyProfileAndSetMyStatus() {
        member(1, "display=&title=&status=&photo=0")
        let a = launchFresh(["-wbOpen", "settings"], server: adminServer)
        openEditProfile(a)
        XCTAssertEqual(a.textFields["profileNameField"].value as? String, "Jamie Park")
        type(a.textFields["profileDisplayNameField"], "Jay")
        type(a.textFields["profileTitleField"], "Founder")
        a.buttons["saveProfileButton"].tap()
        let status = a.buttons["setStatusLink"]
        XCTAssertTrue(reveal(status, in: a), "back in Settings")
        var m = serverMember(1) { $0["display_name"] as? String == "Jay" }
        XCTAssertEqual(m["display_name"] as? String, "Jay")
        XCTAssertEqual(m["title"] as? String, "Founder")
        XCTAssertEqual(m["name"] as? String, "Jamie Park", "the full name as it was")

        // a suggestion fills in the words, emoji and all
        status.tap()
        let suggestion = a.buttons["statusSuggestion-0"]
        XCTAssertTrue(suggestion.waitForExistence(timeout: 5))
        suggestion.tap()
        XCTAssertEqual(a.textFields["statusField"].value as? String, "\u{1F4C5} In a meeting")
        a.buttons["saveStatusButton"].tap()
        waitLabel(status, contains: "In a meeting")
        m = serverMember(1) { $0["status_text"] as? String == "\u{1F4C5} In a meeting" }
        XCTAssertEqual(m["status_text"] as? String, "\u{1F4C5} In a meeting")

        status.tap()
        let clear = a.buttons["clearStatusButton"]
        XCTAssertTrue(clear.waitForExistence(timeout: 5))
        clear.tap()
        XCTAssertTrue(reveal(status, in: a))
        XCTAssertFalse(status.label.contains("In a meeting"))
        m = serverMember(1) { $0["status_text"] as? String == "" }
        XCTAssertEqual(m["status_text"] as? String, "")
        // as it was, for the other tests
        member(1, "display=&title=")
    }

    func testMyPhotoShowsAndCanBeRemoved() {
        member(1, "photo=1")
        let a = launchFresh(["-wbOpen", "settings"], server: adminServer)
        openEditProfile(a)
        // the photo another device put on the server comes down and shows
        let photo = a.descendants(matching: .any).matching(identifier: "profilePhoto").firstMatch
        XCTAssertTrue(photo.waitForExistence(timeout: 5))
        let shown = expectation(for: NSPredicate(format: "value == 'Photo'"), evaluatedWith: photo)
        wait(for: [shown], timeout: 10)
        let remove = a.buttons["removePhotoButton"]
        XCTAssertTrue(remove.exists)
        remove.tap()
        XCTAssertEqual(photo.value as? String, "No photo")
        XCTAssertFalse(remove.exists)
        a.buttons["saveProfileButton"].tap()
        XCTAssertTrue(reveal(a.buttons["editProfileLink"], in: a), "back in Settings")
        let m = serverMember(1) { $0["photo"] is NSNull }
        XCTAssertTrue(m["photo"] is NSNull, "no photo, written as the Mac writes none")
    }

    func testSomeoneElsesStatusShowsInTheirMessagesAndAmongPeople() {
        member(2, "status=\u{1F334} Vacationing&title=Designer")
        let a = launchFresh(["-wbOpen", "channel:303"], server: adminServer)
        // at the top of the direct message, under their name
        let top = a.descendants(matching: .any).matching(identifier: "conversationStatus").firstMatch
        XCTAssertTrue(top.waitForExistence(timeout: 10))
        waitLabel(top, contains: "Vacationing")
        waitLabel(top, contains: "Alex Kim")
        // and whether they are online (the test server says nobody is)
        waitLabel(top, contains: "Offline")
        // and in People with what they do
        openPlace(a, "tools")
        let teams = a.buttons["app-teams"]
        XCTAssertTrue(reveal(teams, in: a))
        teams.tap()
        let about = a.staticTexts["memberAbout-2"]
        XCTAssertTrue(reveal(about, in: a))
        waitLabel(about, contains: "Designer · \u{1F334} Vacationing")
        member(2, "status=&title=")
    }

    func testProfileScreensAreInKorean() {
        let a = XCUIApplication()
        a.launchArguments = ["-wbReset", "YES", "-wbServer", adminServer, "-wbMe", "1", "-wbOpen", "settings"] + testArgs + ["-AppleLanguages", "(ko)", "-AppleLocale", "ko_KR"]
        a.launch()
        let status = a.buttons["setStatusLink"]
        XCTAssertTrue(reveal(status, in: a, timeout: 10))
        XCTAssertTrue(status.label.contains("상태 설정"))
        XCTAssertTrue(a.buttons["editProfileLink"].label.contains("내 프로필 편집"))
        status.tap()
        let suggestion = a.buttons["statusSuggestion-0"]
        XCTAssertTrue(suggestion.waitForExistence(timeout: 5))
        XCTAssertEqual(suggestion.label, "\u{1F4C5} 회의 중")
        XCTAssertTrue(a.navigationBars["상태 설정"].exists)
    }
}
