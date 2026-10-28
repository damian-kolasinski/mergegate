import XCTest

final class ReadHighlightedCodeTests: XCTestCase {
    @MainActor func testHighlightsArriveInAnOpenRealReview() async throws {
        let environment = ProcessInfo.processInfo.environment
        let address = environment["MG_IOS_TEST_HUB_URL"] ?? ""
        let pack = environment["MG_IOS_TEST_HIGHLIGHT"] ?? ""
        guard address.hasPrefix("http"), pack.hasSuffix("swift.tar.gz"),
            pack.hasPrefix("/private/tmp/") || pack.hasPrefix("/tmp/")
        else { throw XCTSkip("Run highlight-fixture.py on a disposable hub and pass its pack path.") }
        let hub = try XCTUnwrap(URL(string: address))
        // Mutations below target only this test's disposable hub, never the operator's project.
        let (itemData, _) = try await URLSession.shared.data(
            from: hub.appending(path: "api/items").appending(component: "mobile-smoke#2"))
        let item = try XCTUnwrap(JSONSerialization.jsonObject(with: itemData) as? [String: Any])
        let identity = try XCTUnwrap(item["item"] as? [String: Any])
        XCTAssertEqual(identity["title"] as? String, "Read checkout source context")
        guard identity["title"] as? String == "Read checkout source context" else { return }
        var remove = URLRequest(url: hub.appending(path: "api/languages/swift"))
        remove.httpMethod = "DELETE"
        remove.setValue("mergegate-ios", forHTTPHeaderField: "X-Mergegate-Client")
        _ = try await URLSession.shared.data(for: remove)
        let app = XCUIApplication()
        app.launchArguments = ["-AppleInterfaceStyle", "Light"]
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
        XCTAssertTrue(app.buttons["loadSourceContext"].waitForExistence(timeout: 20))
        capture(app, "01 — Plain source without an installed grammar")
        app.buttons["loadSourceContext"].tap()
        let expand = app.buttons["expandContext-0"]
        XCTAssertTrue(expand.waitForExistence(timeout: 15))
        expand.tap()
        let source = app.textViews["unifiedDiff"].firstMatch
        let original = try XCTUnwrap(source.value as? String)
        XCTAssertTrue(original.contains("é 🐙"))
        let plain = source.screenshot().pngRepresentation
        let originalFrame = source.frame
        capture(app, "02 — Plain expanded source stays readable")
        var install = URLRequest(url: hub.appending(path: "api/languages"))
        install.httpMethod = "POST"
        install.setValue("mergegate-ios", forHTTPHeaderField: "X-Mergegate-Client")
        install.setValue("application/json", forHTTPHeaderField: "Content-Type")
        install.httpBody = try JSONSerialization.data(withJSONObject: ["from": pack])
        let (installation, response) = try await URLSession.shared.data(for: install)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200, String(decoding: installation, as: UTF8.self))
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { return }
        var changed = false
        for _ in 0..<30 {
            try await Task.sleep(for: .seconds(1))
            if source.screenshot().pngRepresentation != plain {
                changed = true
                break
            }
        }
        XCTAssertTrue(changed, "Real grammar installation must repaint the open source through SSE.")
        XCTAssertEqual(source.value as? String, original)
        XCTAssertEqual(source.frame, originalFrame, "Syntax colors must preserve line layout and reading position.")
        XCTAssertFalse(app.buttons["loadSourceContext"].exists, "Expanded source must stay expanded.")
        XCTAssertEqual(app.staticTexts["reviewIdentity"].label, "mobile-smoke#2 · r1 → main")
        capture(app, "03 — Late hub highlighting with context retained")
        try auditReviewAccessibility(app)
        // Return to the compact changed-code view for the in-thread demo.
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<8 where !open.isHittable { app.swipeUp() }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["loadSourceContext"].waitForExistence(timeout: 15))
        capture(app, "04 — Highlighted File reader")
        app.terminate()
        app.launchArguments = [
            "-AppleInterfaceStyle", "Dark", "-UIPreferredContentSizeCategoryName",
            "UICTContentSizeCategoryL",
        ]
        app.launch()
        XCTAssertTrue(app.buttons["hubSettings"].waitForExistence(timeout: 20))
        for _ in 0..<8 where !open.isHittable { app.swipeUp() }
        XCTAssertTrue(open.isHittable, app.debugDescription)
        open.tap()
        XCTAssertTrue(app.buttons["loadSourceContext"].waitForExistence(timeout: 15))
        capture(app, "05 — Highlighting in dark mode at accessibility size")
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
