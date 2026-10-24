import DiffDocument
import Testing

struct UnifiedHunkTests {
    @Test func coordinatesFollowBothSidesAndIgnoreNewlineNotes() throws {
        let hunk = try UnifiedHunk.build(
            oldStart: 12, oldLines: 2, newStart: 20, newLines: 2,
            lines: [" same", "-old", "\\ No newline at end of file", "+new", "\\ No newline at end of file"])
        #expect(hunk.lines.map(\.oldLine) == [12, 13, nil, nil, nil])
        #expect(hunk.lines.map(\.newLine) == [20, nil, nil, 21, nil])
        #expect(
            hunk.lines.map(\.text) == [
                "same", "old", "\\ No newline at end of file", "new", "\\ No newline at end of file",
            ])
    }

    @Test func combiningMarksAfterPrefixRemainSourceText() throws {
        let source = "\u{0301}leading mark"
        let hunk = try UnifiedHunk.build(oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: ["+" + source])
        #expect(hunk.lines.first?.text == source)
    }

    @Test func additionsAndDeletionsPermitEmptySides() throws {
        let added = try UnifiedHunk.build(oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: ["+é 🐙"])
        #expect(added.lines.first?.oldLine == nil)
        #expect(added.lines.first?.newLine == 1)
        let deleted = try UnifiedHunk.build(oldStart: 1, oldLines: 1, newStart: 0, newLines: 0, lines: ["-gone"])
        #expect(deleted.lines.first?.newLine == nil)
    }

    @Test func invalidBoundaryInputFailsInsteadOfInventingCoordinates() {
        #expect(throws: (any Error).self) {
            try UnifiedHunk.build(oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: ["?invalid"])
        }
        #expect(throws: (any Error).self) {
            try UnifiedHunk.build(oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: ["+added"])
        }
        #expect(throws: (any Error).self) {
            try UnifiedHunk.build(oldStart: .max, oldLines: 1, newStart: 1, newLines: 0, lines: ["-overflow"])
        }
    }
}
