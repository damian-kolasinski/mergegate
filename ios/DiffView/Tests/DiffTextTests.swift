import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct DiffTextTests {
    @Test @MainActor func readerPreservesTextAndDistinguishesBothSidesWithoutColor() {
        let rendered = DiffText.render(
            [
                DiffHunk(
                    header: "@@ -1,1 +1,1 @@",
                    lines: [
                        DiffLine(kind: .removed, oldLine: 1, newLine: nil, text: "old"),
                        DiffLine(kind: .added, oldLine: nil, newLine: 1, text: "é 🐙"),
                    ])
            ], traits: UITraitCollection(preferredContentSizeCategory: .large))
        #expect(rendered.string == "@@ -1,1 +1,1 @@\n1   ·   − old\n·   1   + é 🐙\n\n")
        #expect(rendered.attribute(.backgroundColor, at: 20, effectiveRange: nil) != nil)
    }

    @Test @MainActor func codeFontScalesForAccessibility() throws {
        let hunk = DiffHunk(header: "@@", lines: [])
        let normal = DiffText.render([hunk], traits: UITraitCollection(preferredContentSizeCategory: .large))
        let large = DiffText.render(
            [hunk], traits: UITraitCollection(preferredContentSizeCategory: .accessibilityExtraExtraExtraLarge))
        let normalFont = try #require(normal.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
        let largeFont = try #require(large.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
        #expect(largeFont.pointSize > normalFont.pointSize)
    }
}
