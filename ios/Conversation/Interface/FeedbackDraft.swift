import Foundation

public struct FeedbackDraft: Identifiable, Equatable, Codable, Sendable {
    public let id: UUID
    public let location: CommentDraftLocation
    public let body: String

    public init(id: UUID, location: CommentDraftLocation, body: String) {
        self.id = id
        self.location = location
        self.body = body
    }
}

@MainActor
public protocol FeedbackDrafts {
    func load(scope: CommentDraftScope) throws -> [FeedbackDraft]
    /// Removes only the confirmed snapshot; edits made later remain local.
    func removeSent(_ drafts: [FeedbackDraft], scope: CommentDraftScope) throws
}
