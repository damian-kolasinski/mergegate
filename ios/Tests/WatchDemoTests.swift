import XCTest

final class WatchDemoTests: XCTestCase {
    @MainActor func testFullScreenDemoZoomAndDisappearingVideoControls() throws {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_DEMO"] == "1" else {
            throw XCTSkip("Attach portrait.png and demo.mp4 to review-fixture.py's item and set MG_IOS_TEST_DEMO=1.")
        }
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
        let item = app.buttons["openReview-mobile-smoke#2"]
        XCTAssertTrue(item.waitForExistence(timeout: 20))
        item.tap()
        let demo = app.buttons["reviewDemo"]
        XCTAssertTrue(demo.waitForExistence(timeout: 20))
        demo.tap()
        let image = app.images["demoImage"]
        XCTAssertTrue(image.waitForExistence(timeout: 15))
        XCTAssertEqual(image.value as? String, "100 percent")
        capture(app, "Demo — natural-size portrait in full screen")
        image.pinch(withScale: 2, velocity: 1)
        XCTAssertNotEqual(image.value as? String, "100 percent")
        capture(app, "Demo — pinch to zoom")
        if !app.buttons["closeDemo"].exists {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        }
        XCTAssertTrue(app.buttons["nextDemo"].waitForExistence(timeout: 5))
        app.buttons["nextDemo"].tap()
        let play = app.buttons["demoPlayback"]
        if !play.waitForExistence(timeout: 3) {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        }
        XCTAssertTrue(play.waitForExistence(timeout: 10), app.debugDescription)
        capture(app, "Demo — full-screen video with controls")
        play.tap()
        let done = app.buttons["closeDemo"]
        let hidden = NSPredicate { _, _ in !done.exists || !done.isHittable }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: hidden, object: done)], timeout: 8), .completed)
        XCTAssertFalse(app.buttons["demoPlayback"].exists)
        capture(app, "Demo — unobstructed video playback")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(done.waitForExistence(timeout: 5))
        XCTAssertTrue(done.isHittable)
        let position = app.sliders["Video position"]
        XCTAssertTrue(position.waitForExistence(timeout: 5))
        position.adjust(toNormalizedSliderPosition: 1)
        let paused = NSPredicate { _, _ in play.exists && play.label == "Play" }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: paused, object: play)], timeout: 5), .completed)
        play.tap()
        let replaying = NSPredicate { _, _ in play.exists && play.label == "Pause" }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: replaying, object: play)], timeout: 5), .completed)
        capture(app, "Demo — replay after reaching the end")
        done.tap()
        XCTAssertTrue(app.buttons["reviewFiles"].waitForExistence(timeout: 5))
    }

    @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
