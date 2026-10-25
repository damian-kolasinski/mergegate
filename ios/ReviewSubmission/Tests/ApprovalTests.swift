import ConversationInterface
import Foundation
import Networking
import Testing

@testable import ReviewSubmission

@MainActor struct ApprovalTests {
    @Test func requiresExactTargetConfirmationAndAllowsEmptyApproval() async throws {
        let hub = ApprovalHub()
        let model = try model(hub)
        await model.load()
        model.verdict = .approved
        #expect(model.canSend)
        await model.send()
        await model.send(confirmedTarget: "release")
        #expect(await hub.requests == 0)
        await model.send(confirmedTarget: "main")
        #expect(model.sent && model.sentVerdict == .approved)
        #expect(model.approval?.kind == "merged")
        #expect(model.approval?.commit == "abc123")
        #expect(await hub.requests == 1)
        await model.send(confirmedTarget: "main")
        #expect(await hub.requests == 1)
    }

    @Test func revisionChangingAfterConfirmationNeverApproves() async throws {
        let hub = ApprovalHub()
        let model = try model(hub)
        await model.load()
        model.verdict = .approved
        await hub.advance()
        await model.send(confirmedTarget: "main")
        #expect(!model.sent && model.error?.contains("newer revision") == true)
        #expect(await hub.requests == 0)
    }

    @Test func revisionChangingAtEndpointIsRejectedAndSummaryRetained() async throws {
        let hub = ApprovalHub()
        let model = try model(hub)
        await model.load()
        model.verdict = .approved
        model.summary = "Reviewed calculation é 🐙"
        await hub.rejectStale()
        await model.send(confirmedTarget: "main")
        #expect(!model.sent && !model.uncertain)
        #expect(model.summary == "Reviewed calculation é 🐙")
        #expect(model.error?.contains("stale") == true)
    }

    @Test func lostResponseReconcilesApprovalWithoutApprovingAgain() async throws {
        let hub = ApprovalHub()
        await hub.loseResponse()
        let outbox = MemoryOutbox()
        let model = try model(hub, outbox: outbox)
        await model.load()
        model.verdict = .approved
        await model.send(confirmedTarget: "main")
        #expect(model.sent && model.approval?.commit == "abc123")
        #expect(await hub.requests == 1)
    }

    @Test func lostHeldAndConflictResponsesRetainActionableResult() async throws {
        for kind in ["held", "conflict"] {
            let hub = ApprovalHub(kind: kind)
            await hub.loseResponse()
            let model = try model(hub)
            await model.load()
            model.verdict = .approved
            await model.send(confirmedTarget: "main")
            #expect(model.sent && model.approval?.kind == kind)
            #expect(
                model.approval?.detail.contains(kind == "held" ? "Push failed" : "checkout.swift") == true)
            #expect(await hub.requests == 1)
        }
    }

    @Test func heldConflictAndWaitingRemainDistinctFromMerged() async throws {
        for kind in ["held", "conflict", "waiting"] {
            let hub = ApprovalHub(kind: kind)
            let model = try model(hub)
            await model.load()
            model.verdict = .approved
            await model.send(confirmedTarget: "main")
            #expect(model.sent && model.approval?.kind == kind)
            #expect(model.approval?.title != "Merged")
            #expect(model.approval?.commit == nil)
        }
    }

    @Test func uncertainApprovalSurvivesRelaunchWithoutBlindRetry() async throws {
        let hub = ApprovalHub()
        await hub.failBeforeApproval()
        let outbox = MemoryOutbox()
        let first = try model(hub, outbox: outbox)
        await first.load()
        first.verdict = .approved
        await first.send(confirmedTarget: "main")
        #expect(first.uncertain && !first.sent)
        let restored = try model(hub, outbox: outbox)
        await restored.load()
        await restored.send()
        #expect(await hub.requests == 1)
        #expect(restored.uncertain && !restored.sent)
        await restored.send(allowResend: true, confirmedTarget: "main")
        #expect(restored.sent && restored.approval?.kind == "merged")
        #expect(await hub.requests == 2)
    }

    @Test func closedItemsAndChangedTargetDoNotApprove() async throws {
        for state in ["merged", "merging", "abandoned"] {
            let hub = ApprovalHub(state: state)
            let model = try model(hub)
            await model.load()
            model.verdict = .approved
            await model.send(confirmedTarget: "main")
            #expect(await hub.requests == 0)
        }
        let hub = ApprovalHub()
        let model = try model(hub)
        await model.load()
        model.verdict = .approved
        await hub.changeTarget()
        await model.send(confirmedTarget: "main")
        #expect(await hub.requests == 0)
    }

    private func model(_ hub: ApprovalHub, outbox: MemoryOutbox = MemoryOutbox()) throws
        -> FeedbackViewModel
    {
        try FeedbackViewModel(
            scope: .init(hub: URL(string: "http://my-mac")!, item: "app#1", name: "app#44", revision: 3),
            drafts: LocalDrafts(), outbox: outbox, service: .init(transport: hub))
    }
}

actor ApprovalHub: HTTPTransport {
    var revision = 3
    var state: String
    var target = "main"
    let kind: String
    var requests = 0
    var approved = false
    var lost = false
    var disconnected = false
    var stale = false
    init(kind: String = "merged", state: String = "needs_review") {
        self.kind = kind
        self.state = state
    }
    func advance() { revision += 1 }
    func changeTarget() { target = "release" }
    func rejectStale() { stale = true }
    func loseResponse() { lost = true }
    func failBeforeApproval() { disconnected = true }
    func get(_ url: URL) throws -> Data {
        if url.path == "/api/events" {
            var events: [[String: Any]] = []
            if approved && url.query != nil {
                events.append([
                    "item": "app#1", "revision": 3, "type": "review.submitted",
                    "verdict": "approved", "summary": "", "comments": [], "replies": [],
                ])
                if kind == "merged" {
                    events.append(["item": "app#1", "revision": 3, "type": "item.merged", "commit": "abc123"])
                } else if kind == "held" {
                    events.append([
                        "item": "app#1", "revision": 3, "type": "merge.held", "reason": "Push failed",
                    ])
                } else if kind == "conflict" {
                    events.append([
                        "item": "app#1", "revision": 3, "type": "merge.conflict", "paths": ["checkout.swift"],
                    ])
                }
            }
            return try json(["cursor": approved ? 2 : 0, "events": events])
        }
        return try json(["item": item(), "drafts": [], "draft_replies": []])
    }
    func post(_ url: URL, body: Data) throws -> Data {
        #expect(url.path == "/api/items/app#1/approve" && url.fragment == nil)
        let value = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(value["revision"] as? Int == 3)
        requests += 1
        if stale { throw HTTPFailure(status: 409, code: "stale_revision", message: "stale approval") }
        if disconnected {
            disconnected = false
            throw URLError(.networkConnectionLost)
        }
        #expect(value["summary"] as? String == "")
        approved = true
        state = kind == "merged" ? "merged" : "merging"
        if lost { throw URLError(.timedOut) }
        var outcome: [String: Any] = ["kind": kind]
        switch kind {
        case "merged": outcome["commit"] = "abc123"
        case "held": outcome["reason"] = "Push failed"
        case "waiting": outcome["parent"] = "app#43"
        default: outcome["paths"] = ["checkout.swift"]
        }
        return try json(["item": item(), "outcome": outcome])
    }
    func put(_ url: URL, body: Data) throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) throws -> Data { throw URLError(.unsupportedURL) }
    private func item() -> [String: Any] {
        ["id": "app#1", "revision": ["number": revision], "state": state, "target": target]
    }
    private func json(_ value: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: value)
    }
}

extension FeedbackTests {
    @Test func approvalSendsPreviewedCommentsRepliesAndSummaryBeforeCleanup() async throws {
        let hub = FeedbackHub()
        await hub.addHubReply()
        await hub.addHubDraft()
        let local = LocalDrafts()
        local.values = [draft(.old), draft(.new)]
        let model = try model(hub, local, MemoryOutbox())
        await model.load()
        model.verdict = .approved
        model.summary = "Reviewed delivery é 🐙"
        await model.send(confirmedTarget: "main")
        #expect(model.sent && model.approval?.kind == "merged")
        #expect(local.values.isEmpty)
        #expect(await hub.verdicts == ["approved"])
        #expect(await hub.summaries == ["Reviewed delivery é 🐙"])
        #expect(await hub.sent.count == 3)
        #expect(await hub.sentReplies.count == 1)
        #expect(await hub.uploads.count == 2)
    }
}
