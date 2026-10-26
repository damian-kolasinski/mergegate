import XCTest

final class KeepCommentDraftsTests: XCTestCase {
    @MainActor func testDraftsOnBothSidesSurviveTerminationEditingAndDeletion() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_DRAFTS"] == "1"
        else {
            throw XCTSkip("Run drafts-fixture.py on a disposable smoke hub and set MG_IOS_TEST_DRAFTS=1.")
        }
        let appearance = XCUIDevice.shared.appearance
        defer { XCUIDevice.shared.appearance = appearance }
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
        try openReview(app)
        openReviewMenu(app, "Comment drafts")
        XCTAssertTrue(app.scrollViews["draftList"].waitForExistence(timeout: 5))
        for _ in 0..<20 {
            let existing = app.buttons.matching(
                NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-")
            ).firstMatch
            if !existing.waitForExistence(timeout: 1) { break }
            existing.tap()
            app.buttons["deleteDraft"].tap()
            XCTAssertTrue(app.buttons["Delete permanently"].waitForExistence(timeout: 5))
            app.buttons["Delete permanently"].tap()
        }
        app.buttons["closeDrafts"].tap()
        capture(app, "Drafts — real File reader")
        try comment(app, y: 69)
        XCTAssertEqual(app.staticTexts["draftAnchor"].label, "Before · Line 3", app.debugDescription)
        XCTAssertEqual(app.staticTexts["draftQuote"].label, "Source quote: let total = subtotal")
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText("Should delivery be included here?")
        capture(app, "Drafts — before source saved while typing")
        // Terminate with the composer still open: no Done/save action is allowed to hide lost edits.
        app.terminate()
        app.launch()
        try openReview(app)
        openReviewMenu(app, "Comment drafts")
        let oldDraft = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-")
        ).firstMatch
        XCTAssertTrue(oldDraft.waitForExistence(timeout: 5), app.debugDescription)
        oldDraft.tap()
        XCTAssertEqual(app.staticTexts["draftAnchor"].label, "Before · Line 3")
        XCTAssertEqual(app.textViews["draftBody"].value as? String, "Should delivery be included here?")
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText(" Check the charged total.")
        let editedBody = try XCTUnwrap(app.textViews["draftBody"].value as? String)
        XCTAssertTrue(editedBody.contains("Should delivery be included here?"))
        XCTAssertTrue(editedBody.contains("Check the charged total."))
        app.buttons["closeDrafts"].tap()
        try comment(app, y: 90, text: "é 🐙")
        XCTAssertEqual(app.staticTexts["draftAnchor"].label, "After · Line 3", app.debugDescription)
        XCTAssertTrue(app.staticTexts["draftQuote"].label.contains("é 🐙"))
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText("Keep the delivery charge consistent with the receipt.")
        app.buttons["closeDrafts"].tap()
        app.buttons["reviewFiles"].tap()
        XCTAssertTrue(app.buttons["reviewFile-1"].waitForExistence(timeout: 5))
        app.buttons["reviewFile-1"].tap()
        openReviewMenu(app, "Comment drafts")
        XCTAssertTrue(app.scrollViews["draftList"].waitForExistence(timeout: 5))
        XCTAssertEqual(
            app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-")).count,
            2)
        capture(app, "Drafts — both sides across file navigation")
        try audit(app)
        app.buttons["closeDrafts"].tap()
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL",
        ]
        app.launch()
        try openReview(app)
        openReviewMenu(app, "Comment drafts")
        let rows = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-"))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 5))
        XCTAssertEqual(rows.count, 2)
        capture(app, "Drafts — restored in dark mode")
        try audit(app)
        rows.firstMatch.tap()
        capture(app, "Drafts — accessible comment editor")
        try audit(app)
        app.buttons["deleteDraft"].tap()
        let delete = app.buttons["Delete permanently"].firstMatch
        XCTAssertTrue(delete.waitForExistence(timeout: 5))
        delete.tap()
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 5))
        XCTAssertEqual(rows.count, 1)
        app.buttons["closeDrafts"].tap()
        app.terminate()
        app.launch()
        try openReview(app)
        openReviewMenu(app, "Comment drafts")
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 5))
        XCTAssertEqual(rows.count, 1)
        rows.firstMatch.tap()
        XCTAssertEqual(app.staticTexts["draftAnchor"].label, "Before · Line 3")
        XCTAssertEqual(
            app.textViews["draftBody"].value as? String,
            editedBody)
        app.buttons["deleteDraft"].tap()
        app.buttons["Delete permanently"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["No comment drafts"].waitForExistence(timeout: 5))
        capture(app, "Drafts — deleted without submitting a review")
    }

    @MainActor private func openReview(_ app: XCUIApplication) throws {
        let open = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20), app.debugDescription)
        for _ in 0..<8 {
            if open.waitForExistence(timeout: 1) && open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.exists && open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["reviewMore"].waitForExistence(timeout: 20), app.debugDescription)
        XCTAssertTrue(app.textViews["unifiedDiff"].firstMatch.waitForExistence(timeout: 10))
    }

    /// Long-presses the source line at `y` in the first block, or, once a card has split the
    /// hunk below an earlier line, the same line at the top of the block holding `text`.
    @MainActor private func comment(_ app: XCUIApplication, y: CGFloat, text: String? = nil) throws {
        let first = app.textViews["unifiedDiff"].firstMatch
        var reader = first
        var offset = y
        if let text {
            let holding = app.textViews.matching(
                NSPredicate(format: "identifier == %@ AND value CONTAINS %@", "unifiedDiff", text)
            ).firstMatch
            if holding.exists, holding.frame != first.frame {
                reader = holding
                offset = 14
            }
        }
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 165, dy: offset)).press(
            forDuration: 1.2)
        let menu = app.menuItems["Comment"]
        let button = app.buttons["Comment"]
        XCTAssertTrue(menu.waitForExistence(timeout: 4) || button.exists, app.debugDescription)
        if menu.exists { menu.tap() } else { button.tap() }
        XCTAssertTrue(app.textViews["draftBody"].waitForExistence(timeout: 5), app.debugDescription)
    }

    @MainActor private func audit(_ app: XCUIApplication) throws {
        try auditReviewAccessibility(app)
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
