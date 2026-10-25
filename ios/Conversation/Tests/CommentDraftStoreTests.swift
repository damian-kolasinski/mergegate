import ConversationInterface
import DiffDocumentInterface
import Foundation
import SwiftData
import Testing

@testable import Conversation

struct CommentDraftStoreTests {
    @Test @MainActor func draftsSurviveFreshContainersWithExactAnchorsAndDeletion() throws {
        let directory = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appending(path: "drafts.store")
        let scope = scope()
        let old = draft(side: .old, start: 12, end: 14)
        var new = draft(side: .new, start: 21, end: 21)
        do {
            let store = try CommentDraftStore(configuration: .init(url: url))
            try store.save(old, scope: scope)
            try store.save(new, scope: scope)
        }
        do {
            let store = try CommentDraftStore(configuration: .init(url: url))
            let restored = try store.load(scope: scope)
            #expect(Set(restored.map(\.id)) == [old.id, new.id])
            #expect(restored.first(where: { $0.id == old.id }) == old)
            new.body = "Edited e\u{301} 🐙 feedback\nwith a second line"
            try store.save(new, scope: scope)
            try store.delete(old, scope: scope)
        }
        let reopened = try CommentDraftStore(configuration: .init(url: url))
        #expect(try reopened.load(scope: scope) == [new])
        try reopened.delete(new, scope: scope)
        #expect(try CommentDraftStore(configuration: .init(url: url)).load(scope: scope).isEmpty)
    }

    @Test @MainActor func scopesIsolateHubsItemsAndRevisionsAndNormalizeHubSlash() throws {
        let store = try CommentDraftStore(configuration: .init(isStoredInMemoryOnly: true))
        let draft = draft(side: .new, start: 1, end: 1)
        try store.save(draft, scope: scope())
        #expect(try store.load(scope: scope(hub: "http://my-mac:7800/")) == [draft])
        for other in [
            scope(hub: "http://another-mac:7800"), scope(item: "project#4"), scope(revision: 2),
        ] {
            #expect(try store.load(scope: other).isEmpty)
            try store.delete(draft, scope: other)
        }
        #expect(try store.load(scope: scope()) == [draft])
    }

    private func scope(
        hub: String = "http://my-mac:7800", item: String = "project#3", revision: UInt32 = 1
    ) -> CommentDraftScope {
        .init(hub: URL(string: hub)!, item: item, name: "project#44", revision: revision)
    }

    private func draft(side: SourceAnchor.Side, start: UInt32, end: UInt32) -> CommentDraft {
        .init(
            location: .init(
                path: "src/total + tax #é.swift",
                anchor: .init(
                    side: side, startLine: start, endLine: end, quote: "subtotal + delivery 🐙")),
            body: "Check this range")
    }
}
