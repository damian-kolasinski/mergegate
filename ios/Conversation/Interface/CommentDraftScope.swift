import DiffDocumentInterface
import Foundation

public struct CommentDraftScope: Sendable, Equatable {
    public let hub: URL
    public let item: String
    public let name: String
    public let revision: UInt32

    public init(hub: URL, item: String, name: String, revision: UInt32) {
        self.hub = hub
        self.item = item
        self.name = name
        self.revision = revision
    }
}

public struct CommentDraftLocation: Sendable, Equatable, Codable {
    public let path: String
    public let anchor: SourceAnchor

    public init(path: String, anchor: SourceAnchor) {
        self.path = path
        self.anchor = anchor
    }
}
