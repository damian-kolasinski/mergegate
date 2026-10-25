import DiffDocumentInterface
import Foundation
import UIKit

struct SourceSelection {
    let text: String
    let ranges: [NSRange]

    func copy(in selection: NSRange) -> String {
        let source = text as NSString
        return ranges.compactMap { range -> String? in
            let overlap = NSIntersectionRange(range, selection)
            return overlap.length > 0 ? source.substring(with: overlap) : nil
        }.joined()
    }
}

final class SourceTextView: UITextView {
    static func reader() -> SourceTextView {
        let view = SourceTextView(usingTextLayoutManager: true)
        view.isScrollEnabled = false
        view.isEditable = false
        view.isSelectable = true
        view.textContainer.lineBreakMode = .byWordWrapping
        view.backgroundColor = .clear
        view.textContainerInset = UIEdgeInsets(top: 4, left: 8, bottom: 4, right: 12)
        view.textContainer.lineFragmentPadding = 0
        view.adjustsFontForContentSizeCategory = true
        view.accessibilityIdentifier = "unifiedDiff"
        view.accessibilityHint =
            "Select source text to copy, comment, or read conversations on those lines."
        view.delegate = view
        return view
    }

    var sourceSelection = SourceSelection(text: "", ranges: [])
    var commentSelection = SourceCommentSelection(rows: [])
    var onComment: ((SourceAnchor) -> Void)?

    var onReadConversations: (([SourceAnchor]) -> Void)?

    func install(_ text: NSAttributedString, selection: SourceSelection) {
        sourceSelection = selection
        guard !attributedText.isEqual(to: text) else { return }
        let offset = contentOffset
        let range = selectedRange
        attributedText = text
        if NSMaxRange(range) <= text.length { selectedRange = range }
        setContentOffset(offset, animated: false)
    }

    override func copy(_ sender: Any?) {
        let text = sourceSelection.copy(in: selectedRange)
        if !text.isEmpty { UIPasteboard.general.string = text }
    }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(copy(_:)) {
            return !sourceSelection.copy(in: selectedRange).isEmpty
        }
        // Native share/lookup would include display gutters instead of source.
        return action == #selector(select(_:)) || action == #selector(selectAll(_:))
            ? super.canPerformAction(action, withSender: sender) : false
    }
}
