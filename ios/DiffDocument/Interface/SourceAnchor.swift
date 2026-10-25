public struct SourceAnchor: Sendable, Equatable, Codable {
    public enum Side: String, Sendable, Codable { case old, new }
    public let side: Side
    public let startLine: UInt32
    public let endLine: UInt32
    public let quote: String

    public init(side: Side, startLine: UInt32, endLine: UInt32, quote: String) {
        self.side = side
        self.startLine = startLine
        self.endLine = endLine
        self.quote = quote
    }
}
