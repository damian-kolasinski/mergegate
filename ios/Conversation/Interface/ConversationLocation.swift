import DiffDocumentInterface

public struct ConversationLocation: Sendable {
    public let path: String
    public let anchors: [SourceAnchor]

    public init(path: String, anchors: [SourceAnchor]) {
        self.path = path
        self.anchors = anchors
    }
}
