import ConversationInterface
import Foundation
import SwiftData

@MainActor
protocol CommentDraftStoring {
    func load(scope: CommentDraftScope) throws -> [CommentDraft]
    func save(_ draft: CommentDraft, scope: CommentDraftScope) throws
    func delete(_ draft: CommentDraft, scope: CommentDraftScope) throws
}

@MainActor
final class CommentDraftStore: CommentDraftStoring {
    private let container: ModelContainer
    private let context: ModelContext

    init(configuration: ModelConfiguration = ModelConfiguration()) throws {
        container = try ModelContainer(for: StoredCommentDraft.self, configurations: configuration)
        context = ModelContext(container)
        context.autosaveEnabled = false
    }

    func load(scope: CommentDraftScope) throws -> [CommentDraft] {
        try records(scope: scope).compactMap(\.value).sorted { $0.updatedAt > $1.updatedAt }
    }

    func save(_ draft: CommentDraft, scope: CommentDraftScope) throws {
        if let existing = try records(scope: scope).first(where: { $0.id == draft.id }) {
            existing.body = draft.body
            existing.updatedAt = draft.updatedAt
        } else {
            context.insert(StoredCommentDraft(scope: scope, draft: draft))
        }
        try commit()
    }

    func delete(_ draft: CommentDraft, scope: CommentDraftScope) throws {
        if let existing = try records(scope: scope).first(where: { $0.id == draft.id }) {
            context.delete(existing)
            try commit()
        }
    }

    private func records(scope: CommentDraftScope) throws -> [StoredCommentDraft] {
        let hub = scope.storageHub
        let item = scope.item
        let revision = Int(scope.revision)
        return try context.fetch(
            FetchDescriptor<StoredCommentDraft>(
                predicate: #Predicate {
                    $0.hub == hub && $0.item == item && $0.revision == revision
                }))
    }

    private func commit() throws {
        do { try context.save() } catch {
            context.rollback()
            throw error
        }
    }
}
