import UIKit
import XCTest

@MainActor
func auditInboxAccessibility(_ app: XCUIApplication) throws {
    for pass in 0..<2 {
        var retryUnattributed = false
        try app.performAccessibilityAudit(for: [.contrast, .dynamicType, .hitRegion, .sufficientElementDescription]) {
            issue in
            // An unmatched node can come from an unsettled SwiftUI accessibility snapshot.
            // Preserve it and repeat the full audit once, without changing the UI. A second
            // unmatched report fails, as does every other unresolved issue on either pass.
            if pass == 0, issue.auditType == .contrast, issue.element == nil {
                retryUnattributed = true
                XCTContext.runActivity(named: "Repeat audit for unattributed contrast report") { activity in
                    let report = XCTAttachment(string: issue.detailedDescription)
                    report.lifetime = .keepAlways
                    activity.add(report)
                    let screenshot = XCTAttachment(screenshot: app.screenshot())
                    screenshot.lifetime = .keepAlways
                    activity.add(screenshot)
                }
                return true
            }
            let identifiers: Set<String> = ["inboxSummary", "inboxRemaining", "inboxAdded", "inboxRemoved"]
            guard issue.auditType == .contrast, let element = issue.element,
                element.elementType == .staticText, identifiers.contains(element.identifier)
            else { return false }
            // XCTest can misreport correct text contrast (WWDC23 session 10035). Verify the actual
            // label crop, including its rendered background, rather than exempting a token or label.
            let screenshot = element.screenshot()
            let ratio = InboxRenderedContrast.measure(screenshot.image)
            XCTContext.runActivity(named: "Inbox label contrast: \(ratio.map { String(format: "%.2f:1", $0) } ?? "unmeasurable")") { activity in
                let attachment = XCTAttachment(screenshot: screenshot)
                attachment.lifetime = .keepAlways
                activity.add(attachment)
            }
            return ratio.map { $0 >= 4.5 } ?? false
        }
        if !retryUnattributed { return }
    }
}

enum InboxRenderedContrast {
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
            let rgb = UInt32(pixels[index]) << 16 | UInt32(pixels[index + 1]) << 8 | UInt32(pixels[index + 2])
            histogram[rgb, default: 0] += 1
            let pixel = index / 4
            if pixel < width || pixel >= width * (height - 1) || pixel % width == 0 || pixel % width == width - 1 {
                border[rgb, default: 0] += 1
            }
        }
        guard let background = border.max(by: { $0.value < $1.value }) else { return nil }
        // Liquid Glass can vary slightly behind one label. Group nearby background shades,
        // then use the least favorable shade so a gradient cannot inflate the result.
        let backgroundShades = histogram.filter { nearby($0.key, background.key) }
        // Require substantial solid-color populations. A lone dark pixel, antialias edge, or
        // faint shadow must not turn a real low-contrast label into a passing result.
        let minimum = max(20, width * height / 20)
        guard backgroundShades.values.reduce(0, +) >= width * height / 2,
            let foreground = histogram.filter({
                $0.value >= minimum && !nearby($0.key, background.key)
            }).max(by: { $0.value < $1.value })
        else { return nil }
        return backgroundShades.keys.map { contrast(foreground.key, $0) }.min()
    }

    private static func nearby(_ first: UInt32, _ second: UInt32) -> Bool {
        [0, 8, 16].allSatisfy { shift in
            abs(Int((first >> shift) & 255) - Int((second >> shift) & 255)) <= 16
        }
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
