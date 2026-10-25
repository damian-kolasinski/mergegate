import ConversationInterface
import Foundation
import SwiftData
import Testing

@testable import Conversation

struct FeedbackDraftStoreTests {
    @Test @MainActor func removesOnlyConfirmedSnapshotsAndPreservesLaterEditsAndOtherRevisions() throws {
        let scope = CommentDraftScope(hub: URL(string: "http://my-mac")!, item: "app#1", name: "app#44", revision: 1)
        let other = CommentDraftScope(hub: scope.hub, item: scope.item, name: scope.name, revision: 2)
        let store = try CommentDraftStore(configuration: .init(isStoredInMemoryOnly: true))
        let bridge = FeedbackDraftStore(store: store)
        var draft = CommentDraft(
            location: .init(
                path: "checkout.swift",
                anchor: .init(
                    side: .old, startLine: 3, endLine: 4, quote: "subtotal 🐙")), body: "Initial feedback")
        try store.save(draft, scope: scope)
        let sent = try bridge.load(scope: scope)
        #expect(sent.count == 1)
        #expect(sent.first?.location == draft.location)
        #expect(try bridge.load(scope: other).isEmpty)
        draft.body = "Edited during delivery"
        try store.save(draft, scope: scope)
        try bridge.removeSent(sent, scope: scope)
        #expect(try store.load(scope: scope).first?.body == draft.body)
        try bridge.removeSent(bridge.load(scope: scope), scope: other)
        #expect(try store.load(scope: scope).count == 1)
        try bridge.removeSent(bridge.load(scope: scope), scope: scope)
        #expect(try store.load(scope: scope).isEmpty)
    }
}
