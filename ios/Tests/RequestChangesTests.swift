import XCTest

final class RequestChangesTests: XCTestCase {
    @MainActor func testRequestChangesReturnsItemToWorkingInbox() async throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_REQUEST_CHANGES"] == "1"
        else { throw XCTSkip("Seed drafts-fixture.py and set MG_IOS_TEST_REQUEST_CHANGES=1.") }
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
        capture(app, "Request changes — File reader")
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        app.buttons["feedbackVerdict"].tap()
        app.buttons["Request changes"].tap()
        XCTAssertFalse(app.buttons["sendFeedback"].isEnabled)
        app.textViews["feedbackSummary"].tap()
        app.textViews["feedbackSummary"].typeText(
            "Include delivery in the receipt and add a test for the charged total.")
        app.swipeDown()
        capture(app, "Request changes — actionable feedback")
        try audit(app)
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL",
        ]
        app.launch()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.buttons["sendFeedback"].label, "Request changes")
        XCTAssertEqual(app.staticTexts["feedbackScope"].label, "mobile-smoke#2 · r1")
        XCTAssertEqual(
            app.textViews["feedbackSummary"].value as? String,
            "Include delivery in the receipt and add a test for the charged total.")
        capture(app, "Request changes — restored dark mode")
        try audit(app)
        app.buttons["sendFeedback"].tap()
        XCTAssertTrue(
            app.staticTexts["feedbackSent"].waitForExistence(timeout: 20), app.debugDescription)
        XCTAssertTrue(app.staticTexts["Changes requested"].exists)
        capture(app, "Request changes — confirmed delivery")
        try audit(app)
        app.buttons["closeFeedback"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let item = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(item.waitForExistence(timeout: 10), app.debugDescription)
        let withAgents = app.descendants(matching: .any)["With agents, 1 item"]
        XCTAssertTrue(withAgents.exists, app.debugDescription)
        XCTAssertGreaterThan(item.frame.minY, withAgents.frame.minY)
        XCTAssertTrue(
            app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "changes requested · waiting for r"))
                .firstMatch.exists)
        capture(app, "Request changes — With agents inbox")
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
        XCTAssertEqual(review["verdict"] as? String, "changes_requested")
        XCTAssertEqual(
            review["summary"] as? String,
            "Include delivery in the receipt and add a test for the charged total.")
        let (inboxData, _) = try await URLSession.shared.data(
            from: base.appending(path: "api/inbox"))
        let inbox = try XCTUnwrap(JSONSerialization.jsonObject(with: inboxData) as? [String: Any])
        let working = try XCTUnwrap(inbox["with_agents"] as? [[String: Any]])
        XCTAssertTrue(
            working.contains {
                $0["id"] as? String == "mobile-smoke#2"
                    && $0["state"] as? String == "changes_requested"
            })
        let needsYou = try XCTUnwrap(inbox["needs_you"] as? [[String: Any]])
        XCTAssertFalse(needsYou.contains { $0["id"] as? String == "mobile-smoke#2" })
    }

    @MainActor private func openReview(_ app: XCUIApplication) throws {
        XCTAssertTrue(
            app.buttons["hubSettings"].waitForExistence(timeout: 20), app.debugDescription)
        let open = app.buttons["openReview-mobile-smoke#2"]
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.textViews["unifiedDiff"].waitForExistence(timeout: 20))
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try app.performAccessibilityAudit(for: [
            .contrast, .hitRegion, .sufficientElementDescription,
        ])
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
