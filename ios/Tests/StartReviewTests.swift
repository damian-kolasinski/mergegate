import XCTest

final class StartReviewTests: XCTestCase {
    @MainActor
    func testStartRealReview() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_REVIEW"] == "1" else {
            throw XCTSkip("Run review-fixture.py on a disposable smoke hub and set MG_IOS_TEST_REVIEW=1.")
        }
        let app = XCUIApplication()
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
        if !open.waitForExistence(timeout: 15) { app.swipeUp() }
        XCTAssertTrue(open.waitForExistence(timeout: 15), app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 20), app.debugDescription)
        XCTAssertEqual(app.buttons["reviewFiles"].label, "File 1 of 4, z-checkout.swift")
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r1 → main"))
        XCTAssertTrue(app.staticTexts["All changes → main"].exists && app.staticTexts["4 files left"].exists)
        let reader = app.textViews["unifiedDiff"]
        XCTAssertTrue(reader.waitForExistence(timeout: 5))
        XCTAssertTrue((reader.value as? String ?? "").contains("subtotal + delivery"), app.debugDescription)
        XCTAssertTrue((reader.value as? String ?? "").contains("@@ -0,0 +1,8 @@"))
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription])
        capture(app, "Start review — first real unified diff")
        tapWhenStable(app.buttons["reviewGuide"])
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "amount shown matches the amount charged")).firstMatch.waitForExistence(timeout: 5))
        capture(app, "Review guide — full intent and reading order")
        tapWhenStable(app.buttons["closeReviewGuide"])
        tapWhenStable(app.buttons["reviewFiles"])
        let first = app.buttons["reviewFile-0"]
        XCTAssertTrue(first.waitForExistence(timeout: 5))
        XCTAssertTrue(first.label.contains("z-checkout.swift"), first.label)
        XCTAssertTrue(first.label.contains("Review the total calculation first."), first.label)
        XCTAssertTrue(app.buttons["reviewFile-1"].label.contains("README.md"))
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription])
        capture(app, "Importance-ordered files and reasons")
        tapWhenStable(app.buttons["reviewFile-2"])
        XCTAssertTrue((reader.value as? String ?? "").contains("Region 80:"))
        for _ in 0..<12 where reader.frame.maxY > app.frame.maxY { app.swipeUp() }
        XCTAssertLessThanOrEqual(reader.frame.maxY, app.frame.maxY, "The final source line must remain reachable.")
        capture(app, "Long diff — final source line")
        for _ in 0..<12 where !app.buttons["reviewFiles"].isHittable { app.swipeDown() }
        tapWhenStable(app.buttons["reviewFiles"])
        XCTAssertTrue(app.buttons["reviewFile-1"].waitForExistence(timeout: 5))
        tapWhenStable(app.buttons["reviewFile-1"])
        XCTAssertTrue(app.buttons["reviewFiles"].isHittable, "Changing files resets the reading position.")
        XCTAssertEqual(app.buttons["reviewFiles"].label, "File 2 of 4, README.md")
        XCTAssertTrue((reader.value as? String ?? "").contains("−\t# Mobile verification"))
        XCTAssertTrue((reader.value as? String ?? "").contains("+\t# Mobile review verification"))
        capture(app, "Unified diff — additions and removals")
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.buttons["reviewFiles"].label, "File 2 of 4, README.md")
        tapWhenStable(app.buttons["reviewFiles"])
        XCTAssertTrue(app.buttons["reviewFile-3"].waitForExistence(timeout: 5))
        tapWhenStable(app.buttons["reviewFile-3"])
        XCTAssertTrue(app.staticTexts["Binary file"].waitForExistence(timeout: 5))
        capture(app, "Binary file boundary")
        tapWhenStable(app.navigationBars.buttons.element(boundBy: 0))
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 5))
        open.tap()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 20))
        XCTAssertEqual(app.buttons["reviewFiles"].label, "File 1 of 4, z-checkout.swift")
        app.terminate()
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL", "-AppleInterfaceStyle", "Light"]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<5 where !open.isHittable { app.swipeUp() }
        tapWhenStable(open)
        XCTAssertTrue(reader.waitForExistence(timeout: 20))
        XCTAssertTrue((reader.value as? String ?? "").contains("struct Checkout"))
        for _ in 0..<5 where reader.frame.minY > app.frame.maxY - 100 { app.swipeUp() }
        XCTAssertLessThan(reader.frame.minY, app.frame.maxY - 100, "Source must remain reachable at the largest text size.")
        capture(app, "Review at the largest accessibility text size")
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
