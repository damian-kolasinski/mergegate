import XCTest

final class ReReviewRevisionTests: XCTestCase {
    @MainActor func testCompareSinceReviewAndBaseWithCarryover() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_REREVIEW"] == "1"
        else { throw XCTSkip("Seed rereview-fixture.py and set MG_IOS_TEST_REREVIEW=1.") }
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
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r2"))
        XCTAssertEqual(app.buttons["reviewComparison"].label, "Compare: Since r1")
        XCTAssertEqual(app.staticTexts["reviewViewedProgress"].label, "1 of 2 viewed")
        XCTAssertTrue(app.staticTexts["Changed again"].exists)
        let reader = app.textViews["unifiedDiff"]
        XCTAssertTrue(reader.waitForExistence(timeout: 10))
        XCTAssertTrue((reader.value as? String ?? "").contains("let delivery = 15"))
        try audit(app)
        capture(app, "Re-review — since last review")
        app.buttons["reviewFiles"].tap()
        XCTAssertTrue(app.buttons["reviewFile-1"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["reviewFile-1"].label.contains("Viewed"))
        XCTAssertTrue(app.buttons["reviewFile-1"].label.contains("Unchanged"))
        XCTAssertTrue(app.buttons["reviewFile-2"].label.contains("Change dropped"))
        capture(app, "Re-review — unchanged policy retains viewed state")
        app.buttons["reviewFile-1"].tap()
        XCTAssertEqual(app.buttons["reviewViewed"].value as? String, "Viewed")
        app.buttons["reviewFiles"].tap()
        app.buttons["reviewFile-0"].tap()
        app.buttons["fileConversations"].tap()
        XCTAssertTrue(app.buttons["conversation-c_1"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["conversation-c_1"].label.contains("Moved from r1"))
        XCTAssertTrue(app.buttons["conversation-c_2"].label.contains("Source changed since r1"))
        app.buttons["conversation-c_1"].tap()
        XCTAssertTrue(app.staticTexts["threadLocation"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["threadLocation"].label.contains("Line 4"))
        XCTAssertEqual(app.staticTexts["threadStatus"].label, "Addressed in r2")
        capture(app, "Re-review — carried comment at its new source line")
        app.buttons["closeConversations"].tap()
        app.buttons["reviewConversations"].tap()
        XCTAssertTrue(app.buttons["conversation-c_3"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["conversation-c_3"].label.contains("Outdated"))
        capture(app, "Re-review — outdated conversation retained")
        app.buttons["closeConversations"].tap()
        app.buttons["reviewComparison"].tap()
        app.buttons["compareAll"].tap()
        waitForComparison(app, "Compare: All changes")
        XCTAssertEqual(app.buttons["reviewComparison"].label, "Compare: All changes")
        XCTAssertTrue((reader.value as? String ?? "").contains("−\tlet total = subtotal"))
        XCTAssertEqual(app.staticTexts["reviewViewedProgress"].label, "1 of 2 viewed")
        capture(app, "Re-review — all changes against base")
        app.buttons["reviewComparison"].tap()
        app.buttons["compareSince-1"].tap()
        waitForComparison(app, "Compare: Since r1")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
        app.launch()
        try openReview(app)
        XCTAssertEqual(app.buttons["reviewComparison"].label, "Compare: Since r1")
        // Start inside the reading area: an app-wide swipe starts on the large fixed footer.
        let scroll = app.scrollViews["reviewScroll"]
        XCTAssertTrue(scroll.exists)
        for _ in 0..<8 where reader.frame.height == 0 || reader.frame.maxY > app.buttons["reviewViewed"].frame.minY {
            let top = app.navigationBars.firstMatch.frame.maxY + 20
            let bottom = app.buttons["reviewViewed"].frame.minY - 30
            let start = app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: app.frame.midX, dy: bottom))
            let end = app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: app.frame.midX, dy: top))
            start.press(forDuration: 0.05, thenDragTo: end)
        }
        XCTAssertGreaterThan(reader.frame.height, 0)
        XCTAssertTrue(reader.isHittable, "Source remains reachable.")
        XCTAssertLessThanOrEqual(reader.frame.maxY, app.buttons["reviewViewed"].frame.minY)
        capture(app, "Re-review — dark mode")
        try audit(app, largeText: true)
    }

    @MainActor private func waitForComparison(_ app: XCUIApplication, _ label: String) {
        let predicate = NSPredicate(format: "label == %@", label)
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: predicate, object: app.buttons["reviewComparison"])], timeout: 10), .completed)
    }

    @MainActor private func openReview(_ app: XCUIApplication) throws {
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        let open = app.buttons["openReview-mobile-smoke#2"]
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["reviewComparison"].waitForExistence(timeout: 20))
    }

    @MainActor private func audit(_ app: XCUIApplication, largeText: Bool = false) throws {
        try app.performAccessibilityAudit(for: [.contrast, .hitRegion, .sufficientElementDescription]) { issue in
            guard issue.auditType == .contrast, let element = issue.element,
                element.elementType == .staticText,
                app.scrollViews["reviewScroll"].staticTexts.matching(NSPredicate(format: "label == %@", element.label)).count > 0
            else { return false }
            // XCTest includes obscured scroll labels. A label with no hittable rendered
            // area has no contrast to measure; visible labels still require measured contrast.
            let top = app.navigationBars.firstMatch.frame.maxY
            let bottom = app.buttons["reviewViewed"].frame.minY - 12
            let viewport = CGRect(x: app.frame.minX, y: top, width: app.frame.width, height: max(0, bottom - top))
            guard element.isHittable, viewport.contains(element.frame) else { return true }
            let rendered = element.screenshot()
            let ratio = InboxRenderedContrast.measure(rendered.image)
            XCTContext.runActivity(named: "Visible review label contrast: \(ratio ?? 0)") { activity in
                let attachment = XCTAttachment(screenshot: rendered)
                attachment.lifetime = .keepAlways
                activity.add(attachment)
            }
            // WCAG requires 3:1 for large text; the largest Dynamic Type category exceeds 18pt.
            return ratio.map { $0 >= (largeText ? 3 : 4.5) } ?? false
        }
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
