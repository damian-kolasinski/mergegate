import ConversationInterface
import DiffDocumentInterface
import Foundation
import Networking
import SwiftData
import Testing

@testable import ReviewSubmission

@MainActor
struct FeedbackTests {
    @Test func offlineSummaryAndDraftsRequireFreshRevisionBeforeSending() async throws {
        let hub = FeedbackHub()
        let drafts = LocalDrafts()
        drafts.values = [draft(.new)]
        let outbox = MemoryOutbox()
        let first = try model(hub, drafts, outbox)
        await first.load()
        first.summary = "Keep this offline summary"
        first.saveSummary()
        await hub.disconnect()
        await first.load()
        #expect(!first.canSend && !first.isCurrent)
        #expect(first.drafts.count == 1 && first.summary == "Keep this offline summary")
        let reopened = try model(hub, drafts, outbox)
        await reopened.load()
        #expect(!reopened.canSend && reopened.summary == "Keep this offline summary")
        await hub.reconnect(revision: 4)
        await reopened.load()
        #expect(!reopened.canSend && reopened.error?.contains("newer revision") == true)
        await reopened.send()
        #expect(await hub.reviewRequests == 0)
        #expect(await hub.uploads.isEmpty)
        #expect(drafts.values.count == 1)
        await hub.reconnect(revision: 3)
        await reopened.load()
        #expect(reopened.canSend)
        await reopened.send()
        #expect(reopened.sent && drafts.values.isEmpty)
        #expect(await hub.summaries == ["Keep this offline summary"])
    }

    @Test func sendsBothSourceSidesExactUnicodeSummaryAndPinnedRevision() async throws {
        let hub = FeedbackHub()
        let drafts = LocalDrafts()
        let old = draft(.old, start: 2, end: 4)
        let new = draft(.new, start: 7, end: 7)
        drafts.values = [old, new]
        let model = try model(hub, drafts, MemoryOutbox())
        await model.load()
        model.summary = "  Check the receipt é 🐙\nSecond paragraph  "
        model.saveSummary()
        await model.send()
        #expect(model.sent)
        #expect(drafts.values.isEmpty)
        let uploads = await hub.uploads
        #expect(uploads.count == 2)
        #expect(uploads[0]["revision"]?.value as? Int == 3)
        #expect(uploads[0]["path"]?.value as? String == "src/total + tax #é.swift")
        #expect(uploads[0]["side"]?.value as? String == "old")
        #expect(uploads[0]["start_line"]?.value as? Int == 2)
        #expect(uploads[0]["line"]?.value as? Int == 4)
        #expect(uploads[1]["side"]?.value as? String == "new")
        #expect(uploads[1]["body"]?.value as? String == new.body)
        #expect(await hub.summaries == ["Check the receipt é 🐙\nSecond paragraph"])
        #expect(await hub.verdicts == ["comment"])
        await model.send()
        #expect(await hub.summaries.count == 1)
    }

    @Test func savedAnswerIsPreviewedAndRecoveredFromLostReviewResponse() async throws {
        let hub = FeedbackHub()
        await hub.addHubReply()
        let drafts = LocalDrafts()
        let outbox = MemoryOutbox()
        let model = try model(hub, drafts, outbox)
        await model.load()
        #expect(model.hubReplies.map(\.comment) == ["c_question"])
        #expect(model.hubReplies.map(\.body) == ["Receipt client é 🐙"])
        #expect(model.canSend)
        await hub.loseReviewResponse()
        await model.send()
        #expect(model.sent)
        #expect(await hub.reviewRequests == 1)
    }

    @Test func partialUploadLostResponseReopensWithoutDuplicatingComments() async throws {
        let hub = FeedbackHub()
        await hub.loseUploadResponse()
        let drafts = LocalDrafts()
        drafts.values = [draft(.old), draft(.new)]
        let outbox = MemoryOutbox()
        let first = try model(hub, drafts, outbox)
        await first.load()
        first.summary = "Keep both sides"
        await first.send()
        #expect(!first.sent && first.frozen)
        #expect(drafts.values.count == 2)
        let restored = try model(hub, drafts, outbox)
        await restored.load()
        #expect(restored.summary == "Keep both sides")
        await restored.send()
        #expect(restored.sent)
        #expect(await hub.uploads.count == 2)
        #expect(await hub.summaries == ["Keep both sides"])
    }

    @Test func lostReviewResponseUsesReceiptAndCleanupCanRetryAfterRelaunch() async throws {
        let hub = FeedbackHub()
        await hub.loseReviewResponse()
        let drafts = LocalDrafts()
        drafts.values = [draft(.new)]
        drafts.failRemoval = true
        let outbox = MemoryOutbox()
        let first = try model(hub, drafts, outbox)
        await first.load()
        first.summary = "Receipt survives timeout"
        await first.send()
        #expect(!first.sent)
        #expect(outbox.record.attempt?.confirmed == true)
        drafts.failRemoval = false
        let restored = try model(hub, drafts, outbox)
        await restored.load()
        #expect(restored.sent)
        #expect(drafts.values.isEmpty)
        #expect(await hub.summaries.count == 1)
    }

    @Test func summaryOnlyReceiptAndUncertainDeliveryNeverBlindlyResend() async throws {
        let hub = FeedbackHub()
        await hub.rejectConnectionBeforeReview()
        let outbox = MemoryOutbox()
        let model = try model(hub, LocalDrafts(), outbox)
        await model.load()
        #expect(!model.canSend)
        model.summary = "Summary only"
        await model.send()
        #expect(model.uncertain && !model.sent)
        await model.send()
        #expect(await hub.reviewRequests == 1)
        #expect(model.error?.contains("not confirmed") == true)
        await model.send(allowResend: true)
        #expect(model.sent)
        #expect(await hub.summaries == ["Summary only"])
    }

    @Test func staleClosedAndChangedDraftsDoNotWrite() async throws {
        for state in ["merged", "merging", "abandoned"] {
            let hub = FeedbackHub(state: state)
            let local = LocalDrafts()
            local.values = [draft(.new)]
            let model = try model(hub, local, MemoryOutbox())
            await model.load()
            await model.send()
            #expect(!model.sent)
            #expect(await hub.uploads.isEmpty)
        }
        let hub = FeedbackHub(revision: 4)
        let model = try model(hub, LocalDrafts(), MemoryOutbox())
        await model.load()
        model.summary = "Old revision"
        await model.send()
        #expect(await hub.reviewRequests == 0)
        #expect(model.error?.contains("newer revision") == true)
    }

    @Test func sharedHubDraftChangesRequireReadingNewPreview() async throws {
        let hub = FeedbackHub()
        let model = try model(hub, LocalDrafts(), MemoryOutbox())
        await model.load()
        model.summary = "Summary"
        await hub.addHubDraft()
        await model.send()
        #expect(!model.sent)
        #expect(model.hubDrafts.count == 1)
        #expect(await hub.reviewRequests == 0)
        await model.send()
        #expect(model.sent)
    }

    @Test func rejectedAttemptCanBeEditedWithoutDuplicatingUploadedCopies() async throws {
        let hub = FeedbackHub()
        await hub.loseUploadResponse()
        let drafts = LocalDrafts()
        drafts.values = [draft(.new)]
        let outbox = MemoryOutbox()
        let model = try model(hub, drafts, outbox)
        await model.load()
        model.summary = "Original summary"
        await model.send()
        #expect(model.frozen)
        await model.editFeedback()
        #expect(model.isCurrent && model.canSend)
        #expect(!model.frozen)
        #expect(drafts.values.count == 1)
        #expect(await hub.drafts.isEmpty)
        model.summary = "Corrected summary"
        await model.send()
        #expect(model.sent)
        #expect(await hub.summaries == ["Corrected summary"])
        #expect(await hub.sent.count == 1)
    }

    @Test func definiteRejectionCanRecoverAfterSharedHubDraftIsResolved() async throws {
        let hub = FeedbackHub()
        await hub.addHubDraft()
        await hub.refuseReview()
        let drafts = LocalDrafts()
        drafts.values = [draft(.new)]
        let model = try model(hub, drafts, MemoryOutbox())
        await model.load()
        model.summary = "Review the receipt"
        await model.send()
        #expect(model.frozen && !model.uncertain && !model.sent)
        await hub.removeSharedDraft()
        await model.editFeedback()
        #expect(model.isCurrent && model.canSend)
        #expect(!model.frozen)
        await model.send()
        #expect(model.sent)
        #expect(await hub.sent.count == 1)
        #expect(await hub.summaries == ["Review the receipt"])
    }

    @Test func summaryWriteFailureCanRecoverAndSuccessfulSendLeavesSheetDismissible() async throws {
        let hub = FeedbackHub()
        let outbox = MemoryOutbox()
        let model = try model(hub, LocalDrafts(), outbox)
        await model.load()
        outbox.fail = true
        model.summary = "Retain this summary"
        model.saveSummary()
        #expect(!model.summarySaved && !model.canSend)
        outbox.fail = false
        model.saveSummary()
        #expect(model.summarySaved && model.canSend)
        await model.send()
        #expect(model.sent && model.summarySaved && model.error == nil)
    }

    @Test func swiftDataOutboxRetainsSummaryAndScopesByHubItemRevision() throws {
        let directory = URL.temporaryDirectory.appending(path: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appending(path: "feedback.store")
        let store = try SwiftDataFeedbackOutbox(configuration: .init(url: url))
        var record = FeedbackRecord()
        record.summary = "Unfinished summary é 🐙"
        try store.save(record, scope: scope())
        #expect(
            try SwiftDataFeedbackOutbox(configuration: .init(url: url)).load(scope: scope()).summary
                == record.summary)
        #expect(try store.load(scope: scope(hub: "http://MY-MAC:80/")).summary == record.summary)
        #expect(try store.load(scope: scope(revision: 4)).summary.isEmpty)
        #expect(try store.load(scope: scope(item: "app#2")).summary.isEmpty)
        #expect(try store.load(scope: scope(hub: "http://another-mac")).summary.isEmpty)
        let pending = draft(.old, start: 12, end: 14)
        record.attempt = FeedbackAttempt(
            drafts: [pending], summary: record.summary, cursor: 42, hubDrafts: [], hubReplies: [])
        record.attempt?.uploaded[pending.id] = "c_17"
        record.attempt?.reviewStarted = true
        try store.save(record, scope: scope())
        let restored = try SwiftDataFeedbackOutbox(configuration: .init(url: url)).load(scope: scope())
        #expect(restored.attempt?.drafts == [pending])
        #expect(restored.attempt?.uploaded[pending.id] == "c_17")
        #expect(restored.attempt?.reviewStarted == true)
        #expect(restored.attempt?.cursor == 42)

    }

    func model(_ hub: FeedbackHub, _ drafts: LocalDrafts, _ outbox: MemoryOutbox) throws
        -> FeedbackViewModel
    {
        try FeedbackViewModel(
            scope: scope(), drafts: drafts, outbox: outbox, service: .init(transport: hub))
    }
    func scope(hub: String = "http://my-mac", item: String = "app#1", revision: UInt32 = 3)
        -> CommentDraftScope
    {
        .init(hub: URL(string: hub)!, item: item, name: "app#44", revision: revision)
    }
    func draft(_ side: SourceAnchor.Side, start: UInt32 = 3, end: UInt32 = 3) -> FeedbackDraft {
        .init(
            id: UUID(),
            location: .init(
                path: "src/total + tax #é.swift",
                anchor: .init(
                    side: side, startLine: start, endLine: end, quote: "let total = subtotal é 🐙")),
            body: "Keep Unicode e\u{301} 🐙\nand line breaks")
    }
}

@MainActor final class LocalDrafts: FeedbackDrafts {
    var values: [FeedbackDraft] = []
    var failRemoval = false
    func load(scope: CommentDraftScope) throws -> [FeedbackDraft] { values }
    func removeSent(_ drafts: [FeedbackDraft], scope: CommentDraftScope) throws {
        if failRemoval { throw URLError(.cannotWriteToFile) }
        values.removeAll { drafts.contains($0) }
    }
}

@MainActor final class MemoryOutbox: FeedbackOutbox {
    var record = FeedbackRecord()
    var fail = false
    func load(scope: CommentDraftScope) -> FeedbackRecord { record }
    func save(_ record: FeedbackRecord, scope: CommentDraftScope) throws {
        if fail { throw URLError(.cannotWriteToFile) }
        self.record = record
    }
}

actor FeedbackHub: HTTPTransport {
    var revision: Int
    var offline = false
    let state: String
    var drafts: [[String: SendableJSON]] = []
    var sent: [[String: SendableJSON]] = []
    var replies: [[String: SendableJSON]] = []
    var sentReplies: [[String: SendableJSON]] = []
    var uploads: [[String: SendableJSON]] = []
    var summaries: [String] = []
    var verdicts: [String] = []
    var reviewRequests = 0
    var uploadLost = false
    var reviewLost = false
    var connectionLost = false
    var refusal = false
    init(revision: Int = 3, state: String = "needs_review") {
        self.revision = revision
        self.state = state
    }
    func addHubReply() {
        replies.append([
            "id": .number(17), "comment": .string("c_question"), "body": .string("Receipt client é 🐙"),
        ])
    }
    func disconnect() { offline = true }
    func reconnect(revision: Int) {
        offline = false
        self.revision = revision
    }
    func refuseReview() { refusal = true }
    func removeSharedDraft() { drafts.removeAll { $0["id"]?.value as? String == "c_external" } }
    func loseUploadResponse() { uploadLost = true }
    func loseReviewResponse() { reviewLost = true }
    func rejectConnectionBeforeReview() { connectionLost = true }
    func addHubDraft() {
        drafts.append([
            "id": .string("c_external"), "revision": .number(3), "path": .string("policy.txt"),
            "side": .string("new"), "start_line": .number(1), "line": .number(1),
            "body": .string("Hub feedback"),
        ])
    }
    func get(_ url: URL) throws -> Data {
        if offline { throw URLError(.notConnectedToInternet) }
        #expect(url.fragment == nil)
        if url.path == "/api/events" {
            var events: [[String: Any]] = summaries.enumerated().map { index, summary in
                [
                    "item": "app#1", "revision": revision, "type": "review.submitted",
                    "verdict": verdicts[index],
                    "summary": summary, "comments": sent.map { $0.mapValues(\.value) },
                    "replies": sentReplies.map { $0.mapValues(\.value) },
                ]
            }
            if verdicts.contains("approved") {
                events.append([
                    "item": "app#1", "revision": revision, "type": "item.merged", "commit": "abc123",
                ])
            }
            return try json(["cursor": events.count, "events": url.query == nil ? [] : events])
        }
        #expect(url.path == "/api/items/app#1")
        return try json([
            "item": ["id": "app#1", "revision": ["number": revision], "state": state, "target": "main"],
            "drafts": drafts.map { $0.mapValues(\.value) },
            "draft_replies": replies.map { $0.mapValues(\.value) },
        ])
    }
    func delete(_ url: URL) async throws -> Data {
        drafts.removeAll { $0["id"]?.value as? String == url.lastPathComponent }
        return Data()
    }

    func post(_ url: URL, body: Data) throws -> Data {
        #expect(url.fragment == nil)
        let request = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        if url.path.hasSuffix("/drafts") {
            var comment = request.mapValues { SendableJSON($0) }
            uploads.append(comment)
            comment["id"] = .string("c_\(uploads.count)")
            drafts.append(comment)
            if uploadLost {
                uploadLost = false
                throw URLError(.timedOut)
            }
            return try json(comment.mapValues(\.value))
        }
        #expect(["/api/items/app#1/reviews", "/api/items/app#1/approve"].contains(url.path))
        #expect(request["revision"] as? Int == 3)
        let verdict =
            url.path.hasSuffix("/approve") ? "approved" : try #require(request["verdict"] as? String)
        #expect(["comment", "changes_requested", "approved"].contains(verdict))
        reviewRequests += 1
        if refusal {
            refusal = false
            throw HTTPFailure(
                status: 409, code: "drafts_unplaced", message: "Place or remove the old hub draft")
        }
        if connectionLost {
            connectionLost = false
            throw URLError(.networkConnectionLost)
        }
        let summary = try #require(request["summary"] as? String)
        summaries.append(summary)
        verdicts.append(verdict)
        sent += drafts
        sentReplies += replies
        drafts = []
        replies = []
        if reviewLost {
            reviewLost = false
            throw URLError(.timedOut)
        }
        if verdict == "approved" {
            return try json([
                "item": [
                    "id": "app#1", "revision": ["number": revision], "target": "main", "state": "merged",
                ],
                "outcome": ["kind": "merged", "commit": "abc123"],
            ])
        }
        return try json([
            "revision": revision, "verdict": verdict, "summary": summary,
            "comments": sent.map { $0.mapValues(\.value) },
            "replies": sentReplies.map { $0.mapValues(\.value) },
        ])
    }
    func put(_ url: URL, body: Data) throws -> Data { throw URLError(.unsupportedURL) }
    private func json(_ value: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: value)
    }
}

enum SendableJSON: Sendable {
    case string(String)
    case number(Int)
    init(_ value: Any) {
        if let value = value as? String { self = .string(value) } else { self = .number(value as! Int) }
    }
    var value: Any {
        switch self {
        case .string(let value): value
        case .number(let value): value
        }
    }
}
