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
        view.measured = nil
        // SourceScrollView pans it; the outer review scrolls it vertically.
        view.isScrollEnabled = false
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
        // Adjusting fonts for Dynamic Type resizes the text without setting it.
        view.registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) {
            (view: SourceTextView, _) in
            view.measured = nil
            view.setNeedsLayout()
            view.superview?.setNeedsLayout()
            view.superview?.invalidateIntrinsicContentSize()
        }
        return view
    }

    var revealedAnchor: SourceAnchor?
    var revealedRequest: UUID?
    var pendingReveal: NSRange?
    var revealTopInset: CGFloat = 0
    var didReveal: (() -> Void)?
    /// The source's widest line and its height, measured once per installed text: SwiftUI asks
    /// for the reader's size many times per layout pass, and measuring lays out the whole
    /// document. Overriding attributedText to reset it would trap: accessibility reads that
    /// property off the main thread.
    var measured: CGSize?

    /// The width that shows the widest line unclipped.
    var sourceWidth: CGFloat {
        measure().width + textContainerInset.left + textContainerInset.right
    }

    private func measure() -> CGSize {
        if let measured { return measured }
        guard let manager = textLayoutManager else { return .zero }
        // Laid out unclipped, each line reports its full width. NSString's boundingRect would
        // measure the same, but clipped paragraphs take its single-line path, which stalls for
        // seconds on a long file.
        textContainer.size = CGSize(width: 1_000_000, height: CGFloat.greatestFiniteMagnitude)
        manager.ensureLayout(for: manager.documentRange)
        var width: CGFloat = 0
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location) { fragment in
            for line in fragment.textLineFragments {
                width = max(width, fragment.layoutFragmentFrame.minX + line.typographicBounds.maxX)
            }
            return true
        }
        // Lines clip rather than wrap, so the height holds for any container at least this wide.
        let size = CGSize(
            width: ceil(width) + 1, height: ceil(manager.usageBoundsForTextContainer.maxY))
        measured = size
        return size
    }

    /// Fits the container to the viewport or the widest line, whichever is wider; resizing it
    /// invalidates TextKit's layout, so an unchanged width leaves it alone.
    private func fitContainer(to width: CGFloat) {
        let size = CGSize(
            width: max(width - textContainerInset.left - textContainerInset.right, measure().width),
            height: .greatestFiniteMagnitude)
        if textContainer.size != size { textContainer.size = size }
    }

    override func sizeThatFits(_ size: CGSize) -> CGSize {
        CGSize(
            width: size.width,
            height: measure().height + textContainerInset.top + textContainerInset.bottom)
    }

    override func layoutSubviews() {
        fitContainer(to: bounds.width)
        super.layoutSubviews()
        guard let range = pendingReveal,
            let start = position(from: beginningOfDocument, offset: range.location),
            let end = position(from: start, offset: range.length),
            let textRange = textRange(from: start, to: end)
        else { return }
        let rect = firstRect(for: textRange)
        if let pan = superview as? SourceScrollView {
            let horizontal = min(
                max(0, rect.minX - textContainerInset.left),
                max(0, pan.contentSize.width - pan.bounds.width))
            pan.setContentOffset(CGPoint(x: horizontal, y: 0), animated: false)
        }
        guard rect.height > 0 && rect.minY.isFinite else { return }
        var ancestor = superview
        while let view = ancestor {
            if let scroll = view as? UIScrollView, !(scroll is SourceScrollView), scroll.isScrollEnabled {
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
        if gestureRecognizer.name == "gutterTap" {
            return gutterRow(at: gestureRecognizer.location(in: self)) != nil
        }
        return super.gestureRecognizerShouldBegin(gestureRecognizer)
    }

    func install(_ text: NSAttributedString, selection: SourceSelection) {
        sourceSelection = selection
        guard !attributedText.isEqual(to: text) else { return }
        let range = selectedRange
        attributedText = text
        measured = nil
        // The panning view sizes this reader to its widest line, and SwiftUI to its height.
        superview?.setNeedsLayout()
        superview?.invalidateIntrinsicContentSize()
        if NSMaxRange(range) <= text.length { selectedRange = range }
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
