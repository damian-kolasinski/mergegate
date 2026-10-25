import ConversationInterface
import Foundation
import Networking
import Observation

@MainActor @Observable
final class FeedbackViewModel {
    let scope: CommentDraftScope
    private let draftsStore: any FeedbackDrafts
    private let outbox: any FeedbackOutbox
    private let service: FeedbackWebService
    private var record: FeedbackRecord
    private(set) var isCurrent = false
    private(set) var snapshot: FeedbackSnapshot?
    private(set) var drafts: [FeedbackDraft] = []
    private(set) var busy = false
    private(set) var sent = false
    private(set) var completedTarget: String?
    private(set) var approval: ApprovalOutcome?
    var approvalTarget: String? { record.attempt?.target ?? snapshot?.item.target ?? completedTarget }
    private(set) var error: String?
    private(set) var summarySaved = true
    var summary: String
    private(set) var sentVerdict: FeedbackVerdict = .comment
    var verdict: FeedbackVerdict {
        get { record.attempt?.verdict ?? record.verdict ?? .comment }
        set {
            guard !frozen && !busy else { return }
            record.verdict = newValue
            saveSummary()
        }
    }
    var frozen: Bool { record.attempt != nil }
    var uncertain: Bool { record.attempt?.reviewStarted == true }
    var canSend: Bool {
        !busy && !sent && summarySaved && snapshot != nil && isCurrent
            && (verdict == .approved || frozen
                || !summary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                || !drafts.isEmpty || !hubDrafts.isEmpty || !hubReplies.isEmpty)
    }
    var hubDrafts: [FeedbackComment] { record.attempt?.hubDrafts ?? snapshot?.drafts ?? [] }
    var hubReplies: [FeedbackReply] { record.attempt?.hubReplies ?? snapshot?.draft_replies ?? [] }

    init(
        scope: CommentDraftScope, drafts: any FeedbackDrafts, outbox: any FeedbackOutbox,
        service: FeedbackWebService
    ) throws {
        self.scope = scope
        draftsStore = drafts
        self.outbox = outbox
        self.service = service
        let saved = try outbox.load(scope: scope)
        record = saved
        summary = saved.attempt?.summary ?? saved.summary
    }

    func saveSummary() {
        guard !frozen else { return }
        record.summary = summary
        summarySaved = false
        do {
            try outbox.save(record, scope: scope)
            summarySaved = true
            error = nil
        } catch {
            self.error = "Couldn’t save the summary on this iPhone. \(error.localizedDescription)"
        }
    }

    func load() async {
        guard !busy else { return }
        busy = true
        isCurrent = false
        defer { busy = false }
        do {
            drafts =
                try record.attempt?.drafts
                ?? draftsStore.load(scope: scope).filter {
                    !$0.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                }
            if record.attempt?.confirmed == true {
                try finish()
                return
            }
            if try await reconcile() { return }
            let current = try await service.load(scope: scope)
            try validate(current)
            snapshot = current
            isCurrent = true
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func editFeedback() async {
        guard !busy, let attempt = record.attempt, !attempt.reviewStarted else { return }
        busy = true
        defer { busy = false }
        do {
            let current = try await service.load(scope: scope)
            var available = current.drafts
            let baseline = Set(attempt.hubDrafts.map(\.id))
            for draft in attempt.drafts {
                if let copy = available.first(where: {
                    !baseline.contains($0.id) && $0.matches(draft, revision: scope.revision)
                        && (attempt.uploaded[draft.id] == nil || attempt.uploaded[draft.id] == $0.id)
                }) {
                    try await service.delete(copy.id, scope: scope)
                    available.removeAll { $0.id == copy.id }
                }
            }
            let editable = FeedbackRecord(summary: attempt.summary, verdict: attempt.verdict)
            try outbox.save(editable, scope: scope)
            record = editable
            summary = editable.summary
            drafts = try draftsStore.load(scope: scope).filter {
                !$0.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            }
            let refreshed = try await service.load(scope: scope)
            try validate(refreshed)
            snapshot = refreshed
            isCurrent = true
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func send(allowResend: Bool = false, confirmedTarget: String? = nil) async {
        guard !busy && !sent else { return }
        busy = true
        defer { busy = false }
        do {
            if record.attempt?.confirmed == true {
                try finish()
                return
            }
            if try await reconcile() { return }
            if uncertain && !allowResend {
                throw FeedbackIssue(
                    message:
                        "Delivery is not confirmed yet. Your feedback is retained. Check delivery again before choosing Send again."
                )
            }
            let current = try await service.load(scope: scope)
            try validate(current)
            if verdict == .approved {
                guard let confirmedTarget, !confirmedTarget.isEmpty,
                    confirmedTarget == current.item.target,
                    record.attempt?.target == nil || record.attempt?.target == confirmedTarget
                else {
                    throw FeedbackIssue(message: "Confirm the revision and target before approving.")
                }
            }
            if record.attempt == nil {
                guard let snapshot, snapshot.drafts == current.drafts,
                    snapshot.draft_replies == current.draft_replies
                else {
                    self.snapshot = current
                    throw FeedbackIssue(
                        message: "Hub drafts changed. Read the updated feedback, then send again.")
                }
                let local = try draftsStore.load(scope: scope).filter {
                    !$0.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                }
                guard local == drafts else {
                    drafts = local
                    throw FeedbackIssue(
                        message: "Local drafts changed. Read the updated feedback, then send again.")
                }
                guard canSendContent(current) else {
                    throw FeedbackIssue(message: "Add a summary or comment before sending.")
                }
                let log = try await service.log(scope: scope)
                record.attempt = FeedbackAttempt(
                    drafts: drafts, verdict: verdict,
                    summary: summary.trimmingCharacters(in: .whitespacesAndNewlines),
                    cursor: log.cursor, hubDrafts: current.drafts, hubReplies: current.draft_replies,
                    target: verdict == .approved ? confirmedTarget : nil)
                try save()
            }
            var attempt = record.attempt!
            var available = current.drafts
            let baseline = Set(attempt.hubDrafts.map(\.id))
            for draft in attempt.drafts {
                let match = available.first {
                    !baseline.contains($0.id) && $0.matches(draft, revision: scope.revision)
                        && (attempt.uploaded[draft.id] == nil || attempt.uploaded[draft.id] == $0.id)
                }
                let comment: FeedbackComment
                if let match {
                    comment = match
                } else {
                    guard attempt.uploaded[draft.id] == nil else {
                        throw FeedbackIssue(
                            message:
                                "An uploaded draft changed or was sent elsewhere. Check conversations before continuing."
                        )
                    }
                    comment = try await service.upload(draft, scope: scope)
                }
                attempt.uploaded[draft.id] = comment.id
                available.removeAll { $0.id == comment.id }
                record.attempt = attempt
                try save()
            }
            let ready = try await service.load(scope: scope)
            try validate(ready)
            let expected = Set(attempt.hubDrafts.map(\.id) + Array(attempt.uploaded.values))
            guard Set(ready.drafts.map(\.id)) == expected,
                attempt.hubDrafts.allSatisfy({ ready.drafts.contains($0) }),
                attempt.drafts.allSatisfy({ draft in
                    ready.drafts.contains {
                        $0.id == attempt.uploaded[draft.id] && $0.matches(draft, revision: scope.revision)
                    }
                }), ready.draft_replies == attempt.hubReplies
            else {
                throw FeedbackIssue(
                    message:
                        "Hub drafts changed during delivery. Your feedback is retained; check the hub before continuing."
                )
            }
            if attempt.verdict == .approved && ready.item.target != attempt.target {
                throw FeedbackIssue(
                    message: "The merge target changed. Reopen the completion sheet and confirm it again.")
            }
            attempt.reviewStarted = true
            record.attempt = attempt
            try save()
            do {
                if attempt.verdict == .approved {
                    record.attempt?.approval = try await service.approve(
                        scope: scope, summary: attempt.summary, target: attempt.target!)
                    try save()
                    guard try await reconcile() else {
                        throw FeedbackIssue(
                            message:
                                "Approval receipt is not available yet. Check delivery before continuing.")
                    }
                    return
                }
                let receipt = try await service.submit(
                    scope: scope, summary: attempt.summary, verdict: attempt.verdict ?? .comment)
                guard
                    receipt.revision == scope.revision
                        && receipt.verdict == (attempt.verdict ?? .comment).rawValue
                        && receipt.summary == attempt.summary && Set(receipt.comments.map(\.id)) == expected
                        && receipt.replies == attempt.hubReplies
                else {
                    throw FeedbackIssue(
                        message:
                            "The hub returned a different review receipt. Check delivery before sending again."
                    )
                }
            } catch let failure as HTTPFailure where (400..<500).contains(failure.status) {
                record.attempt?.reviewStarted = false
                try save()
                throw failure
            }
            record.attempt?.confirmed = true
            try save()
            try finish()
        } catch {
            isCurrent = false
            self.error = error.localizedDescription
            if uncertain {
                do { if try await reconcile() { return } } catch {
                    self.error = "Delivery is unconfirmed. \(error.localizedDescription)"
                }
            }
        }
    }

    private func reconcile() async throws -> Bool {
        guard let attempt = record.attempt, attempt.reviewStarted else { return false }
        let log = try await service.log(scope: scope, since: attempt.cursor)
        let expected = Set(attempt.hubDrafts.map(\.id) + Array(attempt.uploaded.values))
        if log.events.contains(where: {
            $0.item == scope.item && $0.revision == scope.revision && $0.type == "review.submitted"
                && $0.verdict == (attempt.verdict ?? .comment).rawValue && $0.summary == attempt.summary
                && Set(($0.comments ?? []).map(\.id)) == expected
                && ($0.replies ?? []) == attempt.hubReplies
        }) {
            if attempt.verdict == .approved && attempt.approval == nil {
                record.attempt?.approval = ApprovalOutcome.recover(
                    events: log.events, item: scope.item, revision: scope.revision)
            }
            record.attempt?.confirmed = true
            try save()
            try finish()
            return true
        }
        return false
    }

    private func finish() throws {
        try draftsStore.removeSent(record.attempt!.drafts, scope: scope)
        sentVerdict = record.attempt?.verdict ?? .comment
        approval = record.attempt?.approval
        completedTarget = record.attempt?.target
        let cleared = FeedbackRecord()
        try outbox.save(cleared, scope: scope)
        record = cleared
        summary = ""
        sent = true
        summarySaved = true
        error = nil
    }

    private func save() throws {
        try outbox.save(record, scope: scope)
        summarySaved = true
    }

    private func validate(_ snapshot: FeedbackSnapshot) throws {
        guard snapshot.item.revision.number == scope.revision else {
            throw FeedbackIssue(
                message:
                    "A newer revision is available. Your r\(scope.revision) drafts are retained. Return to the inbox and review the current revision."
            )
        }
        guard ["needs_review", "changes_requested"].contains(snapshot.item.state) else {
            throw FeedbackIssue(
                message:
                    "This item is \(snapshot.item.state.replacingOccurrences(of: "_", with: " ")). Your feedback is retained."
            )
        }
    }

    private func canSendContent(_ value: FeedbackSnapshot) -> Bool {
        verdict == .approved || !summary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            || !drafts.isEmpty
            || !value.drafts.isEmpty || !value.draft_replies.isEmpty
    }
}
