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
        #expect(rendered.string == "@@ -1,1 +1,1 @@\n\t1\t−\told\n\t1\t+\té 🐙\n")
        #expect(rendered.attribute(.backgroundColor, at: 20, effectiveRange: nil) != nil)
    }

    @Test @MainActor func wrappedRowsKeepACompactGutterOutsideTheSourceColumn() throws {
        let rendered = DiffText.render(
            [
                DiffHunk(
                    header: "",
                    lines: [
                        DiffLine(kind: .removed, oldLine: 120, newLine: nil, text: "old"),
                        DiffLine(kind: .added, oldLine: nil, newLine: 8, text: "new"),
                    ])
            ], traits: UITraitCollection(preferredContentSizeCategory: .large))
        #expect(rendered.string == "\t120\t−\told\n\t8\t+\tnew\n")
        let style = try #require(rendered.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)
        #expect(style.firstLineHeadIndent == 0)
        #expect(style.headIndent == style.tabStops.last?.location)
        #expect(style.headIndent > 0 && style.headIndent < 55)
    }

    @Test @MainActor func fileWideGutterKeepsExpandedBlocksAligned() throws {
        let traits = UITraitCollection(preferredContentSizeCategory: .large)
        let first = DiffText.document(
            [
                DiffHunk(
                    header: "",
                    lines: [
                        DiffLine(kind: .context, oldLine: 1, newLine: 1, text: "before")
                    ])
            ], traits: traits, maximumLine: 1000)
        let last = DiffText.document(
            [
                DiffHunk(
                    header: "",
                    lines: [
                        DiffLine(kind: .added, oldLine: nil, newLine: 1000, text: "after")
                    ])
            ], traits: traits, maximumLine: 1000)
        let firstStyle = try #require(
            first.text.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)
        let lastStyle = try #require(
            last.text.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle)
        #expect(firstStyle.headIndent == lastStyle.headIndent)
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
