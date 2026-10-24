public struct DiffLine: Sendable, Equatable {
    public enum Kind: Sendable { case context, added, removed, note }
    public let kind: Kind
    public let oldLine: UInt32?
    public let newLine: UInt32?
    public let text: String

    public init(kind: Kind, oldLine: UInt32?, newLine: UInt32?, text: String) {
        self.kind = kind
        self.oldLine = oldLine
        self.newLine = newLine
        self.text = text
    }
}

public struct DiffHunk: Sendable, Equatable {
    public let header: String
    public let lines: [DiffLine]

    public init(header: String, lines: [DiffLine]) {
        self.header = header
        self.lines = lines
    }
}
