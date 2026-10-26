import XCTest

final class TriageInboxTests: XCTestCase {
    @MainActor
    func testTriageRealInbox() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_TRIAGE"] == "1" else {
            throw XCTSkip("Run the disposable triage fixture and set MG_IOS_TEST_TRIAGE=1.")
        }
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
        let decisionAction = app.buttons["decisionDetails-mobile-smoke#2"]
        XCTAssertTrue(decisionAction.waitForExistence(timeout: 30), app.debugDescription)
        XCTAssertTrue(decisionAction.label.contains("1 question"), decisionAction.label)
        XCTAssertTrue(decisionAction.isHittable, "The first pending decision must be actionable without scrolling.")
        XCTAssertGreaterThanOrEqual(decisionAction.frame.height, 44)
        // The web's sections, in its order: owed work first, then what the agents hold.
        let needsYou = app.descendants(matching: .any)["Needs you, 2 items"]
        let withAgents = app.descendants(matching: .any)["With agents, 1 item"]
        XCTAssertTrue(needsYou.exists && withAgents.exists, app.debugDescription)
        XCTAssertLessThan(needsYou.frame.minY, withAgents.frame.minY)
        let question = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(question.label.contains("Choose the response format"), question.label)
        XCTAssertLessThan(question.frame.minY, withAgents.frame.minY)
        XCTAssertTrue(app.staticTexts["Retry failed connections"].exists)
        XCTAssertGreaterThan(app.staticTexts["Retry failed connections"].frame.minY, withAgents.frame.minY)
        XCTAssertTrue(app.descendants(matching: .any)["inboxSize"].firstMatch.exists)
        XCTAssertFalse(app.staticTexts["Retire the old endpoint"].exists, "Closed today starts folded, as on the web.")
        let history = app.descendants(matching: .any)["inboxWeekly"].firstMatch
        if history.exists { XCTAssertGreaterThan(history.frame.minY, decisionAction.frame.maxY) }
        try auditInboxAccessibility(app)
        capture(app, "Inbox — web sections and rows")
        XCTAssertTrue(app.staticTexts["Inbox"].firstMatch.exists)
        decisionAction.tap()
        XCTAssertTrue(app.staticTexts["Keep compatibility with the previous response format."].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Which clients still need the legacy format?"].exists)
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.staticTexts["Keep compatibility with the previous response format."].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["closeDecisionDetails"].exists)
        try auditInboxAccessibility(app)
        capture(app, "Read the real question")
        app.buttons["closeDecisionDetails"].tap()
        app.descendants(matching: .any)["inboxClosedToday"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["Retire the old endpoint"].waitForExistence(timeout: 5))
        capture(app, "Closed today unfolded")
        app.buttons["projectFilter"].tap()
        app.buttons["companion"].tap()
        XCTAssertTrue(app.staticTexts["Retire the old endpoint"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["inboxEmpty"].exists)
        XCTAssertFalse(app.buttons["openReview-mobile-smoke#2"].exists)
        XCTAssertFalse(app.staticTexts["Retry failed connections"].exists)
        capture(app, "Project filter — companion closed item")
        app.buttons["projectFilter"].tap()
        app.buttons["All projects"].tap()
        XCTAssertTrue(decisionAction.waitForExistence(timeout: 5))
        app.terminate()
        app.launch()
        XCTAssertTrue(decisionAction.waitForExistence(timeout: 30))
        capture(app, "Reopened inbox")
        app.buttons["openReview-mobile-smoke#2"].tap()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 20), app.debugDescription)
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("mobile-smoke#2"))
        openReviewMenu(app, "Review guide")
        XCTAssertTrue(app.staticTexts["No review guide"].waitForExistence(timeout: 5))
        capture(app, "Question item — open its real review without a guide")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launch()
        XCTAssertTrue(decisionAction.waitForExistence(timeout: 20))
        XCTAssertTrue(app.buttons["projectFilter"].isHittable)
        try auditInboxAccessibility(app)
        capture(app, "Inbox — dark mode")
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
