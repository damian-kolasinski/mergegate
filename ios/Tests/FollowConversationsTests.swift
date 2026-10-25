import XCTest

final class FollowConversationsTests: XCTestCase {
    @MainActor func testSourceThreadsHistoryStatusAndReadOnlyNavigation() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_CONVERSATIONS"] == "1"
        else { throw XCTSkip("Seed conversations-fixture.py and set MG_IOS_TEST_CONVERSATIONS=1.") }
        let appearance = XCUIDevice.shared.appearance
        defer { XCUIDevice.shared.appearance = appearance }
        XCUIDevice.shared.appearance = .light
        let app = XCUIApplication()
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
        app.launch()
        if app.buttons["hubSettings"].waitForExistence(timeout: 3) {
            app.buttons["hubSettings"].tap()
            app.buttons["forgetHub"].tap()
        }
        let field = app.textFields["hubURL"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        if let value = field.value as? String, value.hasPrefix("http") {
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: value.count))
        }
        field.typeText(address)
        app.swipeDown()
        app.buttons["connectHub"].tap()
        let open = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["fileConversations"].waitForExistence(timeout: 20))
        XCTAssertTrue(app.textViews["unifiedDiff"].waitForExistence(timeout: 10))
        capture(app, "Conversations — File reader")
        app.buttons["fileConversations"].tap()
        XCTAssertTrue(
            app.buttons["conversation-c_1"].waitForExistence(timeout: 10), app.debugDescription)
        XCTAssertFalse(app.buttons["conversation-c_4"].exists)
        capture(app, "Conversations — source lines and current status")
        try audit(app)
        app.buttons["conversation-c_1"].tap()
        XCTAssertTrue(app.staticTexts["threadStatus"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.staticTexts["threadStatus"].label, "Operator replied · Question")
        XCTAssertTrue(app.staticTexts["threadLocation"].label.contains("Before · Line 3"))
        XCTAssertEqual(app.staticTexts["threadQuote"].label, "let total = subtotal")
        XCTAssertEqual(
            app.staticTexts["threadComment"].label, "Should the receipt include delivery in this total?")
        XCTAssertTrue(app.staticTexts["The receipt client needs the final charged total."].exists)
        XCTAssertTrue(
            app.staticTexts["Which client still reads the subtotal-only value? Unicode é 🐙 stays intact."]
                .exists)
        capture(app, "Conversations — question and sent operator reply")
        try audit(app)
        app.navigationBars["Thread"].buttons["Conversations"].tap()
        app.buttons["conversation-c_2"].tap()
        XCTAssertTrue(app.staticTexts["threadStatus"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.staticTexts["threadStatus"].label, "Rejected")
        XCTAssertTrue(app.staticTexts["threadLocation"].label.contains("After · Line 3"))
        capture(app, "Conversations — rejection with its reason")
        app.buttons["closeConversations"].tap()
        // Select the removed source line; the native action narrows to its side and coordinates.
        let reader = app.textViews["unifiedDiff"]
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 165, dy: 69)).press(
            forDuration: 1.2)
        let menu = app.menuItems["Conversations"]
        let button = app.buttons["Conversations"]
        XCTAssertTrue(menu.waitForExistence(timeout: 4) || button.exists, app.debugDescription)
        if menu.exists { menu.tap() } else { button.tap() }
        XCTAssertTrue(
            app.buttons["conversation-c_1"].waitForExistence(timeout: 10), app.debugDescription)
        XCTAssertFalse(app.buttons["conversation-c_2"].exists)
        capture(app, "Conversations — native source selection")
        app.buttons["closeConversations"].tap()
        app.buttons["reviewConversations"].tap()
        let policy = app.buttons["conversation-c_4"]
        XCTAssertTrue(app.buttons["conversation-c_3"].waitForExistence(timeout: 10))
        for _ in 0..<4 {
            if policy.exists && policy.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(policy.isHittable, app.debugDescription)
        policy.tap()
        XCTAssertEqual(app.staticTexts["threadStatus"].label, "Awaiting reply")
        XCTAssertTrue(app.staticTexts["File: policy.txt"].exists)
        app.buttons["closeConversations"].tap()
        XCUIDevice.shared.appearance = .dark
        app.terminate()
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<10 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["reviewConversations"].waitForExistence(timeout: 20))
        app.buttons["reviewConversations"].tap()
        XCTAssertTrue(app.buttons["conversation-c_1"].waitForExistence(timeout: 10))
        capture(app, "Conversations — dark mode at largest text size")
        try audit(app)
        app.buttons["conversation-c_1"].tap()
        XCTAssertTrue(app.staticTexts["threadStatus"].waitForExistence(timeout: 5))
        capture(app, "Conversations — accessible thread")
        try audit(app)
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try app.performAccessibilityAudit(for: [
            .contrast, .dynamicType, .hitRegion, .sufficientElementDescription,
        ])
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
