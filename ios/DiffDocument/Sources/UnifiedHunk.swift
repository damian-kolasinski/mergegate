import DiffDocumentInterface
import Foundation

public enum UnifiedHunk {
    // Keep source coordinates independent of the reader's wrapping and gutters.
    public static func build(
        oldStart: UInt32, oldLines: UInt32, newStart: UInt32, newLines: UInt32, lines: [String],
        spans: [[CodeSpan]] = []
    ) throws -> DiffHunk {
        guard oldLines == 0 || oldStart > 0, newLines == 0 || newStart > 0 else { throw InvalidHunk() }
        var old = oldStart
        var new = newStart
        var removed: UInt32 = 0
        var added: UInt32 = 0
        let parsed = try lines.enumerated().map { index, line -> DiffLine in
            let painting = index < spans.count ? spans[index] : []
            switch line.utf8.first {
            case 32:
                let result = DiffLine(
                    kind: .context, oldLine: old, newLine: new,
                    text: String(decoding: line.utf8.dropFirst(), as: UTF8.self), spans: painting)
                old = try advance(old)
                new = try advance(new)
                removed = try advance(removed)
                added = try advance(added)
                return result
            case 43:
                let result = DiffLine(
                    kind: .added, oldLine: nil, newLine: new,
                    text: String(decoding: line.utf8.dropFirst(), as: UTF8.self), spans: painting)
                new = try advance(new)
                added = try advance(added)
                return result
            case 45:
                let result = DiffLine(
                    kind: .removed, oldLine: old, newLine: nil,
                    text: String(decoding: line.utf8.dropFirst(), as: UTF8.self), spans: painting)
                old = try advance(old)
                removed = try advance(removed)
                return result
            case 92 where line == "\\ No newline at end of file":
                return DiffLine(kind: .note, oldLine: nil, newLine: nil, text: line)
            default: throw InvalidHunk()
            }
        }
        guard removed == oldLines, added == newLines else { throw InvalidHunk() }
        let oldBoundary = Int(oldLines == 0 ? oldStart : oldStart - 1)
        let newBoundary = Int(newLines == 0 ? newStart : newStart - 1)
        return DiffHunk(
            header: "@@ -\(oldStart),\(oldLines) +\(newStart),\(newLines) @@", lines: parsed,
            oldRange: oldBoundary..<(oldBoundary + Int(oldLines)),
            newRange: newBoundary..<(newBoundary + Int(newLines)))
    }

    private static func advance(_ value: UInt32) throws -> UInt32 {
        let (next, overflow) = value.addingReportingOverflow(1)
        guard !overflow else { throw InvalidHunk() }
        return next
    }
}

private struct InvalidHunk: LocalizedError {
    var errorDescription: String? { "The hub returned an invalid unified diff." }
}
