import XCTest

/// Scrolls a long, highlighted diff with wide lines both ways under XCTest's scrolling metric.
/// On a device it reports hitches, so a reader change that does work per frame shows up as
/// numbers rather than a feeling; the simulator reports only durations.
final class ScrollLongDiffTests: XCTestCase {
    @MainActor func testVerticalScrollingStaysSmooth() throws {
        let app = try openLongDiff()
        let scroll = app.scrollViews["reviewScroll"]
        let options = XCTMeasureOptions()
        options.invocationOptions = [.manuallyStop]
        measure(metrics: [XCTOSSignpostMetric.scrollingAndDecelerationMetric], options: options) {
            scroll.swipeUp(velocity: .fast)
            stopMeasuring()
            scroll.swipeDown(velocity: .fast)
        }
    }

    @MainActor func testHorizontalScrollingStaysSmooth() throws {
        let app = try openLongDiff()
        let reader = app.textViews["unifiedDiff"].firstMatch
        let options = XCTMeasureOptions()
        options.invocationOptions = [.manuallyStop]
        measure(metrics: [XCTOSSignpostMetric.scrollingAndDecelerationMetric], options: options) {
            reader.swipeLeft(velocity: .fast)
            stopMeasuring()
            reader.swipeRight(velocity: .fast)
        }
    }

    @MainActor private func openLongDiff() throws -> XCUIApplication {
        let address = ProcessInfo.processInfo.environment["MG_IOS_TEST_HUB_URL"] ?? ""
        guard address.hasPrefix("http"), ProcessInfo.processInfo.environment["MG_IOS_TEST_SCROLL"] == "1"
        else {
            throw XCTSkip("Seed scroll-fixture.py's item and set MG_IOS_TEST_SCROLL=1.")
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
        let reader = app.textViews["unifiedDiff"].firstMatch
        XCTAssertTrue(reader.waitForExistence(timeout: 20))
        // Read past the masthead, among the hunks, where reviewing happens.
        app.scrollViews["reviewScroll"].swipeUp()
        return app
    }
}
