// The desktop (egui) app on iPad, installed by scripts/ipad-sim. It draws
// everything itself and has no accessibility elements on iOS yet, so this
// taps by position and keeps screenshots (xcresult attachments) to look at.
// `banner:text` waits for the notification banner with that text and taps it.
// Experimental (docs/ipad.md); skipped unless asked for:
//   TEST_RUNNER_WB_DESK=1 xcodebuild ... -only-testing:WorkboardUITests/DeskUITests test

import XCTest

final class DeskUITests: XCTestCase {
    var app: XCUIApplication!

    override func setUpWithError() throws {
        guard ProcessInfo.processInfo.environment["WB_DESK"] == "1" else {
            throw XCTSkip("set TEST_RUNNER_WB_DESK=1 after scripts/ipad-sim")
        }
        continueAfterFailure = true
        XCUIDevice.shared.orientation = .landscapeLeft
        app = XCUIApplication(bundleIdentifier: "com.seojigwon.workboard.desk")
        app.launchArguments = ["-AppleLanguages", "(ko)", "-AppleLocale", "ko_KR"]
        app.launch()
        sleep(4)
    }

    func shot(_ name: String) {
        let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    /// A tap at a point in the screenshot's points (landscape, 1133 x 744).
    func tap(_ x: CGFloat, _ y: CGFloat) {
        app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: x, dy: y)).tap()
        sleep(2)
    }

    func testWalkThrough() throws {
        shot("1-start")
        let steps = (ProcessInfo.processInfo.environment["WB_DESK_STEPS"] ?? "")
            .split(separator: ";")
        for (n, step) in steps.enumerated() {
            let parts = step.split(separator: ":", maxSplits: 1).map(String.init)
            switch parts[0] {
            case "tap":
                let xy = parts[1].split(separator: ",").compactMap { Double($0) }
                tap(CGFloat(xy[0]), CGFloat(xy[1]))
            case "drag":
                let v = parts[1].split(separator: ",").compactMap { Double($0) }
                let o = app.coordinate(withNormalizedOffset: .zero)
                o.withOffset(CGVector(dx: v[0], dy: v[1]))
                    .press(forDuration: 0.1, thenDragTo: o.withOffset(CGVector(dx: v[2], dy: v[3])))
                sleep(2)
            case "type":
                app.typeText(parts[1])
                sleep(2)
            case "portrait":
                XCUIDevice.shared.orientation = .portrait
                sleep(3)
            case "landscape":
                XCUIDevice.shared.orientation = .landscapeLeft
                sleep(3)
            case "wait":
                sleep(UInt32(parts[1]) ?? 2)
            case "banner":
                // the notification banner with this text (SpringBoard draws
                // it, so a tap by position in the app doesn't reach it)
                let text = parts.count > 1 ? parts[1] : ""
                let banner = XCUIApplication(bundleIdentifier: "com.apple.springboard")
                    .descendants(matching: .any)
                    .matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
                if banner.waitForExistence(timeout: 15) {
                    banner.tap()
                } else {
                    XCTFail("no notification banner saying \(text)")
                }
                sleep(3)
            default:
                XCTFail("unknown step \(step)")
            }
            shot("\(n + 2)-\(parts[0])")
        }
    }
}
