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
            let view = SourceTextView.reader()
            view.traitOverrides.preferredContentSizeCategory = category
            view.install(document.text, selection: document.selection)
            view.commentSelection = document.comments
            let size = view.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
            view.frame = CGRect(origin: .zero, size: size)
            view.layoutIfNeeded()
            let range = (document.text.string as NSString).range(of: source)
            let start = try #require(view.position(from: view.beginningOfDocument, offset: range.location))
            let end = try #require(view.position(from: start, offset: range.length - 1))
            #expect(abs(view.caretRect(for: start).minY - view.caretRect(for: end).minY) < 1)
            #expect(view.contentSize.width > view.bounds.width)
            #expect(view.contentSize.height <= view.bounds.height + 1)
            view.setContentOffset(CGPoint(x: 180, y: 0), animated: false)
            view.selectedRange = range
            let highlighted = NSMutableAttributedString(attributedString: document.text)
            highlighted.addAttribute(.foregroundColor, value: UIColor.systemBlue, range: range)
            view.install(highlighted, selection: document.selection)
            view.layoutIfNeeded()
            #expect(view.contentOffset.x == 180)
            #expect(view.sourceSelection.copy(in: view.selectedRange) == source)
            #expect(view.commentSelection.anchor(in: view.selectedRange)?.startLine == 1)
        }
    }
}
