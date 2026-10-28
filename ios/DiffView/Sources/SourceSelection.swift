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
        // UIKit's TextKit 2 factory initializer doesn't run Swift's property initializers, so the
        // stored properties start zero-filled; set the ones read before UnifiedDiffView sets them.
        view.sourceSelection = SourceSelection(text: "", ranges: [])
        view.commentSelection = SourceCommentSelection(rows: [])
        view.sourceSides = [.old, .new]
        // Zero-filled optionals of these types read as values, not nil.
        view.pendingReveal = nil
        view.revealedRequest = nil
        view.revealedAnchor = nil
        view.isScrollEnabled = true
        view.alwaysBounceVertical = false
        view.showsVerticalScrollIndicator = false
        view.isDirectionalLockEnabled = true
        view.isEditable = false
        view.isSelectable = true
        view.textContainer.lineBreakMode = .byClipping
        view.textContainer.widthTracksTextView = false
        view.backgroundColor = .clear
        view.textContainerInset = UIEdgeInsets(top: 4, left: 8, bottom: 4, right: 12)
        view.textContainer.lineFragmentPadding = 0
        view.adjustsFontForContentSizeCategory = true
        view.accessibilityIdentifier = "unifiedDiff"
        view.accessibilityHint =
            "Swipe horizontally to read long lines. Tap a line number to select that line, then another to extend. Select source text to copy, comment, or read conversations on those lines."
        view.delegate = view
        view.installGutterTap()
        return view
    }

    var revealedAnchor: SourceAnchor?
    var revealedRequest: UUID?
    var pendingReveal: NSRange?
    var revealTopInset: CGFloat = 0
    var didReveal: (() -> Void)?

    override func sizeThatFits(_ size: CGSize) -> CGSize {
        let insets = textContainerInset
        let bounds = attributedText.boundingRect(
            with: CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil)
        textContainer.size = CGSize(
            width: max(size.width - insets.left - insets.right, ceil(bounds.width) + 1),
            height: .greatestFiniteMagnitude)
        if let manager = textLayoutManager {
            manager.ensureLayout(for: manager.documentRange)
        }
        let height = textLayoutManager?.usageBoundsForTextContainer.maxY ?? bounds.height
        // The outer review owns vertical scrolling; this reader only pans across source lines.
        return CGSize(width: size.width, height: ceil(height) + insets.top + insets.bottom)
    }

    override func layoutSubviews() {
        _ = sizeThatFits(bounds.size)
        super.layoutSubviews()
        let horizontalSize = CGSize(
            width: textContainer.size.width + textContainerInset.left + textContainerInset.right,
            height: bounds.height)
        if contentSize != horizontalSize { contentSize = horizontalSize }
        guard let range = pendingReveal,
            let start = position(from: beginningOfDocument, offset: range.location),
            let end = position(from: start, offset: range.length),
            let textRange = textRange(from: start, to: end)
        else { return }
        let rect = firstRect(for: textRange)
        let horizontal = min(max(0, rect.minX - textContainerInset.left), max(0, contentSize.width - bounds.width))
        setContentOffset(CGPoint(x: horizontal, y: 0), animated: false)
        guard rect.height > 0 && rect.minY.isFinite else { return }
        var ancestor = superview
        while let view = ancestor {
            if let scroll = view as? UIScrollView, scroll.isScrollEnabled {
                pendingReveal = nil
                var target = convert(rect, to: scroll).insetBy(dx: 0, dy: -20)
                // Pinned reading controls cover content without changing UIKit's scroll insets.
                target.origin.y -= revealTopInset
                target.size.height += revealTopInset
                scroll.scrollRectToVisible(target, animated: false)
                if let didReveal { Task { @MainActor in didReveal() } }
                return
            }
            ancestor = view.superview
        }
    }

    var sourceSelection = SourceSelection(text: "", ranges: [])
    var commentSelection = SourceCommentSelection(rows: [])
    var sourceSides: [SourceAnchor.Side] = [.old, .new]
    var onComment: ((SourceAnchor) -> Void)?

    var onReadConversations: (([SourceAnchor]) -> Void)?
    var onSelectLines: ((SourceAnchor?) -> Void)?

    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        if gestureRecognizer === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            if abs(velocity.y) > abs(velocity.x) { return false }
        }
        if gestureRecognizer.name == "gutterTap" {
            return gutterRow(at: gestureRecognizer.location(in: self)) != nil
        }
        return super.gestureRecognizerShouldBegin(gestureRecognizer)
    }

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
