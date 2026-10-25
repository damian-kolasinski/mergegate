import CodeStylingInterface
import DiffDocumentInterface
import UIKit

public struct NativeCodeStyling: CodeStyling {
    public init() {}

    @MainActor public func apply(
        spans: [CodeSpan], source: String, font: UIFont, traits: UITraitCollection
    ) -> NSAttributedString {
        let result = NSMutableAttributedString(string: source)
        let units = Array(source.utf16)
        for span in spans {
            // Reject malformed ranges, including half of a surrogate pair, without losing readable source.
            guard span.start >= 0, span.length > 0, span.start <= result.length,
                span.length <= result.length - span.start,
                !isLowSurrogate(units[span.start]),
                span.start + span.length == units.count || !isLowSurrogate(units[span.start + span.length])
            else { continue }
            var attributes: [NSAttributedString.Key: Any] = [:]
            if let color = SyntaxPalette.color(span.tag, traits: traits) {
                attributes[.foregroundColor] = color
            }
            if span.tag == "emphasis",
                let descriptor = font.fontDescriptor.withSymbolicTraits(.traitItalic)
            {
                attributes[.font] = UIFont(descriptor: descriptor, size: font.pointSize)
            }
            if span.tag == "strong" || span.tag == "heading" {
                attributes[.font] = UIFont.monospacedSystemFont(ofSize: font.pointSize, weight: .semibold)
            }
            if span.tag == "link" { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            result.addAttributes(attributes, range: NSRange(location: span.start, length: span.length))
        }
        return result
    }

    private func isLowSurrogate(_ unit: UInt16) -> Bool { (0xDC00...0xDFFF).contains(unit) }
}
