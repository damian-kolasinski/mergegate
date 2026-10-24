import DiffDocument
import Testing

struct SourceContextTests {
    @Test func gapsRetainCoordinatesAcrossInsertionsAndDeletions() throws {
        let first = try UnifiedHunk.build(
            oldStart: 2, oldLines: 1, newStart: 2, newLines: 2,
            lines: ["-old", "+new", "+extra"])
        let second = try UnifiedHunk.build(oldStart: 4, oldLines: 1, newStart: 4, newLines: 0, lines: ["-remove"])
        let context = try SourceContext(
            hunks: [first, second], old: "before\nold\nbetween\nremove\nafter",
            new: "before\nnew\nextra\nbetween\nafter")
        #expect(context.gaps.map { $0.lines.map(\.text) } == [["before"], ["between"], ["after"]])
        #expect(context.gaps.map { $0.lines.first?.oldLine } == [1, 3, 5])
        #expect(context.gaps.map { $0.lines.first?.newLine } == [1, 4, 5])
        #expect(context.gaps.last?.lines.last?.hasNewline == false)
    }

    @Test func emptySidesAndMetadataOnlyFiles() throws {
        let insertion = try UnifiedHunk.build(oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: ["+new"])
        #expect(try SourceContext(hunks: [insertion], old: "", new: "new\n").gaps.allSatisfy { $0.lines.isEmpty })
        let deletion = try UnifiedHunk.build(oldStart: 1, oldLines: 1, newStart: 0, newLines: 0, lines: ["-old"])
        #expect(try SourceContext(hunks: [deletion], old: "old\n", new: "").gaps.allSatisfy { $0.lines.isEmpty })
        let unchanged = try SourceContext(hunks: [], old: "é 🐙\n\nlast\n", new: "é 🐙\n\nlast\n")
        #expect(unchanged.gaps[0].lines.map(\.text) == ["é 🐙", "", "last"])
        #expect(unchanged.gaps[0].lines.last?.hasNewline == true)
    }

    @Test func crlfDoesNotCreateAPhantomEOFLine() throws {
        let context = try SourceContext(hunks: [], old: "first\r\nlast\r\n", new: "first\r\nlast\r\n")
        #expect(context.gaps[0].lines.map(\.text) == ["first\r", "last\r"])
        #expect(context.gaps[0].lines.last?.hasNewline == true)
        let final = try UnifiedHunk.build(
            oldStart: 2, oldLines: 1, newStart: 2, newLines: 1,
            lines: ["-last\r", "+last", "\\ No newline at end of file"])
        let changed = try SourceContext(hunks: [final], old: "first\r\nlast\r\n", new: "first\r\nlast")
        #expect(changed.gaps.map { $0.lines.map(\.text) } == [["first\r"], []])
    }

    @Test func invalidOrMismatchedBlobsDoNotInventContext() throws {
        let hunk = try UnifiedHunk.build(oldStart: 3, oldLines: 1, newStart: 3, newLines: 1, lines: ["-old", "+new"])
        #expect(throws: (any Error).self) { try SourceContext(hunks: [hunk], old: "short", new: "short") }
        #expect(throws: (any Error).self) { try SourceContext(hunks: [], old: "wrong", new: "right") }
        #expect(throws: (any Error).self) {
            try SourceContext(hunks: [hunk, hunk], old: "a\nb\nold", new: "a\nb\nnew")
        }
    }
}
