// The app in Korean: Home's places, screens and task history lines, as the Mac
// shows them. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class KoreanUITests: WorkboardUITestCase {
    func launch(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        // testArgs sets English; Korean comes after it and wins
        a.launchArguments = ["-wbReset", "YES", "-wbServer", openServer, "-wbMe", "1"] + args + testArgs + ["-AppleLanguages", "(ko)", "-AppleLocale", "ko_KR"]
        a.launch()
        return a
    }

    func testHomeIsInKorean() {
        let a = launch([])
        let find = a.buttons["quickFind"]
        XCTAssertTrue(find.waitForExistence(timeout: 10))
        XCTAssertTrue(find.label.contains("빠른 찾기"))
        for (id, ko) in [("inbox", "받은함"), ("today", "오늘"), ("upcoming", "예정"), ("chats", "채팅"), ("docs", "문서"), ("tools", "도구")] {
            XCTAssertTrue(a.buttons["place-\(id)"].label.contains(ko), "\(id) is \(ko)")
        }
        XCTAssertTrue(a.buttons["settingsButton"].label.contains("설정"))
        XCTAssertEqual(a.buttons["newItemButton"].label, "새로 만들기")
        a.buttons["place-tools"].tap()
        XCTAssertTrue(a.buttons["app-goals"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.buttons["app-goals"].label.contains("목표"))
        XCTAssertTrue(a.staticTexts["자주 쓰는 도구"].exists)
        XCTAssertTrue(a.staticTexts["모든 도구"].exists)
    }

    func testTheNamePromptIsInKorean() {
        let a = launch([])
        let new = a.buttons["newSpaceButton"]
        waitEnabled(new)
        new.tap()
        let alert = a.alerts["새 스페이스"]
        XCTAssertTrue(alert.waitForExistence(timeout: 5), "the title in Korean")
        XCTAssertTrue(alert.buttons["만들기"].exists)
        alert.buttons["취소"].tap()
    }

    func testTaskHistoryReadsInKorean() {
        let a = launch(["-wbOpen", "task:206"])
        XCTAssertTrue(a.descendants(matching: .any)["taskName"].firstMatch.waitForExistence(timeout: 10))
        let line = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "이 작업을 만들었습니다")).firstMatch
        for _ in 0..<10 where !line.exists { a.swipeUp() }
        XCTAssertTrue(line.exists, "\"created this task\" in Korean")
    }
}
