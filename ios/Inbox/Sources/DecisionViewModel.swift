import Foundation
import Observation

@MainActor @Observable
final class DecisionViewModel {
    let item: InboxItem
    let decision: InboxTriage.Decision
    private let service: DecisionWebService
    private(set) var busy = false
    private(set) var loaded = false
    private(set) var error: String?
    private(set) var result: String?
    var text = ""
    private var savedText: String?
    var answerSaved: Bool {
        savedText != nil && text.trimmingCharacters(in: .whitespacesAndNewlines) == savedText
    }

    init(item: InboxItem, decision: InboxTriage.Decision, service: DecisionWebService) {
        self.item = item
        self.decision = decision
        self.service = service
    }

    func load() async {
        guard !busy && !loaded else { return }
        busy = true
        defer { busy = false }
        do {
            let snapshot = try await current()
            text = snapshot.draft_replies.last(where: { $0.comment == decision.id })?.body ?? ""
            savedText = text.isEmpty ? nil : text
            loaded = true
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func saveAnswer() async {
        guard decision.kind == "Question", !busy && loaded && result == nil else { return }
        let body = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return }
        busy = true
        defer { busy = false }
        do {
            // Refetch before retrying: a timed-out create may already have saved the draft.
            let snapshot = try await current()
            let reply = snapshot.draft_replies.last(where: { $0.comment == decision.id })
            try await service.saveAnswer(comment: decision.id, reply: reply?.id, body: body)
            savedText = body
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func settle(override: Bool) async {
        guard decision.kind == "Disagreement", let replyID = decision.replyID,
            !busy && loaded && result == nil
        else { return }
        busy = true
        defer { busy = false }
        do {
            let value = try await service.load(item: item.id)
            guard value.item.id == item.id else {
                throw DecisionIssue(message: "The hub returned a different item.")
            }
            let accepted =
                value.comments.first(where: { $0.id == decision.id })?.external?.agreed_reply
                == replyID
            let next = value.replies.first(where: { $0.comment == decision.id && $0.id > replyID })
            let overridden =
                next?.author == "operator" && next?.overrides == true
                && next?.body == text.trimmingCharacters(in: .whitespacesAndNewlines)
            // Check the sent history before revision/state: the recipient may already have acted.
            if (override && overridden) || (!override && accepted) {
                result = override ? "Override sent" : "Rejection accepted"
                error = nil
                return
            }
            try validateItem(value)
            try validateDecision(value)
            try await service.settle(
                comment: decision.id, reply: replyID, override: override,
                note: text.trimmingCharacters(in: .whitespacesAndNewlines))
            result = override ? "Override sent" : "Rejection accepted"
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func feedbackClosed() async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            let value = try await service.load(item: item.id)
            guard value.item.id == item.id else {
                throw DecisionIssue(message: "The hub returned a different item.")
            }
            if let savedText, let question = decision.replyID,
                value.replies.contains(where: {
                    $0.comment == decision.id && $0.id > question && $0.author == "operator"
                        && $0.overrides != true && $0.body == savedText
                })
            {
                result = "Answer sent"
                error = nil
                return
            }
            try validateItem(value)
            try validateDecision(value)
            error = nil
        } catch {
            loaded = false
            self.error =
                "Couldn’t confirm the current decision. Return to the inbox to check delivery. \(error.localizedDescription)"
        }
    }

    private func current() async throws -> DecisionSnapshot {
        let value = try await service.load(item: item.id)
        try validateItem(value)
        try validateDecision(value)
        return value
    }

    private func validateItem(_ value: DecisionSnapshot) throws {
        guard value.item.id == item.id && value.item.revision.number == item.revision else {
            throw DecisionIssue(
                message:
                    "This item changed. Your text is retained. Return to the inbox and reopen its current revision."
            )
        }
        guard ["needs_review", "changes_requested"].contains(value.item.state) else {
            throw DecisionIssue(
                message: "This item no longer takes feedback. Your text is retained.")
        }
    }

    private func validateDecision(_ value: DecisionSnapshot) throws {
        guard let latest = value.decisions.first(where: { $0.id == decision.id }),
            latest.kind == decision.kind, latest.replyID == decision.replyID
        else {
            throw DecisionIssue(
                message:
                    "This decision changed or was resolved elsewhere. Refresh the inbox before continuing."
            )
        }
    }
}
