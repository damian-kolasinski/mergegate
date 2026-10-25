import ConversationInterface

@MainActor
public final class FeedbackDraftStore: FeedbackDrafts {
    private let store: CommentDraftStore

    public init() throws { store = try CommentDraftStore() }

    init(store: CommentDraftStore) { self.store = store }

    public func load(scope: CommentDraftScope) throws -> [FeedbackDraft] {
        try store.load(scope: scope).map {
            FeedbackDraft(id: $0.id, location: $0.location, body: $0.body)
        }
    }

    public func removeSent(_ drafts: [FeedbackDraft], scope: CommentDraftScope) throws {
        for current in try store.load(scope: scope) {
            if drafts.contains(where: {
                $0.id == current.id && $0.body == current.body && $0.location == current.location
            }) {
                try store.delete(current, scope: scope)
            }
        }
    }
}
