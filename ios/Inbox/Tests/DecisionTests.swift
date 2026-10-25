import Foundation
import Networking
import Testing

@testable import Inbox

struct DecisionTests {
    @Test @MainActor func savesRestoresAndEditsAnswerWithoutSendingIt() async throws {
        let hub = DecisionHub()
        let model = try await make(hub)
        await model.load()
        #expect(model.loaded)
        model.text = "  Receipt client é 🐙  "
        await model.saveAnswer()
        #expect(model.answerSaved)
        #expect(await hub.creates == 1)
        #expect(await hub.actions.isEmpty)
        let reopened = try await make(hub)
        await reopened.load()
        #expect(reopened.text == "Receipt client é 🐙")
        #expect(reopened.answerSaved)
        reopened.text = "Both clients"
        #expect(!reopened.answerSaved)
        await reopened.saveAnswer()
        #expect(await hub.creates == 1)
        #expect(await hub.edits == 1)
        #expect(reopened.answerSaved)
    }

    @Test @MainActor func lostCreateResponseRetriesTheSavedReplyWithoutDuplicating() async throws {
        let hub = DecisionHub()
        let model = try await make(hub)
        await model.load()
        model.text = "Receipt client"
        await hub.loseCreate()
        await model.saveAnswer()
        #expect(model.error != nil)
        #expect(model.text == "Receipt client")
        await model.saveAnswer()
        #expect(model.error == nil)
        #expect(await hub.creates == 1)
        #expect(await hub.edits == 1)
    }

    @Test @MainActor func sentAnswerReplacesUnsentEditorAfterFeedbackCloses() async throws {
        let hub = DecisionHub()
        let model = try await make(hub)
        await model.load()
        model.text = "Receipt client"
        await model.saveAnswer()
        await model.feedbackClosed()
        #expect(model.result == nil)
        await hub.sendAnswer()
        await model.feedbackClosed()
        #expect(model.result == "Answer sent")
        #expect(model.error == nil)
    }

    @Test @MainActor func lostSettlementResponseIsConfirmedWithoutSendingAgain() async throws {
        for followup in ["", "revision", "followup"] {
            for override in [true, false] {
                let hub = DecisionHub(rejected: true)
                let model = try await make(hub)
                await model.load()
                model.text = "Still needed"
                await hub.loseSettlement()
                await model.settle(override: override)
                #expect(model.error != nil)
                await hub.change(followup)
                await model.settle(override: override)
                #expect(model.result == (override ? "Override sent" : "Rejection accepted"))
                #expect(model.error == nil)
                #expect(await hub.actions.count == 1)
            }
        }
    }

    @Test @MainActor func refusesStaleRevisionChangedQuestionClosedAndWrongItem() async throws {
        for change in ["revision", "reply", "closed", "identity"] {
            let hub = DecisionHub()
            let model = try await make(hub)
            await model.load()
            model.text = "Retained answer"
            await hub.change(change)
            await model.saveAnswer()
            #expect(model.error != nil)
            #expect(model.text == "Retained answer")
            #expect(await hub.creates == 0)
        }
    }

    @Test @MainActor func acceptsOrOverridesOnlyTheDisplayedRejection() async throws {
        for override in [false, true] {
            let hub = DecisionHub(rejected: true)
            let model = try await make(hub)
            await model.load()
            model.text = "  Still needed é 🐙  "
            await model.settle(override: override)
            #expect(model.result == (override ? "Override sent" : "Rejection accepted"))
            #expect(await hub.actions == [override ? "override" : "agree"])
            #expect(await hub.note == (override ? "Still needed é 🐙" : nil))
            await model.settle(override: override)
            #expect(await hub.actions.count == 1)
        }
    }

    @Test @MainActor func failedLoadAndWriteCanRetryWithoutLosingText() async throws {
        let hub = DecisionHub(rejected: true)
        let model = try await make(hub)
        await hub.failNext()
        await model.load()
        #expect(!model.loaded)
        await model.load()
        #expect(model.loaded)
        model.text = "Keep this note"
        await hub.failNext()
        await model.settle(override: true)
        #expect(model.error != nil)
        #expect(model.text == "Keep this note")
        await model.settle(override: true)
        #expect(model.result == "Override sent")
    }

    @MainActor private func make(_ hub: DecisionHub) async throws -> DecisionViewModel {
        let data = try await hub.get(URL(string: "http://my-mac/api/items/app%231")!)
        let snapshot = try JSONDecoder().decode(DecisionSnapshot.self, from: data)
        let item = try JSONDecoder().decode(
            InboxItem.self,
            from: Data(
                """
                {"id":"app#1","name":"app#91","title":"Receipt","state":"changes_requested","revision":1,"review_files":1,"additions":1,"deletions":0}
                """.utf8))
        return DecisionViewModel(
            item: item, decision: try #require(snapshot.decisions.first),
            service: .init(transport: hub, hub: URL(string: "http://my-mac")!))
    }
}

private actor DecisionHub: HTTPTransport {
    let rejected: Bool
    var creates = 0
    var edits = 0
    var actions: [String] = []
    var note: String?
    var reply: String?
    var changed = ""
    var settled: String?
    var settlementLost = false
    var answerSent = false
    var failing = false
    var lost = false
    init(rejected: Bool = false) { self.rejected = rejected }
    func sendAnswer() { answerSent = true }
    func loseSettlement() { settlementLost = true }
    func loseCreate() { lost = true }
    func change(_ value: String) { changed = value }
    func failNext() { failing = true }
    func get(_ url: URL) throws -> Data {
        #expect(url.path == "/api/items/app#1")
        #expect(url.fragment == nil)
        if failing {
            failing = false
            throw URLError(.timedOut)
        }
        var external: [String: Any] = ["pending": false, "vote": "up"]
        if settled == "agree" { external["agreed_reply"] = 1 }
        var comment: [String: Any] = ["id": "c_1", "body": "Keep charged total"]
        if rejected { comment["external"] = external }
        var replies: [[String: Any]] = [
            [
                "id": changed == "reply" ? 2 : 1, "comment": "c_1", "author": "agent",
                "status": rejected ? "rejected" : "question", "body": "Which clients?",
            ]
        ]
        if settled == "override" || answerSent {
            replies.append([
                "id": 10, "comment": "c_1", "author": "operator",
                "overrides": settled == "override",
                "body": answerSent ? reply! : note!,
            ])
        }
        if changed == "followup" {
            replies.append([
                "id": 11, "comment": "c_1", "author": "agent", "status": "addressed",
                "body": "Fixed",
            ])
        }
        return try json([
            "item": [
                "id": changed == "identity" ? "app#2" : "app#1",
                "revision": ["number": changed == "revision" ? 2 : 1],
                "state": changed == "closed" ? "merged" : "changes_requested",
            ],
            "comments": [comment],
            "replies": replies,
            "draft_replies": reply.map { [["id": 9, "comment": "c_1", "body": $0]] } ?? [],
        ])
    }
    func post(_ url: URL, body: Data) throws -> Data {
        #expect(url.fragment == nil)
        let payload = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        if url.lastPathComponent == "drafts" {
            #expect(url.path == "/api/comments/c_1/drafts")
            creates += 1
            reply = try #require(payload["body"] as? String)
            if lost {
                lost = false
                throw URLError(.timedOut)
            }
            return try json(["id": 9, "comment": "c_1", "body": reply!])
        }
        #expect(["/api/comments/c_1/agree", "/api/comments/c_1/override"].contains(url.path))
        actions.append(url.lastPathComponent)
        note = payload["note"] as? String
        settled = url.lastPathComponent
        if settlementLost {
            settlementLost = false
            throw URLError(.timedOut)
        }
        if settled == "override" {
            return try json([
                "id": 10, "comment": "c_1", "author": "operator", "overrides": true, "body": note!,
            ])
        }
        return try json([
            "id": "c_1", "body": "Keep charged total",
            "external": ["pending": false, "agreed_reply": 1],
        ])
    }
    func put(_ url: URL, body: Data) throws -> Data {
        #expect(url.path == "/api/comments/c_1/drafts/9")
        edits += 1
        let payload = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        reply = try #require(payload["body"] as? String)
        return try json(["id": 9, "comment": "c_1", "body": reply!])
    }
    func delete(_ url: URL) throws -> Data { throw URLError(.unsupportedURL) }
    private func json(_ value: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: value)
    }
}
