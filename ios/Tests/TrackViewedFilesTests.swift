import XCTest

final class TrackViewedFilesTests: XCTestCase {
    @MainActor
    func testViewedProgressPersistsThroughInboxAndRelaunch() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_VIEWED"] == "1" else {
            throw XCTSkip("Run the disposable review/viewed fixtures and set MG_IOS_TEST_VIEWED=1.")
        }
        let originalAppearance = XCUIDevice.shared.appearance
        defer { XCUIDevice.shared.appearance = originalAppearance }
        XCUIDevice.shared.appearance = .light
        let app = XCUIApplication()
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
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
        XCTAssertTrue(open.waitForExistence(timeout: 20), app.debugDescription)
        open.tap()
        let viewed = app.buttons["reviewViewed"]
        XCTAssertTrue(viewed.waitForExistence(timeout: 20), app.debugDescription)
        // Normalize a repeated verification run using the same real control.
        if viewed.value as? String == "Viewed" { tapWhenStable(viewed) }
        assertProgress(app, "0 of 3 viewed")
        XCTAssertTrue(app.staticTexts["3 files left"].exists)
        capture(app, "Viewed — file reader before marking")
        tapWhenStable(viewed)
        assertProgress(app, "1 of 3 viewed")
        XCTAssertEqual(viewed.value as? String, "Viewed")
        XCTAssertTrue(app.staticTexts["2 files left"].exists)
        XCTAssertTrue((app.textViews["unifiedDiff"].value as? String ?? "").contains("subtotal + delivery"))
        try audit(app)
        capture(app, "Viewed — confirmed by the real hub")
        tapWhenStable(app.buttons["reviewFiles"])
        XCTAssertTrue(app.buttons["reviewFile-0"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["reviewFile-0"].label.contains("Viewed"))
        capture(app, "Viewed — importance-ordered file list")
        tapWhenStable(app.buttons["reviewFile-3"])
        XCTAssertTrue(app.staticTexts["trustedViewed"].waitForExistence(timeout: 5))
        XCTAssertFalse(viewed.exists)
        assertProgress(app, "1 of 3 viewed")
        capture(app, "Viewed — trusted files count automatically")
        tapWhenStable(app.navigationBars.buttons.element(boundBy: 0))
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 5))
        let inboxItem = app.descendants(matching: .any).matching(identifier: "inboxItem-mobile-smoke#2").firstMatch
        XCTAssertTrue(inboxItem.staticTexts["2 of 3 review files remaining"].waitForExistence(timeout: 20), app.debugDescription)
        capture(app, "Viewed — progress back in the Decision desk")
        tapWhenStable(open)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Viewed")
        assertProgress(app, "1 of 3 viewed")
        app.terminate()
        app.launch()
        XCTAssertTrue(open.waitForExistence(timeout: 20))
        tapWhenStable(open)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Viewed")
        capture(app, "Viewed — persisted after process termination")
        tapWhenStable(viewed)
        assertProgress(app, "0 of 3 viewed")
        XCTAssertEqual(viewed.value as? String, "Not viewed")
        tapWhenStable(app.navigationBars.buttons.element(boundBy: 0))
        XCTAssertTrue(inboxItem.staticTexts["3 of 3 review files remaining"].waitForExistence(timeout: 20))
        tapWhenStable(open)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Not viewed")
        assertProgress(app, "0 of 3 viewed")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<5 where !open.isHittable { app.swipeUp() }
        tapWhenStable(open)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertTrue(viewed.isHittable)
        tapWhenStable(viewed)
        assertProgress(app, "1 of 3 viewed")
        let reader = app.textViews["unifiedDiff"]
        let source = app.scrollViews.firstMatch
        for _ in 0..<3 { source.swipeUp() }
        XCTAssertTrue(reader.isHittable)
        XCTAssertTrue(viewed.isHittable)
        try audit(app)
        capture(app, "Viewed — dark mode at the largest text size")
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription]) { issue in
            print("Accessibility: \(issue.detailedDescription)\n\(issue.element?.debugDescription ?? "No element")")
            return false
        }
    }

    @MainActor private func assertProgress(_ app: XCUIApplication, _ value: String) {
        let progress = app.staticTexts["reviewViewedProgress"]
        let expected = NSPredicate(format: "label == %@", value)
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: expected, object: progress)], timeout: 15), .completed, app.debugDescription)
    }

    @MainActor private func tapWhenStable(_ element: XCUIElement) {
        var previous: CGRect?
        let stable = NSPredicate { _, _ in
            let frame = element.frame
            defer { previous = frame }
            return element.isHittable && !frame.isEmpty && frame == previous
        }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: stable, object: element)], timeout: 8), .completed)
        element.tap()
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
