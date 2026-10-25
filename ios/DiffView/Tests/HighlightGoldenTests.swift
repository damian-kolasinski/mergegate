import CodeStyling
import DiffDocumentInterface
import UIKit
import XCTest

@testable import DiffView

final class HighlightGoldenTests: XCTestCase {
    @MainActor func testHighlightedSourceGoldens() throws {
        for (name, style, category, height) in [
            ("highlight-light", UIUserInterfaceStyle.light, UIContentSizeCategory.large, CGFloat(280)),
            ("highlight-dark", .dark, .large, 280),
            ("highlight-accessibility", .light, .accessibilityExtraExtraExtraLarge, 680),
        ] {
            let traits = UITraitCollection(traitsFrom: [
                UITraitCollection(userInterfaceStyle: style), UITraitCollection(preferredContentSizeCategory: category),
            ])
            let line = "let message = \"é 🐙\""
            let spans = [CodeSpan(start: 0, length: 3, tag: "keyword"), CodeSpan(start: 14, length: 6, tag: "string")]
            let hunks = [
                DiffHunk(
                    header: "@@ -8,2 +8,2 @@",
                    lines: [
                        DiffLine(
                            kind: .context, oldLine: 8, newLine: 8, text: "// Checkout policy",
                            spans: [CodeSpan(start: 0, length: 18, tag: "comment")]),
                        DiffLine(kind: .removed, oldLine: 9, newLine: nil, text: line, spans: spans),
                        DiffLine(
                            kind: .added, oldLine: nil, newLine: 9,
                            text: "let message = deliveryForRegion(customer.region)",
                            spans: [
                                CodeSpan(start: 0, length: 3, tag: "keyword"),
                                CodeSpan(start: 14, length: 17, tag: "function"),
                                CodeSpan(start: 40, length: 6, tag: "property"),
                            ]),
                    ])
            ]
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
                    view.attributedText = DiffText.document(hunks, traits: traits, styling: NativeCodeStyling()).text
                    view.layoutIfNeeded()
                    view.layer.render(in: context.cgContext)
                }
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = name
            attachment.lifetime = .keepAlways
            add(attachment)
            let url = try XCTUnwrap(Bundle(for: Self.self).resourceURL).appending(path: "Goldens/\(name).png")
            let expected = try XCTUnwrap(UIImage(contentsOfFile: url.path))
            XCTAssertEqual(try pixels(image), try pixels(expected), "Highlighted rendering changed: \(name)")
            let document = DiffText.document(hunks, traits: traits, styling: NativeCodeStyling())
            XCTAssertEqual(
                document.selection.copy(in: NSRange(location: 0, length: document.text.length)),
                "// Checkout policy\n" + line + "\nlet message = deliveryForRegion(customer.region)\n")
        }
    }

    @MainActor private func pixels(_ image: UIImage) throws -> Data {
        let image = try XCTUnwrap(image.cgImage)
        var bytes = Data(count: image.width * image.height * 4)
        try bytes.withUnsafeMutableBytes { storage in
            let context = try XCTUnwrap(
                CGContext(
                    data: storage.baseAddress, width: image.width, height: image.height,
                    bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        return bytes
    }
}
