import DiffDocumentInterface
import Foundation

struct SourceCommentSelection {
    struct Row {
        let range: NSRange
        let line: DiffLine
    }
    let rows: [Row]

    func conversationAnchors(in selection: NSRange) -> [SourceAnchor] {
        guard selection.length > 0 else { return [] }
        let selected = rows.filter { NSIntersectionRange($0.range, selection).length > 0 }
        return [SourceAnchor.Side.old, .new].compactMap { side in
            let lines = selected.filter { (side == .old ? $0.line.oldLine : $0.line.newLine) != nil }
            let numbers = lines.compactMap { side == .old ? $0.line.oldLine : $0.line.newLine }
            guard let first = numbers.first, first > 0,
                numbers.enumerated().allSatisfy({ UInt64($0.element) == UInt64(first) + UInt64($0.offset) })
            else { return nil }
            return SourceAnchor(side: side, startLine: first, endLine: numbers.last!, quote: "")
        }
    }

    func anchor(in selection: NSRange) -> SourceAnchor? {
        guard selection.length > 0 else { return nil }
        let selected = rows.filter { NSIntersectionRange($0.range, selection).length > 0 }
        guard !selected.isEmpty else { return nil }
        let side: SourceAnchor.Side = selected.contains { $0.line.kind == .removed } ? .old : .new
        let numbers = selected.compactMap { side == .old ? $0.line.oldLine : $0.line.newLine }
        // A selection crossing both sides or an undisplayed gap cannot name one source range.
        guard numbers.count == selected.count, let first = numbers.first, first > 0,
            numbers.enumerated().allSatisfy({ UInt64($0.element) == UInt64(first) + UInt64($0.offset) })
        else { return nil }
        return SourceAnchor(
            side: side, startLine: first, endLine: numbers.last!,
            quote: selected.suffix(5).map(\.line.text).joined(separator: "\n"))
    }
}
