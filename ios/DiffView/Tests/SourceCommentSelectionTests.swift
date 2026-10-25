import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct SourceCommentSelectionTests {
    @Test @MainActor func revealsALineDeepInsideATallHunkInTheAncestorReader() async throws {
        let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 320, height: 200))
        let reader = SourceTextView.reader()
        let document = DiffText.document(
            [
                .init(
                    header: "",
                    lines: (1...200).map {
                        .init(
                            kind: .added, oldLine: nil, newLine: UInt32($0),
                            text: "Source line \($0)")
                    })
            ], traits: UITraitCollection())
        reader.install(document.text, selection: document.selection)
        let height = reader.sizeThatFits(
            CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude)
        ).height
        reader.frame = CGRect(x: 0, y: 0, width: 320, height: height)
        scroll.addSubview(reader)
        scroll.contentSize = reader.bounds.size
        let anchor = SourceAnchor(side: .new, startLine: 180, endLine: 180, quote: "")
        var reveals = 0
        reader.didReveal = { reveals += 1 }
        reader.pendingReveal = try #require(document.comments.range(for: anchor))
        reader.setNeedsLayout()
        reader.layoutIfNeeded()
        #expect(reader.pendingReveal == nil)
        #expect(scroll.contentOffset.y > 200)
        await Task.yield()
        #expect(reveals == 1)
        reader.setNeedsLayout()
        reader.layoutIfNeeded()
        await Task.yield()
        #expect(reveals == 1)
    }

    @Test @MainActor func anchorsUseSourceLinesAndWholeUnicodeRowsRatherThanWrappedRows() throws {
        let long = String(repeating: "e\u{301} 🐙 long source ", count: 30)
        let doc = DiffText.document(
            [
                .init(
                    header: "@@ -4,2 +8,2 @@",
                    lines: [
                        .init(kind: .removed, oldLine: 4, newLine: nil, text: "old 🐙"),
                        .init(kind: .removed, oldLine: 5, newLine: nil, text: "old end"),
                        .init(kind: .added, oldLine: nil, newLine: 8, text: long),
                        .init(kind: .context, oldLine: 6, newLine: 9, text: "end"),
                    ])
            ], traits: UITraitCollection())
        let text = doc.text.string as NSString
        let old = try #require(doc.comments.anchor(in: text.range(of: "old 🐙\n\t5\t−\told end")))
        #expect(old.side == .old && old.startLine == 4 && old.endLine == 5)
        #expect(old.quote == "old 🐙\nold end")
        let revealed = try #require(doc.comments.range(for: old))
        #expect(doc.selection.copy(in: revealed) == "old 🐙\nold end\n")
        #expect(
            doc.comments.range(for: .init(side: .old, startLine: 99, endLine: 99, quote: "")) == nil
        )
        let selected = text.range(of: long)
        let anchor = try #require(
            doc.comments.anchor(in: NSRange(location: selected.location + 12, length: 4)))
        #expect(anchor.side == .new && anchor.startLine == 8 && anchor.endLine == 8)
        #expect(anchor.quote == long)
        #expect(doc.comments.anchor(in: NSRange(location: 0, length: doc.text.length)) == nil)
        #expect(doc.comments.anchor(in: NSRange(location: 0, length: 3)) == nil)
        #expect(doc.comments.anchor(in: NSRange(location: 0, length: 0)) == nil)
    }

    @Test @MainActor func contextUsesNewCoordinatesAndSelectionsCannotBridgeHiddenSource() throws {
        let doc = DiffText.document(
            [
                .init(
                    header: "",
                    lines: [
                        .init(kind: .context, oldLine: 10, newLine: 12, text: "context"),
                        .init(kind: .context, oldLine: 30, newLine: 32, text: "later"),
                    ])
            ], traits: UITraitCollection())
        let anchor = try #require(
            doc.comments.anchor(in: (doc.text.string as NSString).range(of: "context")))
        #expect(anchor.side == .new && anchor.startLine == 12)
        let threads = doc.comments.conversationAnchors(
            in: (doc.text.string as NSString).range(of: "context"))
        #expect(threads.map(\.side) == [.old, .new])
        #expect(threads.map(\.startLine) == [10, 12])
        #expect(
            doc.comments.conversationAnchors(in: NSRange(location: 0, length: doc.text.length))
                .isEmpty)
        #expect(doc.comments.anchor(in: NSRange(location: 0, length: doc.text.length)) == nil)
    }
}
