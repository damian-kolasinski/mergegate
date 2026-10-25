import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct SourceSelectionTests {
    @Test @MainActor func copyingSkipsGuttersHeadersNotesAndPreservesUnicodeAndMissingNewline() {
        let document = DiffText.document(
            [
                DiffHunk(
                    header: "@@ -1,1 +1,1 @@",
                    lines: [
                        DiffLine(kind: .removed, oldLine: 1, newLine: nil, text: "old 🐙"),
                        DiffLine(kind: .note, oldLine: nil, newLine: nil, text: "\\ No newline at end of file"),
                        DiffLine(kind: .added, oldLine: nil, newLine: 1, text: "  new e\u{301}"),
                        DiffLine(kind: .note, oldLine: nil, newLine: nil, text: "\\ No newline at end of file"),
                    ])
            ], traits: UITraitCollection())
        #expect(
            document.selection.copy(in: NSRange(location: 0, length: document.text.length)) == "old 🐙  new e\u{301}")
        let range = (document.text.string as NSString).range(of: "new e\u{301}")
        #expect(document.selection.copy(in: range) == "new e\u{301}")
        #expect(document.selection.copy(in: NSRange(location: 0, length: 10)).isEmpty)
        let view = SourceTextView(usingTextLayoutManager: true)
        view.attributedText = document.text
        view.sourceSelection = document.selection
        view.selectedRange = range
        view.copy(nil)
    }

    @Test @MainActor func wrappedSelectionCopiesSourceWithoutInsertedDisplayBreaks() {
        let source = String(repeating: "long_identifier_🐙", count: 30)
        let document = DiffText.document(
            [
                DiffHunk(
                    header: "@@",
                    lines: [
                        DiffLine(kind: .added, oldLine: nil, newLine: 5, text: source),
                        DiffLine(kind: .context, oldLine: 5, newLine: 6, text: "last", hasNewline: false),
                    ])
            ], traits: UITraitCollection())
        #expect(document.selection.copy(in: NSRange(location: 0, length: document.text.length)) == source + "\nlast")
        let view = SourceTextView(usingTextLayoutManager: true)
        view.isScrollEnabled = false
        view.textContainer.lineBreakMode = .byCharWrapping
        view.attributedText = document.text
        #expect(view.sizeThatFits(CGSize(width: 200, height: CGFloat.greatestFiniteMagnitude)).height > 300)
    }
}
