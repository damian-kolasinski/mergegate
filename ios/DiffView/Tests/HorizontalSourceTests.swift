import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct HorizontalSourceTests {
    @Test @MainActor func longLinesScrollWithoutWrappingOrChangingSourceSelection() throws {
        for category in [UIContentSizeCategory.large, .accessibilityExtraExtraExtraLarge] {
            let source = "\tlet message = \"é 🐙\" + " + String(repeating: "long_source_name + ", count: 20)
            let document = DiffText.document([
                DiffHunk(header: "@@ -1,1 +1,1 @@", lines: [
                    DiffLine(kind: .added, oldLine: nil, newLine: 1, text: source),
                    DiffLine(kind: .context, oldLine: 2, newLine: 2, text: "end"),
                ])
            ], traits: UITraitCollection(preferredContentSizeCategory: category))
            let pan = SourceScrollView()
            let view = pan.reader
            view.traitOverrides.preferredContentSizeCategory = category
            view.install(document.text, selection: document.selection)
            view.commentSelection = document.comments
            let size = pan.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
            pan.frame = CGRect(origin: .zero, size: size)
            pan.layoutIfNeeded()
            let range = (document.text.string as NSString).range(of: source)
            let start = try #require(view.position(from: view.beginningOfDocument, offset: range.location))
            let end = try #require(view.position(from: start, offset: range.length - 1))
            #expect(abs(view.caretRect(for: start).minY - view.caretRect(for: end).minY) < 1)
            // The whole line is laid out, so panning to its end shows it without clipping.
            #expect(view.caretRect(for: end).maxX <= view.bounds.width)
            #expect(pan.contentSize.width > pan.bounds.width)
            #expect(pan.contentSize.height <= pan.bounds.height + 1)
            pan.setContentOffset(CGPoint(x: 180, y: 0), animated: false)
            view.selectedRange = range
            let highlighted = NSMutableAttributedString(attributedString: document.text)
            highlighted.addAttribute(.foregroundColor, value: UIColor.systemBlue, range: range)
            view.install(highlighted, selection: document.selection)
            pan.layoutIfNeeded()
            #expect(pan.contentOffset.x == 180)
            #expect(view.sourceSelection.copy(in: view.selectedRange) == source)
            #expect(view.commentSelection.anchor(in: view.selectedRange)?.startLine == 1)
        }
    }

    @Test @MainActor func panningSidewaysLeavesTheReaderLaidOut() {
        let document = DiffText.document([
            DiffHunk(header: "", lines: (1...300).map {
                DiffLine(
                    kind: .added, oldLine: nil, newLine: UInt32($0),
                    text: "let value\($0) = " + String(repeating: "source + ", count: 30))
            })
        ], traits: UITraitCollection())
        let pan = SourceScrollView()
        pan.reader.install(document.text, selection: document.selection)
        pan.frame = CGRect(
            origin: .zero,
            size: pan.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude)))
        pan.layoutIfNeeded()
        let reader = pan.reader.frame
        let container = pan.reader.textContainer.size
        for step in 1...20 {
            pan.contentOffset.x = CGFloat(step * 20)
            pan.layoutIfNeeded()
        }
        // A pan moves the laid-out reader; it doesn't resize it or lay it out again.
        #expect(pan.reader.frame == reader)
        #expect(pan.reader.textContainer.size == container)
        #expect(pan.reader.contentOffset == .zero)
    }
}
