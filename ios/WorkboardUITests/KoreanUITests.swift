// The app in Korean: tab names, screens and task history lines, as the Mac
// shows them. Needs the open fake server:
//   swift run wb-fake-server --port 18787

import XCTest

final class KoreanUITests: XCTestCase {
    let open = "ws://127.0.0.1:18787"

    override func setUp() { continueAfterFailure = false }

    func launch(_ args: [String]) -> XCUIApplication {
        let a = XCUIApplication()
        // testArgs sets English; Korean comes after it and wins
        a.launchArguments = ["-wbReset", "YES", "-wbServer", open, "-wbMe", "1"] + args + testArgs + ["-AppleLanguages", "(ko)", "-AppleLocale", "ko_KR"]
        a.launch()
        return a
    }

    func testTabsAndHomeAreInKorean() {
        let a = launch(["-wbTab", "home"])
        XCTAssertTrue(a.navigationBars["홈"].waitForExistence(timeout: 10))
        XCTAssertTrue(a.tabBars.buttons["채팅"].exists)
        XCTAssertTrue(a.tabBars.buttons["더 보기"].exists)
        a.tabBars.buttons["더 보기"].tap()
        XCTAssertTrue(a.buttons["app-goals"].waitForExistence(timeout: 5))
        XCTAssertTrue(a.buttons["app-goals"].label.contains("목표"))
    }

    func testTaskHistoryReadsInKorean() {
        let a = launch(["-wbOpen", "task:206"])
        XCTAssertTrue(a.descendants(matching: .any)["taskName"].firstMatch.waitForExistence(timeout: 10))
        let line = a.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "이 작업을 만들었습니다")).firstMatch
        for _ in 0..<10 where !line.exists { a.swipeUp() }
        XCTAssertTrue(line.exists, "\"created this task\" in Korean")
    }
}
