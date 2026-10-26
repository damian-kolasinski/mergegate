import XCTest

final class ApproveAndMergeTests: XCTestCase {
    @MainActor func testStaleApprovalThenMergeExactRevisionAndTarget() async throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_APPROVAL"] == "1"
        else { throw XCTSkip("Seed approval-fixture.py and set MG_IOS_TEST_APPROVAL=1.") }
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
        capture(app, "Approve and merge — File reader")
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        app.buttons["feedbackVerdict"].tap()
        app.buttons["Approve & merge"].tap()
        XCTAssertTrue(app.buttons["sendFeedback"].isEnabled)
        XCTAssertEqual(app.staticTexts["approvalTarget"].label, "r1 → main")
        capture(app, "Approve and merge — feedback preview")
        try audit(app)
        app.buttons["sendFeedback"].tap()
        let confirm = app.buttons["Approve & merge into main"]
        XCTAssertTrue(confirm.waitForExistence(timeout: 5))
        capture(app, "Approve and merge — exact revision confirmation")
        // The host controller submits real CLI work while the confirmation is open.
        _ = try await control("advance", method: "POST")
        confirm.tap()
        XCTAssertTrue(app.staticTexts["feedbackError"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["feedbackError"].label.contains("newer revision"))
        XCTAssertFalse(app.staticTexts["feedbackSent"].exists)
        capture(app, "Approve and merge — stale revision blocked")
        app.buttons["closeFeedback"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.buttons["feedbackVerdict"].waitForExistence(timeout: 10))
        app.buttons["feedbackVerdict"].tap()
        app.buttons["Approve & merge"].tap()
        XCTAssertEqual(app.staticTexts["approvalTarget"].label, "r2 → main")
        capture(app, "Approve and merge — current revision")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL",
        ]
        app.launch()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.buttons["sendFeedback"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.buttons["sendFeedback"].label, "Approve & merge")
        XCTAssertEqual(app.staticTexts["feedbackScope"].label, "mobile-smoke#2 · r2")
        capture(app, "Approve and merge — dark mode")
        try audit(app)
        app.terminate()
        XCUIDevice.shared.appearance = .light
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
        app.launch()
        try openReview(app)
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.buttons["sendFeedback"].waitForExistence(timeout: 10))
        app.buttons["sendFeedback"].tap()
        XCTAssertTrue(confirm.waitForExistence(timeout: 5))
        capture(app, "Approve and merge — confirm r2 into main")
        confirm.tap()
        XCTAssertTrue(
            app.staticTexts["feedbackSent"].waitForExistence(timeout: 25), app.debugDescription)
        XCTAssertTrue(app.staticTexts["Merged"].exists)
        let target = try await control("verify")
        let commit = try XCTUnwrap(target["commit"] as? String)
        XCTAssertTrue(app.staticTexts["approvalResult"].label.contains(commit))
        capture(app, "Approve and merge — target commit verified")
        try audit(app)
        app.buttons["closeFeedback"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let closedToday = app.descendants(matching: .any)["inboxClosedToday"].firstMatch
        XCTAssertTrue(closedToday.waitForExistence(timeout: 10), app.debugDescription)
        closedToday.tap()
        let item = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(item.waitForExistence(timeout: 10), app.debugDescription)
        XCTAssertGreaterThan(item.frame.minY, closedToday.frame.minY)
        capture(app, "Approve and merge — closed inbox")
        try await verifyHub(address)
        app.terminate()
    }

    @MainActor private func control(_ path: String, method: String = "GET") async throws -> [String:
        Any]
    {
        let address = try XCTUnwrap(ProcessInfo.processInfo.environment["MG_IOS_TEST_CONTROL_URL"])
        var request = URLRequest(url: try XCTUnwrap(URL(string: address)?.appending(path: path)))
        request.httpMethod = method
        let (data, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual(
            (response as? HTTPURLResponse)?.statusCode, 200, String(decoding: data, as: UTF8.self))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
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
        XCTAssertEqual(reviews.first?["revision"] as? Int, 2)
        XCTAssertEqual(reviews.first?["verdict"] as? String, "approved")
        let (data2, _) = try await URLSession.shared.data(from: base.appending(path: "api/inbox"))
        let inbox = try XCTUnwrap(JSONSerialization.jsonObject(with: data2) as? [String: Any])
        let closed = try XCTUnwrap(inbox["closed"] as? [[String: Any]])
        XCTAssertTrue(
            closed.contains {
                $0["id"] as? String == "mobile-smoke#2" && $0["state"] as? String == "merged"
            })
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
