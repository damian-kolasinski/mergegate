import XCTest

final class ResumeOfflineTests: XCTestCase {
    @MainActor func testCachedHighlightedReviewDraftPositionAndRevisionReconciliation() async throws {
        continueAfterFailure = false
        let environment = ProcessInfo.processInfo.environment
        let address = environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), environment["MG_IOS_TEST_OFFLINE"] == "1"
        else { throw XCTSkip("Seed offline-fixture.py and set MG_IOS_TEST_OFFLINE=1.") }
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
        let reader = app.textViews["unifiedDiff"].firstMatch
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 150, dy: 90)).press(
            forDuration: 1.2)
        let menu = app.menuItems["Comment"]
        let button = app.buttons["Comment"]
        XCTAssertTrue(menu.waitForExistence(timeout: 4) || button.exists, app.debugDescription)
        if menu.exists { menu.tap() } else { button.tap() }
        XCTAssertTrue(app.textViews["draftBody"].waitForExistence(timeout: 5))
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText("Check delivery once. é 🐙")
        app.buttons["closeDrafts"].tap()
        app.buttons["loadSourceContext"].tap()
        let expand = app.buttons["expandContext-0"]
        XCTAssertTrue(expand.waitForExistence(timeout: 10))
        expand.tap()
        app.scrollViews["reviewScroll"].swipeUp()
        let onlineSourceY = app.textViews["unifiedDiff"].firstMatch.frame.minY
        XCTAssertLessThan(onlineSourceY, 100, "The test must leave the reader scrolled into source.")
        capture(app, "Offline 01 — source and highlighting saved with reading position")
        _ = try await control("pause")
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 5))
        XCTAssertTrue(
            app.staticTexts["Offline · saved inbox"].waitForExistence(timeout: 25), app.debugDescription)
        capture(app, "Offline 02 — saved Decision desk after relaunch without hub")
        try openReview(app)
        XCTAssertFalse(app.buttons["loadSourceContext"].exists, app.debugDescription)
        XCTAssertTrue(app.textViews["unifiedDiff"].firstMatch.exists)
        // The expanded prefix remains in the document after relaunch, with its Unicode source intact.
        XCTAssertTrue(
            (app.textViews["unifiedDiff"].firstMatch.value as? String ?? "").contains(
                "original source context"))
        let restoredSource = app.textViews["unifiedDiff"].firstMatch
        let positionRestored = NSPredicate { _, _ in restoredSource.frame.minY < 150 }
        let restored = XCTNSPredicateExpectation(predicate: positionRestored, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [restored], timeout: 10), .completed)
        XCTAssertLessThan(
            abs(restoredSource.frame.minY - onlineSourceY), 160,
            "The saved scroll offset should survive source relayout and the offline notice.")
        capture(app, "Offline 03 — restored source context highlighting and position")
        app.buttons["reviewDrafts"].tap()
        let draft = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-")
        ).firstMatch
        XCTAssertTrue(draft.waitForExistence(timeout: 5))
        draft.tap()
        XCTAssertEqual(app.textViews["draftBody"].value as? String, "Check delivery once. é 🐙")
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText(" Edited offline.")
        app.buttons["closeDrafts"].tap()
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 5))
        app.textViews["feedbackSummary"].tap()
        app.textViews["feedbackSummary"].typeText("Offline summary stays with r1.")
        XCTAssertFalse(app.buttons["sendFeedback"].isEnabled)
        capture(app, "Offline 04 — local feedback retained and sending unavailable")
        app.buttons["closeFeedback"].tap()
        _ = try await control("resume-advance")
        app.scrollViews["reviewScroll"].swipeDown()
        XCTAssertTrue(
            app.buttons["openLatestRevision"].waitForExistence(timeout: 25), app.debugDescription)
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r1"))
        capture(app, "Offline 05 — reconnect finds r2 and keeps r1 work")
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 5))
        XCTAssertEqual(
            app.textViews["feedbackSummary"].value as? String, "Offline summary stays with r1.")
        XCTAssertTrue(app.staticTexts["feedbackError"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["sendFeedback"].isEnabled)
        XCTAssertTrue(app.staticTexts["feedbackError"].label.contains("newer revision"))
        _ = try await control("verify-stale")
        capture(app, "Offline 06 — stale submission blocked with drafts preserved")
        app.buttons["closeFeedback"].tap()
        app.buttons["openLatestRevision"].tap()
        XCTAssertTrue(app.staticTexts["reviewIdentity"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r2"))
        app.buttons["reviewFeedback"].tap()
        XCTAssertTrue(app.textViews["feedbackSummary"].waitForExistence(timeout: 5))
        app.textViews["feedbackSummary"].tap()
        app.textViews["feedbackSummary"].typeText("Current revision checked after reconnect.")
        XCTAssertTrue(app.buttons["sendFeedback"].waitForExistence(timeout: 10))
        app.buttons["sendFeedback"].tap()
        XCTAssertTrue(app.staticTexts["feedbackSent"].waitForExistence(timeout: 15))
        _ = try await control("verify-sent")
        capture(app, "Offline 07 — current revision feedback verified on real hub")
        app.buttons["closeFeedback"].tap()
        try audit(app)
        _ = try await control("pause")
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL",
        ]
        app.launch()
        try openReview(app)
        for _ in 0..<5 { app.scrollViews["reviewScroll"].swipeDown() }
        try audit(app)
        let offlineDetail = app.staticTexts["offlineReviewDetail"]
        let reconnect = app.buttons["reconnectReview"]
        for _ in 0..<14 {
            let frame = offlineDetail.frame
            let top = app.navigationBars.firstMatch.frame.maxY + 8
            let bottom = app.buttons["reviewViewed"].frame.minY - 8
            if frame.minY >= top && reconnect.frame.maxY <= bottom { break }
            let scroll = app.scrollViews["reviewScroll"]
            let start = scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            let end = scroll.coordinate(
                withNormalizedOffset: CGVector(dx: 0.5, dy: frame.minY < top ? 0.65 : 0.35))
            start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.2)
        }
        XCTAssertTrue(offlineDetail.isHittable)
        XCTAssertGreaterThanOrEqual(offlineDetail.frame.minY, app.navigationBars.firstMatch.frame.maxY)
        XCTAssertLessThanOrEqual(reconnect.frame.maxY, app.buttons["reviewViewed"].frame.minY)
        capture(app, "Offline 08 — dark mode")
        try audit(app)
        app.terminate()
    }

    @MainActor private func openReview(_ app: XCUIApplication) throws {
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 25))
        let open = app.buttons["openReview-mobile-smoke#2"]
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["reviewDrafts"].waitForExistence(timeout: 25), app.debugDescription)
        XCTAssertTrue(app.textViews["unifiedDiff"].firstMatch.waitForExistence(timeout: 10))
    }

    @MainActor private func control(_ path: String) async throws -> [String: Any] {
        let address = try XCTUnwrap(ProcessInfo.processInfo.environment["MG_IOS_TEST_CONTROL_URL"])
        var request = URLRequest(url: try XCTUnwrap(URL(string: address)?.appending(path: path)))
        request.httpMethod = "POST"
        let (data, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual(
            (response as? HTTPURLResponse)?.statusCode, 200, String(decoding: data, as: UTF8.self))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try app.performAccessibilityAudit(for: [
            .contrast, .hitRegion, .sufficientElementDescription,
        ]) { issue in
            if issue.auditType == .contrast, let element = issue.element,
                element.elementType == .staticText || element.elementType == .button
            {
                let scroll = app.scrollViews["reviewScroll"]
                let top = app.navigationBars.firstMatch.frame.maxY
                let bottom = app.buttons["reviewViewed"].frame.minY
                let visible = CGRect(
                    x: scroll.frame.minX, y: top, width: scroll.frame.width, height: bottom - top)
                let belongsToReader = scroll.descendants(matching: element.elementType).matching(
                    NSPredicate(format: "label == %@", element.label)
                )
                .allElementsBoundByIndex.contains { $0.frame == element.frame }
                // The audit reports scrolled text behind the navigation glass and bottom bar.
                // Its pixels are occluded; audit the title at the top and the detail fully in view.
                if belongsToReader && !visible.contains(element.frame) {
                    XCTContext.runActivity(named: "Occluded reader text contrast") { activity in
                        let report = XCTAttachment(string: issue.detailedDescription)
                        report.lifetime = .keepAlways
                        activity.add(report)
                    }
                    return true
                }
            }
            guard issue.auditType == .contrast, let element = issue.element,
                element.elementType == .staticText, element.identifier == "offlineReviewDetail"
            else { return false }
            // XCTest can misclassify large text; require contrast in its actual rendered crop.
            let screenshot = element.screenshot()
            let ratio = InboxRenderedContrast.measure(screenshot.image)
            XCTContext.runActivity(
                named:
                    "Offline detail contrast: \(ratio.map { String(format: "%.2f:1", $0) } ?? "unmeasurable")"
            ) { activity in
                let crop = XCTAttachment(screenshot: screenshot)
                crop.lifetime = .keepAlways
                activity.add(crop)
            }
            return ratio.map { $0 >= 4.5 } ?? false
        }
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
