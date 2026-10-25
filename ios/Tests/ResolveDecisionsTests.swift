import XCTest

final class ResolveDecisionsTests: XCTestCase {
    @MainActor func testAnswersOverridesAndAcceptsThroughRealHub() async throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_DECISIONS"] == "1"
        else { throw XCTSkip("Run decisions-fixture.py and set MG_IOS_TEST_DECISIONS=1.") }
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
        let details = app.buttons["decisionDetails-app#1"]
        XCTAssertTrue(details.waitForExistence(timeout: 30), app.debugDescription)
        capture(app, "01 Decision desk — question and two rejections")
        details.tap()
        tap(app.buttons["resolve-c_1"], app)
        let editor = app.textViews["decisionText"]
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        editor.tap()
        editor.typeText("The receipt client needs the final charged total.")
        app.swipeUp()
        tap(app.buttons["saveDecisionAnswer"], app)
        XCTAssertTrue(
            app.staticTexts["decisionAnswerSaved"].waitForExistence(timeout: 10),
            app.debugDescription)
        capture(app, "02 Answer saved — explicit delivery boundary")
        try auditInboxAccessibility(app)
        // The saved reply survives process termination and is still pending until sent.
        app.terminate()
        app.launch()
        XCTAssertTrue(details.waitForExistence(timeout: 20))
        details.tap()
        tap(app.buttons["resolve-c_1"], app)
        XCTAssertTrue(app.staticTexts["decisionAnswerSaved"].waitForExistence(timeout: 10))
        XCTAssertEqual(editor.value as? String, "The receipt client needs the final charged total.")
        tap(app.buttons["sendDecisionFeedback"], app)
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 10))
        capture(app, "03 Preview answer before sending")
        tap(app.buttons["sendFeedback"], app)
        XCTAssertTrue(
            app.staticTexts["feedbackSent"].waitForExistence(timeout: 20), app.debugDescription)
        capture(app, "04 Answer delivery confirmed")
        app.buttons["closeFeedback"].tap()
        XCTAssertTrue(app.staticTexts["decisionResult"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["decisionResult"].label, "Answer sent")
        app.navigationBars["Answer question"].buttons.element(boundBy: 0).tap()
        app.buttons["closeDecisionDetails"].tap()
        try await verify(address, questions: 0, rejections: 2)
        XCTAssertTrue(details.waitForExistence(timeout: 15))
        details.tap()
        tap(app.buttons["resolve-c_2"], app)
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        editor.tap()
        editor.typeText("Keep the calculation separate so receipt behavior can be tested.")
        app.swipeUp()
        capture(app, "05 Resolve a rejection with an optional note")
        tap(app.buttons["overrideDecision"], app)
        XCTAssertTrue(
            app.staticTexts["decisionResult"].waitForExistence(timeout: 15), app.debugDescription)
        XCTAssertEqual(app.staticTexts["decisionResult"].label, "Override sent")
        capture(app, "05 Override sent immediately")
        app.navigationBars["Resolve rejection"].buttons.element(boundBy: 0).tap()
        app.buttons["closeDecisionDetails"].tap()
        try await verify(address, questions: 0, rejections: 1)
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ]
        app.launch()
        XCTAssertTrue(details.waitForExistence(timeout: 20))
        details.tap()
        tap(app.buttons["resolve-c_3"], app)
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        app.swipeUp()
        tap(app.buttons["acceptDecision"], app)
        let confirm = app.buttons["Accept and close"]
        XCTAssertTrue(confirm.waitForExistence(timeout: 5))
        confirm.tap()
        XCTAssertTrue(
            app.staticTexts["decisionResult"].waitForExistence(timeout: 15), app.debugDescription)
        XCTAssertEqual(app.staticTexts["decisionResult"].label, "Rejection accepted")
        capture(app, "06 Rejection accepted — dark largest text")
        app.swipeDown()
        try app.performAccessibilityAudit(for: [
            .contrast, .dynamicType, .hitRegion, .sufficientElementDescription,
        ])
        app.navigationBars["Resolve rejection"].buttons.element(boundBy: 0).tap()
        tap(app.buttons["closeDecisionDetails"], app)
        try await verify(address, questions: 0, rejections: 0)
        XCTAssertTrue(app.buttons["openReview-app#1"].waitForExistence(timeout: 15))
        let cleared = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == false"), object: details)
        XCTAssertEqual(
            XCTWaiter.wait(for: [cleared], timeout: 15), .completed, app.debugDescription)
        capture(app, "07 Pending decisions cleared from inbox")
        app.terminate()
    }

    @MainActor private func verify(_ address: String, questions: Int, rejections: Int) async throws
    {
        let base = try XCTUnwrap(URL(string: address))
        let (inboxData, _) = try await URLSession.shared.data(
            from: base.appending(path: "api/inbox"))
        let inbox = try XCTUnwrap(JSONSerialization.jsonObject(with: inboxData) as? [String: Any])
        let entries = ["needs_you", "with_agents"].flatMap { inbox[$0] as? [[String: Any]] ?? [] }
        let item = try XCTUnwrap(entries.first { $0["id"] as? String == "app#1" })
        XCTAssertEqual(item["questions"] as? Int ?? 0, questions)
        XCTAssertEqual(item["rejections"] as? Int ?? 0, rejections)
        XCTAssertEqual(item["votes"] as? Int ?? 0, 0)
        let (data, _) = try await URLSession.shared.data(
            from: base.appending(path: "api/events").appending(queryItems: [
                .init(name: "since", value: "0")
            ]))
        let log = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let events = try XCTUnwrap(log["events"] as? [[String: Any]])
        let sent = events.filter {
            $0["type"] as? String == "review.submitted" && $0["verdict"] as? String == "comment"
        }
        XCTAssertEqual(sent.count, 1)
        let reply = try XCTUnwrap((sent.first?["replies"] as? [[String: Any]])?.first)
        XCTAssertEqual(reply["comment"] as? String, "c_1")
        XCTAssertEqual(
            reply["body"] as? String, "The receipt client needs the final charged total.")
        if rejections < 2 {
            let overrides = events.filter { $0["type"] as? String == "comment.override" }
            XCTAssertEqual(overrides.count, 1)
            let override = try XCTUnwrap(overrides.first?["reply"] as? [String: Any])
            XCTAssertEqual(override["comment"] as? String, "c_2")
            XCTAssertEqual(
                override["body"] as? String,
                "Keep the calculation separate so receipt behavior can be tested.")
        }
    }

    @MainActor private func tap(_ element: XCUIElement, _ app: XCUIApplication) {
        for _ in 0..<8 {
            if element.waitForExistence(timeout: 1) && element.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable, app.debugDescription)
        element.tap()
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
