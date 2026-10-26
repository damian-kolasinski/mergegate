import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct SourceLineSelectionTests {
    @Test @MainActor func gutterTapsSelectExtendAndClearLinesOnOneSide() throws {
        let view = SourceTextView.reader()
        let document = DiffText.document(
            [
                DiffHunk(
                    header: "@@ -1,2 +1,3 @@",
                    lines: [
                        .init(kind: .context, oldLine: 1, newLine: 1, text: "let a = 1", hasNewline: true),
                        .init(kind: .removed, oldLine: 2, newLine: nil, text: "let b = 2", hasNewline: true),
                        .init(kind: .added, oldLine: nil, newLine: 2, text: "let b = 3", hasNewline: true),
                        .init(kind: .added, oldLine: nil, newLine: 3, text: "let c = 4", hasNewline: true),
                    ])
            ], traits: view.traitCollection)
        view.install(document.text, selection: document.selection)
        view.commentSelection = document.comments
        var reported: [SourceAnchor?] = []
        view.onSelectLines = { reported.append($0) }
        let rows = document.comments.rows

        view.selectGutterRow(rows[2])
        #expect(reported.last??.side == .new)
        #expect(reported.last??.startLine == 2 && reported.last??.endLine == 2)
        // Another line on the same side extends the range.
        view.selectGutterRow(rows[3])
        #expect(reported.last??.startLine == 2 && reported.last??.endLine == 3)
        // A removal can't join new-side lines, so the selection starts over there.
        view.selectGutterRow(rows[1])
        #expect(reported.last??.side == .old)
        #expect(reported.last??.startLine == 2 && reported.last??.endLine == 2)
        // The same single line again clears it.
        view.selectGutterRow(rows[1])
        #expect(reported.last == .some(nil))

        // Only the gutter selects: a point over the source text belongs to text selection.
        view.frame = CGRect(x: 0, y: 0, width: 390, height: 400)
        view.layoutIfNeeded()
        let start = try #require(view.position(from: view.beginningOfDocument, offset: rows[2].range.location))
        let line = view.caretRect(for: start)
        #expect(view.gutterRow(at: CGPoint(x: line.minX + 4, y: line.midY))?.line.newLine == 2)
        #expect(view.gutterRow(at: CGPoint(x: view.bounds.width - 20, y: line.midY)) == nil)
    }
}
