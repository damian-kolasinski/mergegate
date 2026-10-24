import DiffDocumentInterface
import Foundation

public struct SourceContext: Sendable {
    public struct Gap: Sendable, Identifiable {
        public let id: Int
        public let lines: [DiffLine]
    }
    public let gaps: [Gap]

    // Blob IDs pin both sides to the displayed comparison, even if a new revision arrives.
    public init(hunks: [DiffHunk], old: String, new: String) throws {
        let oldLines = Self.lines(old)
        let newLines = Self.lines(new)
        var oldEnd = 0
        var newEnd = 0
        var gaps: [Gap] = []
        for index in 0...hunks.count {
            let oldStart = index < hunks.count ? hunks[index].oldRange.lowerBound : oldLines.count
            let newStart = index < hunks.count ? hunks[index].newRange.lowerBound : newLines.count
            guard oldStart >= oldEnd, newStart >= newEnd,
                oldStart <= oldLines.count, newStart <= newLines.count,
                oldStart - oldEnd == newStart - newEnd
            else { throw InvalidContext() }
            let lines = try (0..<(oldStart - oldEnd)).map { offset -> DiffLine in
                guard oldLines[oldEnd + offset] == newLines[newEnd + offset] else { throw InvalidContext() }
                return DiffLine(
                    kind: .context, oldLine: UInt32(oldEnd + offset + 1),
                    newLine: UInt32(newEnd + offset + 1), text: newLines[newEnd + offset],
                    hasNewline: newEnd + offset < newLines.count - 1 || new.utf8.last == 10)
            }
            gaps.append(Gap(id: index, lines: lines))
            if index < hunks.count {
                oldEnd = hunks[index].oldRange.upperBound
                newEnd = hunks[index].newRange.upperBound
                guard oldEnd <= oldLines.count, newEnd <= newLines.count else { throw InvalidContext() }
            }
        }
        self.gaps = gaps
    }

    private static func lines(_ source: String) -> [String] {
        guard !source.isEmpty else { return [] }
        var lines = source.components(separatedBy: "\n")
        if source.utf8.last == 10 { lines.removeLast() }
        return lines
    }
}

private struct InvalidContext: LocalizedError {
    var errorDescription: String? {
        "Source context does not match this comparison. Return to the inbox and reopen the review."
    }
}
