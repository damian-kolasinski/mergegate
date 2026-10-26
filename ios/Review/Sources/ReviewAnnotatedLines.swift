import ConversationInterface
import DiffDocumentInterface

// Splits a block of displayed lines after each line that a thread or draft ends on, so its card
// sits right below it. Unmarked blocks stay whole, keeping native selection across them.
enum ReviewAnnotatedLines {
    struct Segment {
        let hunk: DiffHunk
        /// The marks whose cards follow this segment; empty for the last one.
        let marks: [SourceMark]
    }

    static func split(_ hunk: DiffHunk, marks: Set<SourceMark>) -> [Segment] {
        guard !marks.isEmpty else { return [Segment(hunk: hunk, marks: [])] }
        var segments: [Segment] = []
        var lines: [DiffLine] = []
        var pending: [SourceMark] = []
        // The header names the whole hunk, so only its first segment shows it.
        var header = hunk.header
        func cut() {
            segments.append(
                Segment(
                    hunk: DiffHunk(
                        header: header, lines: lines, oldRange: hunk.oldRange,
                        newRange: hunk.newRange),
                    marks: pending))
            header = ""
            lines = []
            pending = []
        }
        for (index, line) in hunk.lines.enumerated() {
            lines.append(line)
            pending += self.marks(on: line).filter(marks.contains)
            // A "No newline" note belongs with its line, so the card waits until after it.
            let noteFollows = index + 1 < hunk.lines.count && hunk.lines[index + 1].kind == .note
            if !pending.isEmpty && !noteFollows { cut() }
        }
        if !lines.isEmpty || segments.isEmpty { cut() }
        return segments
    }

    /// Whether `hunk` shows the first line `anchor` names, so only that block reveals it.
    static func shows(_ hunk: DiffHunk, start anchor: SourceAnchor) -> Bool {
        hunk.lines.contains { line in
            (anchor.side == .old ? line.oldLine : line.newLine) == anchor.startLine
                && line.kind != (anchor.side == .old ? .added : .removed)
        }
    }

    /// A removal is only on the old side and an addition only on the new; unchanged context is
    /// on both, so a thread on either side's number lands below it.
    private static func marks(on line: DiffLine) -> [SourceMark] {
        switch line.kind {
        case .removed: [line.oldLine.map { SourceMark(side: .old, line: $0) }].compactMap { $0 }
        case .added: [line.newLine.map { SourceMark(side: .new, line: $0) }].compactMap { $0 }
        case .context:
            [
                line.newLine.map { SourceMark(side: .new, line: $0) },
                line.oldLine.map { SourceMark(side: .old, line: $0) },
            ].compactMap { $0 }
        case .note: []
        }
    }
}
