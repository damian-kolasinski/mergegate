import DiffDocumentInterface
import UIKit

// Tapping a line number selects that source line and tapping another extends the selection to
// it: the touch version of the web's gutter "+" and shift-click. Whatever the selection, the
// reader hears the source range it names, so it can offer to comment on it.
extension SourceTextView {
    func installGutterTap() {
        let tap = UITapGestureRecognizer(target: self, action: #selector(tapGutter(_:)))
        tap.name = "gutterTap"
        addGestureRecognizer(tap)
        // The text view's own taps would clear the selection this one makes.
        for recognizer in gestureRecognizers ?? [] where recognizer !== tap
            && recognizer is UITapGestureRecognizer
        {
            recognizer.require(toFail: tap)
        }
    }

    /// The comment row under `point` when the point is left of that row's source text.
    func gutterRow(at point: CGPoint) -> SourceCommentSelection.Row? {
        guard let position = closestPosition(to: point) else { return nil }
        let index = offset(from: beginningOfDocument, to: position)
        guard
            let row = commentSelection.rows.first(where: {
                index >= $0.range.location && index < NSMaxRange($0.range)
            })
        else { return nil }
        let source = NSMaxRange(row.range) - 1 - row.line.text.utf16.count
        guard let start = self.position(from: beginningOfDocument, offset: source) else { return nil }
        return point.x < caretRect(for: start).minX ? row : nil
    }

    @objc private func tapGutter(_ tap: UITapGestureRecognizer) {
        guard let row = gutterRow(at: tap.location(in: self)) else { return }
        selectGutterRow(row)
    }

    /// The same line again clears it; another line on the same side extends to it; anything
    /// else starts over at the tapped line.
    func selectGutterRow(_ row: SourceCommentSelection.Row) {
        let line = NSRange(location: row.range.location, length: max(0, row.range.length - 1))
        let current = selectedRange
        if NSEqualRanges(current, line) {
            selectedRange = NSRange(location: line.location, length: 0)
            reportSelection()
            return
        }
        var next = line
        if current.length > 0, commentSelection.anchor(in: NSUnionRange(current, line)) != nil {
            next = NSUnionRange(current, line)
        }
        if !isFirstResponder { _ = becomeFirstResponder() }
        selectedRange = next
        reportSelection()
    }

    func reportSelection() {
        guard let onSelectLines else { return }
        var anchor = commentSelection.anchor(in: selectedRange)
        if let side = anchor?.side, !sourceSides.contains(side) { anchor = nil }
        onSelectLines(anchor)
    }
}
