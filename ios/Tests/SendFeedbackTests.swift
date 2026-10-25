import XCTest

final class SendFeedbackTests: XCTestCase {
    @MainActor func testSendDraftsAndSummaryThroughRealHub() async throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_FEEDBACK"] == "1"
        else { throw XCTSkip("Seed drafts-fixture.py and set MG_IOS_TEST_FEEDBACK=1.") }
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
        try openReview(app)
        capture(app, "Feedback — File reader")
        try comment(app, y: 69, body: "Should delivery be included in the receipt?")
        try comment(app, y: 90, body: "Keep the charged total consistent with the receipt.")
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["feedbackScope"].label, "mobile-smoke#2 · r1")
        XCTAssertTrue(app.staticTexts["2 local comments"].exists)
        XCTAssertTrue(app.staticTexts["Before · Line 3"].exists)
        app.textViews["feedbackSummary"].tap()
        app.textViews["feedbackSummary"].typeText("Check delivery and receipt consistency.")
        app.swipeDown()
        capture(app, "Feedback — summary and source comments")
        // Summary and comments must survive termination without a Send or Done action.
        app.terminate()
        app.launch()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        XCTAssertEqual(
            app.textViews["feedbackSummary"].value as? String, "Check delivery and receipt consistency.")
        try audit(app)
        capture(app, "Feedback — restored completion sheet")
        app.swipeUp()
        capture(app, "Feedback — both source anchors")
        app.buttons["sendFeedback"].tap()
        XCTAssertTrue(
            app.staticTexts["feedbackSent"].waitForExistence(timeout: 20), app.debugDescription)
        capture(app, "Feedback — hub confirmed receipt")
        try audit(app)
        app.buttons["closeFeedback"].tap()
        app.buttons["reviewDrafts"].tap()
        XCTAssertTrue(app.staticTexts["No comment drafts"].waitForExistence(timeout: 5))
        app.buttons["closeDrafts"].tap()
        app.buttons["reviewConversations"].tap()
        XCTAssertTrue(app.buttons["conversation-c_1"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["conversation-c_2"].exists)
        capture(app, "Feedback — sent conversations")
        app.buttons["closeConversations"].tap()
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ]
        app.launch()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        XCTAssertFalse((app.textViews["feedbackSummary"].value as? String ?? "").contains("Check delivery"))
        XCTAssertFalse(app.buttons["sendFeedback"].isEnabled)
        capture(app, "Feedback — dark and largest text size")
        try audit(app)
        app.buttons["closeFeedback"].tap()
        app.terminate()
        try await verifyHub(address)
    }

    @MainActor private func verifyHub(_ address: String) async throws {
        let base = try XCTUnwrap(URL(string: address))
        let (data, _) = try await URLSession.shared.data(
            from: base.appending(path: "api/events").appending(queryItems: [
                .init(name: "since", value: "0")
            ]))
        let log = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let events = try XCTUnwrap(log["events"] as? [[String: Any]])
        let reviews = events.filter {
            $0["item"] as? String == "mobile-smoke#2" && $0["type"] as? String == "review.submitted"
        }
        XCTAssertEqual(reviews.count, 1)
        let review = try XCTUnwrap(reviews.first)
        XCTAssertEqual(review["revision"] as? Int, 1)
        XCTAssertEqual(review["summary"] as? String, "Check delivery and receipt consistency.")
        XCTAssertEqual(review["verdict"] as? String, "comment")
        let comments = try XCTUnwrap(review["comments"] as? [[String: Any]])
        XCTAssertEqual(comments.count, 2)
        XCTAssertEqual(Set(comments.compactMap { $0["side"] as? String }), ["old", "new"])
        for comment in comments {
            XCTAssertEqual(comment["revision"] as? Int, 1)
            XCTAssertEqual(comment["path"] as? String, "checkout.swift")
            XCTAssertEqual(comment["start_line"] as? Int, 3)
            XCTAssertEqual(comment["line"] as? Int, 3)
        }
        XCTAssertEqual(
            Set(comments.compactMap { $0["body"] as? String }),
            [
                "Should delivery be included in the receipt?",
                "Keep the charged total consistent with the receipt.",
            ])
    }

    @MainActor private func openReview(_ app: XCUIApplication) throws {
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20), app.debugDescription)
        let open = app.buttons["openReview-mobile-smoke#2"]
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.textViews["unifiedDiff"].waitForExistence(timeout: 20))
    }

    @MainActor private func comment(_ app: XCUIApplication, y: CGFloat, body: String) throws {
        let reader = app.textViews["unifiedDiff"]
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 165, dy: y)).press(
            forDuration: 1.2)
        let menu = app.menuItems["Comment"]
        XCTAssertTrue(
            menu.waitForExistence(timeout: 4) || app.buttons["Comment"].exists, app.debugDescription)
        if menu.exists { menu.tap() } else { app.buttons["Comment"].tap() }
        XCTAssertTrue(app.textViews["draftBody"].waitForExistence(timeout: 5))
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText(body)
        app.buttons["closeDrafts"].tap()
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
