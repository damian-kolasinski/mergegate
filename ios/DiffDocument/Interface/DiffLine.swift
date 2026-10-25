public struct DiffLine: Sendable, Equatable {
    public enum Kind: Sendable { case context, added, removed, note }
    public let kind: Kind
    public let oldLine: UInt32?
    public let newLine: UInt32?
    public let text: String
    public let spans: [CodeSpan]
    public let hasNewline: Bool

    public init(
        kind: Kind, oldLine: UInt32?, newLine: UInt32?, text: String, hasNewline: Bool = true,
        spans: [CodeSpan] = []
    ) {
        self.kind = kind
        self.oldLine = oldLine
        self.newLine = newLine
        self.text = text
        self.hasNewline = hasNewline
        self.spans = spans
    }
}

public struct DiffHunk: Sendable, Equatable {
    public let header: String
    public let lines: [DiffLine]
    public let oldRange: Range<Int>
    public let newRange: Range<Int>

    public init(
        header: String, lines: [DiffLine], oldRange: Range<Int> = 0..<0, newRange: Range<Int> = 0..<0
    ) {
        self.header = header
        self.lines = lines
        self.oldRange = oldRange
        self.newRange = newRange
    }
}
