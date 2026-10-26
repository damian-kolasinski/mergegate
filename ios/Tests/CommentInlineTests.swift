import XCTest

final class CommentInlineTests: XCTestCase {
    @MainActor func testGutterCommentInlineDraftAndReviewComment() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"),
            ProcessInfo.processInfo.environment["MG_IOS_TEST_INLINE"] == "1"
        else {
            throw XCTSkip("Run drafts-fixture.py on a disposable smoke hub and set MG_IOS_TEST_INLINE=1.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryL"]
        app.launch()
        if !app.buttons["hubSettings"].waitForExistence(timeout: 3) {
            let field = app.textFields["hubURL"]
            XCTAssertTrue(field.waitForExistence(timeout: 5))
            field.tap()
            field.typeText(address)
            app.swipeDown()
            app.buttons["connectHub"].tap()
        }
        let open = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(open.waitForExistence(timeout: 20), app.debugDescription)
        open.tap()
        let reader = app.textViews["unifiedDiff"].firstMatch
        XCTAssertTrue(reader.waitForExistence(timeout: 20), app.debugDescription)

        // A tap on a line number picks that line; the pill offers to comment on it.
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 16, dy: 34)).tap()
        let pill = app.buttons["commentOnLines"]
        XCTAssertTrue(pill.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertTrue(pill.label.hasPrefix("Comment on "), pill.label)
        capture(app, "Inline — a tapped line number offers a comment")
        pill.tap()
        let body = app.textViews["draftBody"]
        XCTAssertTrue(body.waitForExistence(timeout: 5), app.debugDescription)
        body.tap()
        body.typeText("Picked from the gutter.")
        app.buttons["closeDrafts"].tap()

        // The draft shows under its line until it is sent.
        let card = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "inlineDraft-"))
            .firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 10), app.debugDescription)
        XCTAssertTrue(card.label.contains("Picked from the gutter."), card.label)
        capture(app, "Inline — the draft under its line")
        card.tap()
        XCTAssertEqual(app.textViews["draftBody"].value as? String, "Picked from the gutter.")
        app.buttons["closeDrafts"].tap()

        // A comment on the whole review, written mid-review and kept for whichever outcome.
        app.buttons["reviewComment"].tap()
        let comment = app.textViews["reviewComment"]
        XCTAssertTrue(comment.waitForExistence(timeout: 5), app.debugDescription)
        comment.typeText("Whole review: looks right.")
        capture(app, "Inline — review comment")
        app.buttons["closeReviewComment"].tap()
        openCompletion(app)
        XCTAssertEqual(
            app.textViews["feedbackSummary"].value as? String, "Whole review: looks right.")
        XCTAssertTrue(app.staticTexts["Picked from the gutter."].exists)
        try auditReviewAccessibility(app)
        app.buttons["closeFeedback"].tap()
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
