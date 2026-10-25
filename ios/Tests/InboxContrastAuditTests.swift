import UIKit
import XCTest

final class InboxContrastAuditTests: XCTestCase {
    @MainActor func testRenderedContrastRejectsLowContrastAndIsolatedDarkPixels() throws {
        XCTAssertGreaterThan(try XCTUnwrap(InboxRenderedContrast.measure(label(ink: .black))), 4.5)
        XCTAssertLessThan(try XCTUnwrap(InboxRenderedContrast.measure(label(ink: .lightGray))), 4.5)
        XCTAssertNil(InboxRenderedContrast.measure(label(ink: .black, inkWidth: 1, inkHeight: 1)))
        XCTAssertNil(InboxRenderedContrast.measure(label(ink: .white)))
        XCTAssertGreaterThan(try XCTUnwrap(InboxRenderedContrast.measure(label(ink: .black, gradient: true))), 4.5)
        XCTAssertLessThan(try XCTUnwrap(InboxRenderedContrast.measure(label(ink: .lightGray, gradient: true))), 4.5)
    }

    @MainActor private func label(
        ink: UIColor, inkWidth: CGFloat = 25, inkHeight: CGFloat = 20, gradient: Bool = false
    ) -> UIImage {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        return UIGraphicsImageRenderer(size: CGSize(width: 80, height: 30), format: format).image { context in
            UIColor.white.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 80, height: 30))
            if gradient {
                for row in 0..<30 {
                    UIColor(white: CGFloat(230 + row / 3) / 255, alpha: 1).setFill()
                    context.fill(CGRect(x: 0, y: row, width: 80, height: 1))
                }
            }
            ink.setFill()
            context.fill(CGRect(x: 10, y: 5, width: inkWidth, height: inkHeight))
        }
    }
}
