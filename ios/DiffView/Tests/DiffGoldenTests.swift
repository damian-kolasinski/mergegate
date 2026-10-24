import DiffDocumentInterface
import SwiftUI
import UIKit
import XCTest

@testable import DiffView

final class DiffGoldenTests: XCTestCase {
    @MainActor func testUnifiedDiffGoldens() throws {
        let cases: [(String, UIUserInterfaceStyle, UIContentSizeCategory, CGFloat)] = [
            ("light", .light, .large, 280),
            ("dark", .dark, .large, 280),
            ("accessibility", .light, .accessibilityExtraExtraExtraLarge, 680),
        ]
        for (name, style, category, height) in cases {
            let traits = UITraitCollection(traitsFrom: [
                UITraitCollection(userInterfaceStyle: style),
                UITraitCollection(preferredContentSizeCategory: category),
            ])
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            format.opaque = true
            format.preferredRange = .standard
            let size = CGSize(width: 390, height: height)
            let image = UIGraphicsImageRenderer(size: size, format: format).image { context in
                traits.performAsCurrent {
                    UIColor(Appearance.page).setFill()
                    context.fill(CGRect(origin: .zero, size: size))
                    let view = SourceTextView.reader()
                    view.overrideUserInterfaceStyle = style
                    view.traitOverrides.preferredContentSizeCategory = category
                    view.frame = CGRect(origin: .zero, size: size)
                    view.attributedText = DiffText.render(
                        [
                            DiffHunk(
                                header: "@@ -8,2 +8,2 @@",
                                lines: [
                                    DiffLine(kind: .context, oldLine: 8, newLine: 8, text: "var total: Int {"),
                                    DiffLine(kind: .removed, oldLine: 9, newLine: nil, text: "    subtotal"),
                                    DiffLine(
                                        kind: .added, oldLine: nil, newLine: 9,
                                        text: "    subtotal + deliveryForRegion(customer.region)"),
                                    DiffLine(
                                        kind: .note, oldLine: nil, newLine: nil, text: "\\ No newline at end of file"),
                                ])
                        ], traits: traits
                    )
                    view.layoutIfNeeded()
                    view.layer.render(in: context.cgContext)
                }
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "diff-golden-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            let url = try XCTUnwrap(Bundle(for: Self.self).resourceURL).appending(path: "Goldens/\(name).png")
            guard let expected = UIImage(contentsOfFile: url.path) else {
                XCTFail("Missing inspected golden: \(name).png")
                continue
            }
            XCTAssertEqual(
                try XCTUnwrap(pixels(image)), try XCTUnwrap(pixels(expected)), "Diff rendering changed: \(name)")
        }
    }

    @MainActor private func pixels(_ image: UIImage) -> Data? {
        guard let image = image.cgImage else { return nil }
        var bytes = Data(count: image.width * image.height * 4)
        let drawn = bytes.withUnsafeMutableBytes { storage -> Bool in
            guard
                let context = CGContext(
                    data: storage.baseAddress, width: image.width, height: image.height,
                    bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            return true
        }
        return drawn ? bytes : nil
    }
}
