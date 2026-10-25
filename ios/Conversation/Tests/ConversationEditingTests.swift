import ConversationInterface
import Foundation
import Networking
import Testing

@testable import Conversation

struct ConversationEditingTests {
    private let scope = CommentDraftScope(
        hub: URL(string: "http://hub")!, item: "sample#1", name: "sample#11", revision: 2)

    @Test @MainActor func lostCreateResponseReusesSavedDraftAndRetainsTextOnStaleRevision()
        async throws
    {
        let transport = EditingTransport()
        let model = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: nil), text: " Unicode é 🐙 ",
            service: .init(scope: scope, transport: transport))
        await model.save()
        #expect(!model.saved && model.error != nil)
        await model.save()
        #expect(model.saved)
        #expect(await transport.creates == 1)
        let stale = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: nil), text: "Retained",
            service: .init(scope: scope, transport: transport))
        await transport.advance()
        await stale.save()
        #expect(!stale.saved && stale.error != nil)
        #expect(stale.text == "Retained")
        #expect(await transport.creates == 1)
    }
    @Test @MainActor func changedTextAfterLostCreateEditsTheRecoveredDraft() async throws {
        let transport = EditingTransport()
        let model = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: nil), text: "Original",
            service: .init(scope: scope, transport: transport))
        await model.save()
        model.text = "Corrected"
        await model.save()
        #expect(model.saved)
        #expect(await transport.creates == 1)
        #expect(await transport.draftBody() == "Corrected")
    }
    @Test @MainActor func repeatedLostResponsesKeepEditingTheSameRecoveredDraft() async throws {
        let transport = EditingTransport()
        let model = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: nil), text: "A",
            service: .init(scope: scope, transport: transport))
        await model.save()
        await transport.loseNextEdit()
        model.text = "B"
        await model.save()
        #expect(!model.saved)
        model.text = "C"
        await model.save()
        #expect(model.saved)
        #expect(await transport.creates == 1)
        #expect(await transport.draftBody() == "C")
    }

    @Test @MainActor func sentReplyIsNotRecreatedAfterUncertainDelivery() async throws {
        let transport = EditingTransport()
        let model = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: nil), text: "Answer",
            service: .init(scope: scope, transport: transport))
        await model.save()
        await transport.sendSavedReply()
        await model.save()
        #expect(model.saved)
        #expect(await transport.creates == 1)
    }
    @Test func decisionRechecksRejectionAndNeverOverridesAnAlreadyAnsweredThread() async throws {
        let transport = EditingTransport()
        let service = ConversationEditingService(scope: scope, transport: transport)
        await transport.answerRejection()
        await #expect(throws: (any Error).self) {
            try await service.decide(comment: "c_1", decision: .override(4, "Still needed"))
        }
        #expect(await transport.decisions == 0)
    }
    @Test @MainActor func editedReplyDisappearingIsNotRecreated() async throws {
        let transport = EditingTransport()
        let model = ConversationEditorModel(
            kind: .reply(comment: "c_1", id: 9), text: "Edited answer",
            service: .init(scope: scope, transport: transport))
        await model.save()
        #expect(model.error != nil && !model.saved)
        #expect(await transport.creates == 0)
    }
}
private actor EditingTransport: HTTPTransport {
    var revision = 2
    var creates = 0
    var decisions = 0
    var draft: [String: Any]?
    var replies: [[String: Any]] = []
    var lose = true
    var loseEdit = false
    func loseNextEdit() { loseEdit = true }
    func advance() { revision += 1 }
    func sendSavedReply() {
        if let draft { replies.append(draft) }
        draft = nil
    }
    func answerRejection() {
        replies = [
            reply(id: 4, author: "agent", body: "Not needed", status: "rejected"),
            reply(id: 5, author: "operator", body: "Okay"),
        ]
    }
    func get(_ url: URL) async throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "item": ["id": "sample#1", "state": "needs_review", "revision": ["number": revision]],
            "comments": [
                [
                    "id": "c_1", "revision": 2, "body": "Question",
                    "external": ["author": "reviewer", "pending": false],
                ]
            ],
            "replies": replies, "drafts": [], "draft_replies": draft.map { [$0] } ?? [],
        ])
    }
    func post(_ url: URL, body: Data) async throws -> Data {
        if url.lastPathComponent == "drafts" {
            creates += 1
            let text = (try JSONSerialization.jsonObject(with: body) as! [String: String])["body"]!
            let saved = reply(id: 9, author: "operator", body: text)
            draft = saved
            if lose {
                lose = false
                throw URLError(.networkConnectionLost)
            }
            return try JSONSerialization.data(withJSONObject: saved)
        }
        decisions += 1
        throw URLError(.unsupportedURL)
    }
    func draftBody() -> String? { draft?["body"] as? String }
    func put(_ url: URL, body: Data) async throws -> Data {
        guard draft != nil else { throw URLError(.unsupportedURL) }
        draft?["body"] = (try JSONSerialization.jsonObject(with: body) as! [String: String])["body"]
        if loseEdit {
            loseEdit = false
            throw URLError(.networkConnectionLost)
        }
        return try JSONSerialization.data(withJSONObject: draft!)
    }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
    private func reply(id: Int, author: String, body: String, status: String? = nil) -> [String:
        Any]
    {
        [
            "id": id, "comment": "c_1", "author": author, "body": body,
            "status": status as Any? ?? NSNull(), "created_at": "2026-10-25T10:00:00Z",
        ]
    }
}
