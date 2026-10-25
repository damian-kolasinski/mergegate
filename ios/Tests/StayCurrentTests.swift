import XCTest

final class StayCurrentTests: XCTestCase {
    @MainActor func testLiveInboxReplyRevisionAndReconnectPreserveActiveDraft() async throws {
        let environment = ProcessInfo.processInfo.environment
        let address = environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), environment["MG_IOS_TEST_LIVE_UPDATES"] == "1"
        else { throw XCTSkip("Seed live-updates-fixture.py and set MG_IOS_TEST_LIVE_UPDATES=1.") }
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
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        capture(app, "Live updates — Decision desk before arrival")
        _ = try await control("new-item")
        let arrival = app.buttons["openReview-mobile-smoke#3"]
        for _ in 0..<8 {
            if arrival.waitForExistence(timeout: 1) && arrival.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(arrival.exists, app.debugDescription)
        capture(app, "Live updates — new item arrives without refresh")
        let open = app.buttons["openReview-mobile-smoke#2"]
        for _ in 0..<8 {
            if open.isHittable { break }
            app.swipeDown()
        }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.textViews["unifiedDiff"].waitForExistence(timeout: 20))
        app.buttons["loadSourceContext"].tap()
        XCTAssertTrue(
            app.staticTexts["All source context is already visible"].waitForExistence(timeout: 10))
        app.buttons["reviewConversations"].tap()
        XCTAssertTrue(app.buttons["conversation-c_1"].waitForExistence(timeout: 10))
        app.buttons["conversation-c_1"].tap()
        XCTAssertTrue(app.staticTexts["threadStatus"].waitForExistence(timeout: 5))
        _ = try await control("reply")
        let reply = app.staticTexts[
            "Delivery is included in the receipt. Unicode é 🐙 stays intact."]
        XCTAssertTrue(reply.waitForExistence(timeout: 15), app.debugDescription)
        capture(app, "Live updates — reply arrives inside the open thread")
        app.navigationBars["Thread"].buttons["Conversations"].tap()
        _ = try await control("comment")
        XCTAssertTrue(
            app.buttons["conversation-c_2"].waitForExistence(timeout: 15), app.debugDescription)
        capture(app, "Live updates — sent review comments arrive in the open list")
        app.buttons["closeConversations"].tap()
        let reader = app.textViews["unifiedDiff"]
        reader.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 165, dy: 69)).press(
            forDuration: 1.2)
        let menu = app.menuItems["Comment"]
        let button = app.buttons["Comment"]
        XCTAssertTrue(menu.waitForExistence(timeout: 4) || button.exists, app.debugDescription)
        if menu.exists { menu.tap() } else { button.tap() }
        XCTAssertTrue(app.textViews["draftBody"].waitForExistence(timeout: 5))
        let body = "Keep this r1 draft while new revisions arrive. é 🐙"
        app.textViews["draftBody"].tap()
        app.textViews["draftBody"].typeText(body)
        _ = try await control("advance")
        XCTAssertEqual(app.textViews["draftBody"].value as? String, body)
        XCTAssertTrue(app.staticTexts["draftScope"].label.contains("r1"), app.debugDescription)
        capture(app, "Live updates — active r1 draft survives r2 arrival")
        app.buttons["closeDrafts"].tap()
        XCTAssertTrue(
            app.staticTexts["New revision r2 available"].waitForExistence(timeout: 15),
            app.debugDescription)
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r1"))
        XCTAssertTrue(app.staticTexts["All source context is already visible"].exists)
        capture(app, "Live updates — r2 notice keeps the r1 File reader")
        try audit(app)
        _ = try await control("pause")
        XCTAssertTrue(
            app.staticTexts["reviewReconnecting"].waitForExistence(timeout: 15)
                || app.otherElements["reviewReconnecting"].exists, app.debugDescription)
        capture(app, "Live updates — hub disconnected, review retained")
        _ = try await control("resume")
        XCTAssertTrue(
            app.staticTexts["New revision r3 available"].waitForExistence(timeout: 20),
            app.debugDescription)
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r1"))
        app.buttons["reviewDrafts"].tap()
        let draft = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "commentDraft-")
        ).firstMatch
        XCTAssertTrue(draft.waitForExistence(timeout: 5))
        draft.tap()
        XCTAssertEqual(app.textViews["draftBody"].value as? String, body)
        XCUIDevice.shared.press(.home)
        _ = try await control("advance")
        app.activate()
        XCTAssertTrue(app.textViews["draftBody"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.textViews["draftBody"].value as? String, body)
        app.buttons["closeDrafts"].tap()
        XCTAssertTrue(
            app.staticTexts["New revision r4 available"].waitForExistence(timeout: 20),
            app.debugDescription)
        capture(app, "Live updates — foreground refetch preserves draft and revision")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        for _ in 0..<8 {
            if open.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(open.waitForExistence(timeout: 10), app.debugDescription)
        open.tap()
        XCTAssertTrue(app.staticTexts["reviewIdentity"].waitForExistence(timeout: 20))
        XCTAssertTrue(app.staticTexts["reviewIdentity"].label.contains("r4"))
        app.terminate()
        XCUIDevice.shared.appearance = .dark
        app.launchArguments = [
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<12 {
            if open.isHittable { break }
            app.swipeUp()
        }
        open.tap()
        XCTAssertTrue(app.staticTexts["reviewIdentity"].waitForExistence(timeout: 20))
        _ = try await control("advance")
        XCTAssertTrue(app.staticTexts["New revision r5 available"].waitForExistence(timeout: 15))
        capture(app, "Live updates — dark mode at largest text size")
        let retainedSource = app.textViews["unifiedDiff"]
        for _ in 0..<6 where !retainedSource.isHittable { app.swipeUp() }
        XCTAssertTrue(retainedSource.isHittable, app.debugDescription)
        for _ in 0..<3 { app.scrollViews["reviewScroll"].swipeUp() }
        XCTAssertTrue(retainedSource.isHittable, app.debugDescription)
        XCTAssertTrue((retainedSource.value as? String ?? "").contains("revision 4"))
        capture(app, "Live updates — accessible retained source")
        try audit(app)
        app.terminate()
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
            .contrast, .dynamicType, .hitRegion, .sufficientElementDescription,
        ])
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
