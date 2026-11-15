// UI tests for round 3: the owner's People and Backups screens, and Trash.
// Needs the fake servers from docs/ios-design.md:
//   swift run wb-fake-server --port 18787
//   swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev

import XCTest

final class Round3UITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func launch(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        a.launchArguments = args
        a.launch()
        return a
    }

    func testOwnerMakesACodeChangesARoleAndRestoresABackup() {
        let a = launch(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 1, email: "jamie@example.com", password: "jamie-password-1")
        a.buttons["settingsButton"].tap()
        let people = a.buttons["peopleLink"]
        XCTAssertTrue(people.waitForExistence(timeout: 5), "owners see the Team section")
        people.tap()

        // an invite code for Sam, emailed to the address on record
        let samMenu = a.buttons["personMenu-3"]
        XCTAssertTrue(samMenu.waitForExistence(timeout: 8))
        samMenu.tap()
        let invite = a.buttons["Invite code"].exists ? a.buttons["Invite code"] : a.buttons["New code"]
        XCTAssertTrue(invite.waitForExistence(timeout: 5))
        invite.tap()
        XCTAssertTrue(a.staticTexts["inviteCode-3"].waitForExistence(timeout: 5))
        waitLabel(a.staticTexts["inviteNote"].firstMatch, contains: "sam@example.com")
        XCTAssertFalse(a.buttons["personMenu-1"].exists, "nothing to do to myself")

        // make Alex an admin and back
        a.buttons["personMenu-2"].tap()
        let makeAdmin = a.buttons["Make admin"]
        XCTAssertTrue(makeAdmin.waitForExistence(timeout: 5))
        makeAdmin.tap()
        waitLabel(a.staticTexts["personRole-2"], contains: "Admin")
        a.buttons["personMenu-2"].tap()
        a.buttons["Make member"].tap()
        waitLabel(a.staticTexts["personRole-2"], contains: "Member")

        // the owner restores a backup
        admin("backup?name=daily-20261115.json")
        a.navigationBars.buttons.element(boundBy: 0).tap()
        let backups = a.buttons["backupsLink"]
        XCTAssertTrue(backups.waitForExistence(timeout: 5))
        backups.tap()
        let restore = a.buttons["restore-daily-20261115.json"]
        XCTAssertTrue(restore.waitForExistence(timeout: 8))
        restore.tap()
        a.buttons["confirmRestore"].firstMatch.tap()
        waitLabel(a.staticTexts["restoreNotice"].firstMatch, contains: "2026-11-15 (daily)")
    }

    func testMembersDontSeeTheTeamSection() {
        let a = launch(["-wbReset", "YES", "-wbServer", signInServer])
        join(a, member: 3, password: "sam-password-r3")
        a.buttons["settingsButton"].tap()
        XCTAssertTrue(a.buttons["trashLink"].waitForExistence(timeout: 5))
        XCTAssertFalse(a.buttons["peopleLink"].exists)
        XCTAssertFalse(a.buttons["backupsLink"].exists)
    }

    func testDeletedTaskComesBackFromTrash() {
        let open = "ws://127.0.0.1:18787"
        let a = launch(["-wbReset", "YES", "-wbServer", open, "-wbMe", "1", "-wbOpen", "list:100:120"])
        let add = a.buttons["addTaskButton"]
        XCTAssertTrue(add.waitForExistence(timeout: 10))
        let enabled = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: add)
        wait(for: [enabled], timeout: 10)
        let name = "Trash me \(Int.random(in: 1000...9999))"
        add.tap()
        let tf = a.textFields["newTaskField"]
        XCTAssertTrue(tf.waitForExistence(timeout: 5))
        tf.typeText(name + "\n")
        let row = a.staticTexts[name].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        let more = a.buttons["taskMoreMenu"]
        XCTAssertTrue(more.waitForExistence(timeout: 5))
        more.tap()
        a.buttons["deleteTaskButton"].tap()
        a.buttons["confirmDeleteTask"].firstMatch.tap()
        XCTAssertTrue(a.navigationBars["Backlog"].waitForExistence(timeout: 5))
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: row)
        wait(for: [gone], timeout: 5)
        a.terminate()

        // Trash is kept on the phone across launches
        let b = launch(["-wbServer", open, "-wbOpen", "settings"])
        let trash = b.buttons["trashLink"]
        XCTAssertTrue(trash.waitForExistence(timeout: 10))
        trash.tap()
        let restore = b.buttons["restoreTrash-\(name)"]
        XCTAssertTrue(restore.waitForExistence(timeout: 5))
        let ready = expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: restore)
        wait(for: [ready], timeout: 10)
        restore.tap()
        waitLabel(b.staticTexts["trashMessage"], contains: "Restored")
        XCTAssertFalse(restore.exists)
        b.terminate()

        let c = launch(["-wbServer", open, "-wbOpen", "list:100:120"])
        XCTAssertTrue(c.staticTexts[name].firstMatch.waitForExistence(timeout: 10), "the task is back in its list")
    }
}
