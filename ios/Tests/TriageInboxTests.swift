import XCTest

final class TriageInboxTests: XCTestCase {
    @MainActor
    func testTriageRealInbox() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_TRIAGE"] == "1" else {
            throw XCTSkip("Run the disposable triage fixture and set MG_IOS_TEST_TRIAGE=1.")
        }
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
        XCTAssertTrue(app.staticTexts["Which clients still need the legacy format?"].waitForExistence(timeout: 30), app.debugDescription)
        XCTAssertTrue(app.staticTexts["1 of 2 review files remaining"].exists)
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription])
        capture(app, "Decision desk — question and remaining work")
        app.buttons["decisionDetails-mobile-smoke#2"].tap()
        XCTAssertTrue(app.staticTexts["Keep compatibility with the previous response format."].waitForExistence(timeout: 5))
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.staticTexts["Keep compatibility with the previous response format."].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["closeDecisionDetails"].exists)
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription])
        capture(app, "Read the real question")
        app.buttons["closeDecisionDetails"].tap()
        app.buttons["actionFilter-Review"].tap()
        XCTAssertTrue(app.staticTexts["Connect and reopen verification"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Which clients still need the legacy format?"].exists)
        app.buttons["actionFilter-Decide"].tap()
        XCTAssertTrue(app.staticTexts["Which clients still need the legacy format?"].waitForExistence(timeout: 5))
        app.buttons["actionFilter-All states"].tap()
        app.buttons["projectFilter"].tap()
        app.buttons["companion"].tap()
        XCTAssertTrue(app.staticTexts["Retire the old endpoint"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Which clients still need the legacy format?"].exists)
        capture(app, "Project filter — companion closed item")
        app.buttons["projectFilter"].tap()
        app.buttons["All projects"].tap()
        app.buttons["stateFilter"].tap()
        app.buttons["In progress"].tap()
        XCTAssertTrue(app.staticTexts["Retry failed connections"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Choose the response format"].exists)
        capture(app, "State filter — work in progress")
        app.buttons["stateFilter"].tap()
        app.buttons["Closed today"].tap()
        XCTAssertTrue(app.staticTexts["Retire the old endpoint"].waitForExistence(timeout: 5))
        app.buttons["stateFilter"].tap()
        app.buttons["Merging"].tap()
        XCTAssertTrue(app.staticTexts["No matching items"].waitForExistence(timeout: 5))
        app.buttons["stateFilter"].tap()
        app.buttons["All states"].tap()
        app.terminate()
        app.launch()
        XCTAssertTrue(app.staticTexts["Which clients still need the legacy format?"].waitForExistence(timeout: 30))
        capture(app, "Reopened Decision desk")
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
