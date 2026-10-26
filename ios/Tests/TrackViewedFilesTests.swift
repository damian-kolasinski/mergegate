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
        XCTAssertTrue(app.descendants(matching: .any)["reviewStats"].firstMatch.label.contains("0 of 3 reviewed"))
        capture(app, "Viewed — file reader before marking")
        tapWhenStable(viewed)
        assertProgress(app, "1 of 3 viewed")
        XCTAssertEqual(viewed.value as? String, "Viewed")
        XCTAssertTrue(app.descendants(matching: .any)["reviewStats"].firstMatch.label.contains("1 of 3 reviewed"))
        XCTAssertTrue((app.textViews["unifiedDiff"].firstMatch.value as? String ?? "").contains("subtotal + delivery"))
        try audit(app)
        capture(app, "Viewed — confirmed by the real hub")
        tapWhenStable(app.buttons["reviewFiles"])
        XCTAssertTrue(app.buttons["reviewFile-0"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["reviewFile-0"].label.contains("Viewed"))
        // Trusted files fold into their own group, as on the web.
        XCTAssertFalse(app.buttons["reviewFile-3"].exists)
        tapWhenStable(app.descendants(matching: .any)["reviewTrustedFiles"].firstMatch)
        capture(app, "Viewed — tree with trusted files unfolded")
        tapWhenStable(app.buttons["reviewFile-3"])
        XCTAssertTrue(app.descendants(matching: .any)["trustedViewed"].firstMatch.waitForExistence(timeout: 5))
        XCTAssertFalse(viewed.exists)
        assertProgress(app, "1 of 3 viewed")
        capture(app, "Viewed — trusted files count automatically")
        tapWhenStable(app.navigationBars.buttons.element(boundBy: 0))
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 5))
        let inboxItem = app.descendants(matching: .any).matching(identifier: "inboxItem-mobile-smoke#2").firstMatch
        XCTAssertTrue(inboxItem.waitForExistence(timeout: 20), app.debugDescription)
        capture(app, "Viewed — back in the inbox")
        tapWhenStable(open)
        showFirstFile(app)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Viewed")
        assertProgress(app, "1 of 3 viewed")
        app.terminate()
        app.launch()
        XCTAssertTrue(open.waitForExistence(timeout: 20))
        tapWhenStable(open)
        showFirstFile(app)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Viewed")
        capture(app, "Viewed — persisted after process termination")
        tapWhenStable(viewed)
        assertProgress(app, "0 of 3 viewed")
        XCTAssertEqual(viewed.value as? String, "Not viewed")
        tapWhenStable(app.navigationBars.buttons.element(boundBy: 0))
        XCTAssertTrue(inboxItem.waitForExistence(timeout: 20))
        tapWhenStable(open)
        showFirstFile(app)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertEqual(viewed.value as? String, "Not viewed")
        assertProgress(app, "0 of 3 viewed")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<5 where !open.isHittable { app.swipeUp() }
        tapWhenStable(open)
        showFirstFile(app)
        XCTAssertTrue(viewed.waitForExistence(timeout: 20))
        XCTAssertTrue(viewed.isHittable)
        tapWhenStable(viewed)
        assertProgress(app, "1 of 3 viewed")
        let reader = app.textViews["unifiedDiff"].firstMatch
        let source = app.scrollViews.firstMatch
        for _ in 0..<3 { source.swipeUp() }
        XCTAssertTrue(reader.isHittable)
        capture(app, "Viewed — bars hidden while reading")
        source.swipeDown()
        XCTAssertTrue(viewed.waitForExistence(timeout: 5))
        XCTAssertTrue(viewed.isHittable, "Scrolling back up brings the review bar back.")
        try audit(app)
        capture(app, "Viewed — dark mode")
        // The bar's main action marks the open file and moves to the next one still to view.
        tapWhenStable(viewed)
        assertProgress(app, "0 of 3 viewed")
        let file = app.buttons["reviewFiles"].label
        let primary = app.buttons["reviewPrimary"]
        XCTAssertEqual(primary.label, "Viewed & next")
        tapWhenStable(primary)
        assertProgress(app, "1 of 3 viewed")
        let moved = NSPredicate(format: "label != %@", file)
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: moved, object: app.buttons["reviewFiles"])], timeout: 10), .completed)
        capture(app, "Viewed & next — moved to the next file")
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try auditReviewAccessibility(app)
    }

    // Reopening restores the last file read, which may be the trusted receipt with no mark.
    @MainActor private func showFirstFile(_ app: XCUIApplication) {
        let files = app.buttons["reviewFiles"]
        XCTAssertTrue(files.waitForExistence(timeout: 20), app.debugDescription)
        if files.label.hasPrefix("File 1 of") { return }
        tapWhenStable(files)
        tapWhenStable(app.buttons["reviewFile-0"])
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
