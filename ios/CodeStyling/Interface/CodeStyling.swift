import DiffDocumentInterface
import UIKit

public protocol CodeStyling: Sendable {
    @MainActor func apply(spans: [CodeSpan], source: String, font: UIFont, traits: UITraitCollection)
        -> NSAttributedString
}
