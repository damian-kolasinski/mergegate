import ConversationInterface
import Foundation
import Testing

@testable import Conversation

struct CommentDraftViewModelTests {
    @Test @MainActor func unchangedDraftCanBeReadWithoutWritingAndUnsavedEditsCanBeDiscarded() throws {
        let store = DraftMemory()
        let model = try model(store)
        model.body = "Saved feedback"
        model.persist()
        model.showList()
        store.fail = true
        model.edit(try #require(model.drafts.first))
        model.persist()
        #expect(model.error == nil)
        #expect(!model.hasUnsavedChanges)
        model.showList()
        #expect(model.editing == nil)
        model.edit(try #require(model.drafts.first))
        model.body = "Unsaved replacement"
        model.persist()
        #expect(model.hasUnsavedChanges)
        model.discardChanges()
        #expect(model.error == nil)
        #expect(model.editing == nil)
        #expect(model.drafts.first?.body == "Saved feedback")
    }

    @Test @MainActor func typingPersistsWithoutClosingAndEditingKeepsIdentity() throws {
        let store = DraftMemory()
        let model = try model(store)
        #expect(store.values.isEmpty)
        model.persist()
        #expect(store.values.isEmpty)
        model.body = "A partially written thought 🐙"
        model.persist()
        let id = try #require(model.editing?.id)
        #expect(store.values.first?.body == model.body)
        model.showList()
        model.edit(try #require(model.drafts.first))
        model.body = ""
        model.persist()
        #expect(store.values.count == 1)
        #expect(store.values.first?.id == id)
        #expect(store.values.first?.body == "")
        model.delete()
        #expect(model.editing == nil)
        #expect(store.values.isEmpty)
    }

    @Test @MainActor func failedSaveRetainsTextAndAnchorUntilRetry() throws {
        let store = DraftMemory()
        let model = try model(store)
        store.fail = true
        model.body = "Feedback that must survive a failed save"
        let original = model.editing
        model.showList()
        #expect(model.error != nil)
        #expect(model.editing == original)
        #expect(model.body == "Feedback that must survive a failed save")
        #expect(model.drafts.isEmpty)
        store.fail = false
        model.persist()
        #expect(model.error == nil)
        #expect(store.values.first?.location == original?.location)
        store.fail = true
        model.delete()
        #expect(model.error != nil)
        #expect(model.drafts.count == 1)
        store.fail = false
        model.retry()
        #expect(model.drafts.isEmpty)
    }

    @MainActor private func model(_ store: DraftMemory) throws -> CommentDraftViewModel {
        try .init(
            scope: .init(hub: URL(string: "http://my-mac:7800")!, item: "p#1", name: "p#1", revision: 3),
            store: store,
            location: .init(
                path: "file.swift",
                anchor: .init(
                    side: .old, startLine: 4, endLine: 6, quote: "old source")))
    }
}

@MainActor private final class DraftMemory: CommentDraftStoring {
    var values: [CommentDraft] = []
    var fail = false
    func load(scope: CommentDraftScope) throws -> [CommentDraft] { values }
    func save(_ draft: CommentDraft, scope: CommentDraftScope) throws {
        if fail { throw CocoaError(.fileWriteOutOfSpace) }
        values.removeAll { $0.id == draft.id }
        values.append(draft)
    }
    func delete(_ draft: CommentDraft, scope: CommentDraftScope) throws {
        if fail { throw CocoaError(.fileWriteNoPermission) }
        values.removeAll { $0.id == draft.id }
    }
}
