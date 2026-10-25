import XCTest

final class WebParityTests: XCTestCase {
    @MainActor func testNativeRulesPriorityMediaDraftReplyAndAbandon() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_PARITY"] == "1"
        else { throw XCTSkip("Seed parity-fixture.py and set MG_IOS_TEST_PARITY=1.") }
        let app = XCUIApplication()
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
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        let pending = app.buttons["pendingRules-mobile-smoke"]
        reveal(pending, in: app)
        pending.tap()
        XCTAssertTrue(
            app.buttons["Approve"].waitForExistence(timeout: 10), app.debugDescription)
        capture(app, "Parity — rule preview")
        app.buttons["Approve"].tap()
        XCTAssertTrue(app.staticTexts["Generated fixture"].waitForExistence(timeout: 10))
        app.buttons["Done"].tap()
        let review = app.buttons["openReview-mobile-smoke#3"]
        reveal(review, in: app)
        // Decision cards use their own Open review button identity.
        if review.exists && review.isHittable {
            review.tap()
        } else {
            let title = app.staticTexts["Native parity verification"]
            reveal(title, in: app)
            title.tap()
        }
        XCTAssertTrue(app.buttons["reviewMore"].waitForExistence(timeout: 20), app.debugDescription)
        app.buttons["reviewMore"].tap()
        let save = app.buttons["savePriority"]
        reveal(save, in: app)
        app.buttons["itemPriorityPicker"].tap()
        app.buttons["p0"].tap()
        save.tap()
        XCTAssertTrue(app.staticTexts["itemActionResult"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["itemActionResult"].label, "Priority saved")
        capture(app, "Parity — native item actions")
        app.buttons["Done"].tap()
        let demo = app.buttons["reviewDemo"]
        reveal(demo, in: app)
        demo.tap()
        XCTAssertTrue(
            app.buttons["Share original"].waitForExistence(timeout: 15), app.debugDescription)
        capture(app, "Parity — attached demo")
        app.buttons["Done"].tap()
        app.buttons["reviewFiles"].tap()
        app.buttons["reviewFile-0"].tap()
        let preview = app.buttons["toggleFilePreview"]
        reveal(preview, in: app)
        preview.tap()
        XCTAssertTrue(app.staticTexts["Native parity"].waitForExistence(timeout: 10))
        capture(app, "Parity — Markdown preview")
        app.buttons["reviewFiles"].tap()
        app.buttons["reviewFile-1"].tap()
        reveal(preview, in: app)
        preview.tap()
        XCTAssertTrue(app.buttons["Share before-preview.png"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["Share after-preview.png"].exists)
        capture(app, "Parity — image comparison")
        app.buttons["reviewConversations"].tap()
        let thread = app.buttons["conversation-c_5"]
        reveal(thread, in: app)
        thread.tap()
        let reply = app.buttons["replyToThread"]
        reveal(reply, in: app)
        reply.tap()
        let text = app.textViews["conversationDraftText"]
        XCTAssertTrue(text.waitForExistence(timeout: 5))
        text.tap()
        text.typeText("The native preview shows the table.")
        app.buttons["Save draft"].tap()
        XCTAssertTrue(
            app.staticTexts["The native preview shows the table."].waitForExistence(timeout: 10))
        capture(app, "Parity — saved thread reply")
        let source = app.buttons["showConversationSource"]
        reveal(source, in: app)
        source.tap()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["reviewFiles"].label.contains("README.md"))
        XCTAssertTrue(app.textViews["unifiedDiff"].firstMatch.waitForExistence(timeout: 5))
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.buttons["sendFeedback"].waitForExistence(timeout: 10))
        app.buttons["sendFeedback"].tap()
        XCTAssertTrue(app.staticTexts["feedbackSent"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["nextReview"].waitForExistence(timeout: 10))
        capture(app, "Parity — feedback and next review")
        app.buttons["closeFeedback"].tap()
        app.buttons["reviewMore"].tap()
        let abandon = app.buttons["Abandon item…"]
        reveal(abandon, in: app)
        abandon.tap()
        app.buttons["Abandon item"].tap()
        XCTAssertTrue(app.staticTexts["itemActionResult"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["itemActionResult"].label, "Abandoned")
        app.buttons["Done"].tap()
        XCTAssertTrue(app.staticTexts["reviewClosed"].waitForExistence(timeout: 10))
        capture(app, "Parity — closed item")
        app.buttons["nextReview"].tap()
        XCTAssertTrue(app.staticTexts["reviewIdentity"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("mobile-smoke#1"))
    }

    @MainActor func testReadNativeDetailsAtAccessibilitySize() throws {
        guard ProcessInfo.processInfo.environment["MG_IOS_TEST_PARITY"] == "1" else {
            throw XCTSkip("Run the disposable parity fixture first.")
        }
        let appearance = XCUIDevice.shared.appearance
        defer { XCUIDevice.shared.appearance = appearance }
        XCUIDevice.shared.appearance = .dark
        let app = XCUIApplication()
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        let review = app.buttons["openReview-mobile-smoke#3"]
        reveal(review, in: app)
        review.tap()
        XCTAssertTrue(app.buttons["reviewMore"].waitForExistence(timeout: 15))
        app.buttons["reviewMore"].tap()
        XCTAssertTrue(app.staticTexts["Native parity verification"].waitForExistence(timeout: 10))
        try app.performAccessibilityAudit(for: [
            .contrast, .hitRegion, .sufficientElementDescription,
        ])
        let save = app.buttons["savePriority"]
        reveal(save, in: app)
        XCTAssertTrue(save.waitForExistence(timeout: 10))
        // The category-changing audit loses virtualized List nodes. Measure the scaled control directly.
        XCTAssertGreaterThan(save.frame.height, 44)
        capture(app, "Parity — dark accessibility item controls")
        let rules = app.buttons["Project rules"]
        reveal(rules, in: app)
        rules.tap()
        XCTAssertTrue(app.staticTexts["Generated fixture"].firstMatch.waitForExistence(timeout: 10))
        capture(app, "Parity — dark accessibility rules")
        try auditInboxAccessibility(app)
    }

    @MainActor private func reveal(_ element: XCUIElement, in app: XCUIApplication) {
        for _ in 0..<12 {
            if element.waitForExistence(timeout: 0.5) && element.isHittable { return }
            app.swipeUp()
        }
    }
    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
