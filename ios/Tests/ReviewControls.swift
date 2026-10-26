import UIKit
import XCTest

/// Chooses an item of the reader's ⋯ menu by its title, as "Review guide" or "Item details".
@MainActor
func openReviewMenu(_ app: XCUIApplication, _ title: String) {
    let more = app.buttons["reviewMore"]
    XCTAssertTrue(more.waitForExistence(timeout: 20), app.debugDescription)
    more.tap()
    let item = app.buttons[title]
    XCTAssertTrue(item.waitForExistence(timeout: 5), app.debugDescription)
    item.tap()
}

/// Audits the reader. XCTest can misreport text contrast on translucent and tinted surfaces
/// (WWDC23 session 10035), so a contrast report passes only when the element's rendered crop
/// measures at least 4.5:1; every other issue fails.
@MainActor
func auditReviewAccessibility(_ app: XCUIApplication) throws {
    try app.performAccessibilityAudit(for: [.contrast, .hitRegion, .sufficientElementDescription]) {
        issue in
        guard issue.auditType == .contrast, let element = issue.element, element.exists,
            !element.frame.isEmpty
        else {
            print("Accessibility: \(issue.compactDescription) · \(issue.element?.debugDescription ?? "no element")")
            return false
        }
        // XCTest includes labels scrolled under the bars; with no visible area there is no
        // rendered contrast to measure. Visible labels still need it.
        guard element.isHittable, app.frame.contains(element.frame) else { return true }
        let crop = element.screenshot()
        let ratio = RenderedTextContrast.measure(crop.image)
        XCTContext.runActivity(
            named: "Review label contrast: \(ratio.map { String(format: "%.2f:1", $0) } ?? "unmeasurable")"
        ) { activity in
            let attachment = XCTAttachment(screenshot: crop)
            attachment.lifetime = .keepAlways
            activity.add(attachment)
        }
        return ratio.map { $0 >= 4.5 } ?? false
    }
}

enum RenderedTextContrast {
    /// The contrast of a label's crop against its most common edge color. Antialiasing only
    /// blends text toward its background, so the strongest color with a real population can't
    /// exceed the text's own contrast, while thin glyphs still qualify.
    static func measure(_ image: UIImage) -> Double? {
        guard let image = image.cgImage else { return nil }
        let width = image.width
        let height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        // SAFETY: The RGBA buffer covers every row; the context stays inside this scoped pointer.
        let drawn = pixels.withUnsafeMutableBytes { bytes in
            guard
                let context = CGContext(
                    data: bytes.baseAddress, width: width, height: height, bitsPerComponent: 8,
                    bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return nil }
        var histogram: [UInt32: Int] = [:]
        var border: [UInt32: Int] = [:]
        for index in stride(from: 0, to: pixels.count, by: 4) where pixels[index + 3] == 255 {
            let rgb =
                UInt32(pixels[index]) << 16 | UInt32(pixels[index + 1]) << 8 | UInt32(pixels[index + 2])
            histogram[rgb, default: 0] += 1
            let pixel = index / 4
            if pixel < width || pixel >= width * (height - 1) || pixel % width == 0
                || pixel % width == width - 1
            {
                border[rgb, default: 0] += 1
            }
        }
        guard let background = border.max(by: { $0.value < $1.value })?.key else { return nil }
        // Ink is what stands apart from the background; a soft shadow or gradient behind the
        // label doesn't. The text's own color must make up a real share of the ink.
        let ink = histogram.filter { contrast($0.key, background) >= 1.5 }
        let minimum = max(6, ink.values.reduce(0, +) / 20)
        return ink.filter { $0.value >= minimum }.map { contrast($0.key, background) }.max()
    }

    private static func contrast(_ first: UInt32, _ second: UInt32) -> Double {
        let a = luminance(first)
        let b = luminance(second)
        return (max(a, b) + 0.05) / (min(a, b) + 0.05)
    }

    private static func luminance(_ rgb: UInt32) -> Double {
        func linear(_ channel: UInt32) -> Double {
            let value = Double(channel & 255) / 255
            return value <= 0.04045 ? value / 12.92 : pow((value + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * linear(rgb >> 16) + 0.7152 * linear(rgb >> 8) + 0.0722 * linear(rgb)
    }
}

/// Opens the completion sheet on an outcome from the review bar's outcome menu: "Send comments",
/// "Request changes" or "Approve & merge".
@MainActor
func openCompletion(_ app: XCUIApplication, _ outcome: String = "Send comments") {
    let outcomes = app.buttons["reviewOutcomes"]
    XCTAssertTrue(outcomes.waitForExistence(timeout: 20), app.debugDescription)
    outcomes.tap()
    let item = app.buttons[outcome]
    XCTAssertTrue(item.waitForExistence(timeout: 5), app.debugDescription)
    item.tap()
}
