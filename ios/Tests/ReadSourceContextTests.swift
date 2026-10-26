import XCTest

final class ReadSourceContextTests: XCTestCase {
    @MainActor func testReadRealSourceContext() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_SOURCE"] == "1" else {
            throw XCTSkip("Run source-fixture.py on a disposable smoke hub and set MG_IOS_TEST_SOURCE=1.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-AppleInterfaceStyle", "Light"]
        app.launch()
        if !app.buttons["hubSettings"].waitForExistence(timeout: 3) {
            let field = app.textFields["hubURL"]
            XCTAssertTrue(field.waitForExistence(timeout: 5))
            field.tap()
            if let value = field.value as? String, value.hasPrefix("http") {
                field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: value.count))
            }
            field.typeText(address)
            app.swipeDown()
            app.buttons["connectHub"].tap()
        }
        let open = app.buttons["openReview-mobile-smoke#2"]
        if !open.waitForExistence(timeout: 15) { app.swipeUp() }
        XCTAssertTrue(open.waitForExistence(timeout: 15), app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["loadSourceContext"].waitForExistence(timeout: 20))
        capture(app, "01 — Real unified diff before context expansion")
        app.buttons["loadSourceContext"].tap()
        XCTAssertTrue(app.buttons["expandContext-0"].waitForExistence(timeout: 15), app.debugDescription)
        app.buttons["expandContext-0"].tap()
        let source = app.textViews["unifiedDiff"].firstMatch
        XCTAssertTrue((source.value as? String ?? "").contains("Checkout pricing — original source context"))
        XCTAssertTrue((source.value as? String ?? "").contains("é 🐙"))
        try app.performAccessibilityAudit(for: [.contrast, .hitRegion, .sufficientElementDescription])
        capture(app, "02 — Expanded source with wrapped Unicode line")
        source.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 190, dy: 55)).press(forDuration: 1.2)
        XCTAssertTrue(
            app.menuItems["Copy"].waitForExistence(timeout: 5) || app.buttons["Copy"].exists, app.debugDescription)
        capture(app, "03 — Native source selection and copy")
        if app.menuItems["Copy"].exists { app.menuItems["Copy"].tap() } else { app.buttons["Copy"].tap() }
        let middle = app.buttons["expandContext-1"]
        for _ in 0..<12 where !middle.isHittable { app.swipeUp() }
        XCTAssertTrue(middle.isHittable)
        middle.tap()
        XCTAssertTrue(middle.waitForExistence(timeout: 3), "A long gap remains progressively expandable.")
        capture(app, "04 — Expand the gap between real hunks")
        for _ in 0..<20 where !app.buttons["expandContext-2"].isHittable { app.swipeUp() }
        XCTAssertTrue(app.buttons["expandContext-2"].isHittable)
        app.buttons["expandContext-2"].tap()
        let last = app.textViews.matching(identifier: "unifiedDiff").allElementsBoundByIndex.last!
        XCTAssertTrue((last.value as? String ?? "").contains("Pricing policy 80"))
        for _ in 0..<10 where last.frame.maxY > app.frame.maxY { app.swipeUp() }
        XCTAssertLessThanOrEqual(last.frame.maxY, app.frame.maxY)
        capture(app, "05 — Trailing source context")
        for _ in 0..<30 where !app.buttons["reviewFiles"].isHittable { app.swipeDown() }
        app.buttons["reviewFiles"].tap()
        XCTAssertTrue(app.buttons["reviewFile-1"].waitForExistence(timeout: 5))
        app.buttons["reviewFile-1"].tap()
        XCTAssertTrue(app.staticTexts["No changed text"].waitForExistence(timeout: 5))
        app.buttons["loadSourceContext"].tap()
        XCTAssertTrue(app.buttons["expandContext-0"].waitForExistence(timeout: 10))
        app.buttons["expandContext-0"].tap()
        XCTAssertTrue(
            (app.textViews["unifiedDiff"].firstMatch.value as? String ?? "").contains("Unchanged renamed source é 🐙"))
        capture(app, "06 — Read unchanged renamed source")
        app.terminate()
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL", "-AppleInterfaceStyle",
            "Dark",
        ]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<5 where !open.isHittable { app.swipeUp() }
        open.tap()
        XCTAssertTrue(app.buttons["loadSourceContext"].waitForExistence(timeout: 20))
        app.buttons["loadSourceContext"].tap()
        let expand = app.buttons["expandContext-0"]
        for _ in 0..<6 where !expand.isHittable { app.swipeUp() }
        XCTAssertTrue(expand.waitForExistence(timeout: 10))
        expand.tap()
        capture(app, "07 — Source context at accessibility size in dark mode")
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
