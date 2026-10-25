import Foundation
import Testing

@testable import ReviewSubmission

extension FeedbackTests {
    @Test func requestsChangesWithActionableSummaryAndExactVerdict() async throws {
        let hub = FeedbackHub()
        let outbox = MemoryOutbox()
        let model = try model(hub, LocalDrafts(), outbox)
        await model.load()
        model.verdict = .changesRequested
        #expect(!model.canSend)
        await model.send()
        #expect(await hub.reviewRequests == 0)
        model.summary = "  Include delivery in the receipt. é 🐙  "
        model.saveSummary()
        let restored = try self.model(hub, LocalDrafts(), outbox)
        #expect(restored.verdict == .changesRequested)
        await restored.load()
        await restored.send()
        #expect(restored.sent && restored.sentVerdict == .changesRequested)
        #expect(await hub.verdicts == ["changes_requested"])
        #expect(await hub.summaries == ["Include delivery in the receipt. é 🐙"])
    }

    @Test func partialRequestRetainsVerdictAndCommentsAcrossRelaunch() async throws {
        let hub = FeedbackHub()
        await hub.loseUploadResponse()
        let drafts = LocalDrafts()
        drafts.values = [draft(.old), draft(.new)]
        let outbox = MemoryOutbox()
        let first = try model(hub, drafts, outbox)
        await first.load()
        first.verdict = .changesRequested
        await first.send()
        #expect(first.frozen)
        first.verdict = .comment
        #expect(first.verdict == .changesRequested)
        let restored = try model(hub, drafts, outbox)
        await restored.load()
        await restored.send()
        #expect(restored.sent && restored.sentVerdict == .changesRequested)
        #expect(await hub.verdicts == ["changes_requested"])
        #expect(await hub.uploads.count == 2)
        #expect(drafts.values.isEmpty)
    }

    @Test func lostRequestReceiptReconcilesTheSelectedVerdict() async throws {
        let hub = FeedbackHub()
        await hub.loseReviewResponse()
        let model = try model(hub, LocalDrafts(), MemoryOutbox())
        await model.load()
        model.verdict = .changesRequested
        model.summary = "Return the receipt total to the charged amount."
        await model.send()
        #expect(model.sent && model.sentVerdict == .changesRequested)
        #expect(await hub.reviewRequests == 1)
    }

    @Test func differentVerdictCannotConfirmAnUncertainRequest() async throws {
        let hub = FeedbackHub()
        await hub.rejectConnectionBeforeReview()
        let outbox = MemoryOutbox()
        let model = try model(hub, LocalDrafts(), outbox)
        await model.load()
        model.verdict = .changesRequested
        model.summary = "Same text"
        await model.send()
        // Another client sent a comment with identical text after the saved cursor.
        let body = Data(#"{"revision":3,"verdict":"comment","summary":"Same text"}"#.utf8)
        _ = try await hub.post(URL(string: "http://my-mac/api/items/app%231/reviews")!, body: body)
        let restored = try self.model(hub, LocalDrafts(), outbox)
        await restored.load()
        #expect(!restored.sent && restored.uncertain)
        await restored.send()
        #expect(!restored.sent)
        #expect(await hub.reviewRequests == 2)
    }

    @Test func editingAnAttemptKeepsOutcomeAndStaleRequestsDoNotUpload() async throws {
        let hub = FeedbackHub()
        await hub.loseUploadResponse()
        let local = LocalDrafts()
        local.values = [draft(.new)]
        let model = try model(hub, local, MemoryOutbox())
        await model.load()
        model.verdict = .changesRequested
        await model.send()
        await model.editFeedback()
        #expect(!model.frozen && model.verdict == .changesRequested)
        let stale = FeedbackHub(revision: 4)
        let old = try self.model(stale, local, MemoryOutbox())
        await old.load()
        old.verdict = .changesRequested
        await old.send()
        #expect(!old.sent && local.values.count == 1)
        #expect(await stale.uploads.isEmpty)
        #expect(await stale.reviewRequests == 0)
    }

    @Test func legacyOutboxDefaultsToCommentAndNewVerdictSurvivesEncoding() throws {
        let legacy = Data(
            #"{"summary":"Before outcome selection","attempt":{"drafts":[],"summary":"Retained feedback","cursor":42,"hubDrafts":[],"hubReplies":[],"uploaded":[],"reviewStarted":false,"confirmed":false}}"#
                .utf8)
        var record = try JSONDecoder().decode(FeedbackRecord.self, from: legacy)
        #expect(record.verdict == nil && record.attempt?.verdict == nil)
        record.verdict = .changesRequested
        record.attempt?.verdict = .changesRequested
        let restored = try JSONDecoder().decode(
            FeedbackRecord.self, from: JSONEncoder().encode(record))
        #expect(
            restored.verdict == .changesRequested && restored.attempt?.verdict == .changesRequested)
    }
}
