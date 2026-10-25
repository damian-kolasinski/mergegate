public struct CodeSpan: Sendable, Equatable {
    public let start: Int
    public let length: Int
    public let tag: String

    public init(start: Int, length: Int, tag: String) {
        self.start = start
        self.length = length
        self.tag = tag
    }
}
