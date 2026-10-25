import Foundation
import Observation

@MainActor @Observable
final class ConversationEditorModel {
    enum Kind {
        case reply(comment: String, id: UInt64?)
        case comment(ThreadComment)
    }
    let kind: Kind
    let service: ConversationEditingService
    var text: String
    var path = ""
    var side = "new"
    var start = ""
    var end = ""
    var moveAnchor = false
    private(set) var busy = false
    private(set) var saved = false
    var error: String?
    private var sentBeforeAttempt: Set<UInt64>?
    private var recoveredReplyID: UInt64?
    private var uncertain = false
    private var attemptedBody: String?
    private var draftsBeforeAttempt: Set<UInt64> = []

    init(kind: Kind, text: String, service: ConversationEditingService) {
        self.kind = kind
        self.text = text
        self.service = service
        if case .comment(let comment) = kind {
            path = comment.path ?? ""
            side = comment.side ?? "new"
            start = comment.start_line.map(String.init) ?? ""
            end = comment.line.map(String.init) ?? ""
        }
    }
    func save() async {
        guard !busy, !saved else { return }
        let body = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return }
        busy = true
        defer { busy = false }
        do {
            let current = try await service.current()
            switch kind {
            case .reply(let comment, let id):
                guard let target = current.comments.first(where: { $0.id == comment }),
                    target.external?.vote != "down", target.external?.summary != true
                else {
                    throw ConversationEditIssue(message: "This comment no longer accepts replies.")
                }
                if uncertain, let sentBeforeAttempt, let attemptedBody,
                    current.replies.contains(where: {
                        $0.comment == comment && $0.author == "operator" && $0.body == attemptedBody
                            && !sentBeforeAttempt.contains($0.id)
                    })
                {
                    guard body == attemptedBody else {
                        throw ConversationEditIssue(
                            message:
                                "The previous text was already sent. Your edited text is retained here. Close this editor and read the delivered reply before adding another."
                        )
                    }
                    saved = true
                    error = nil
                    return
                }
                if let id,
                    !(current.draft_replies ?? []).contains(where: {
                        $0.id == id && $0.comment == comment
                    })
                {
                    throw ConversationEditIssue(
                        message:
                            "This draft reply was already sent or deleted. Refresh the conversation before continuing."
                    )
                }
                let drafts = current.draft_replies ?? []
                let recovered =
                    uncertain
                    ? drafts.first {
                        $0.comment == comment && $0.body == attemptedBody
                            && !draftsBeforeAttempt.contains($0.id)
                    } : nil
                let existing = drafts.first { $0.comment == comment && $0.body == body }
                let targetID = id ?? recoveredReplyID ?? recovered?.id
                if let targetID,
                    !drafts.contains(where: { $0.id == targetID && $0.comment == comment })
                {
                    throw ConversationEditIssue(
                        message:
                            "The recovered draft was sent or deleted. Refresh the conversation before continuing."
                    )
                }
                recoveredReplyID = targetID
                sentBeforeAttempt = Set(current.replies.map(\.id))
                draftsBeforeAttempt = Set(drafts.map(\.id))
                attemptedBody = body
                uncertain = true
                if existing == nil || targetID != nil {
                    recoveredReplyID = try await service.saveReply(
                        comment: comment, reply: targetID, body: body
                    ).id
                }
            case .comment(let comment):
                guard (current.drafts ?? []).contains(where: { $0.id == comment.id }) else {
                    throw ConversationEditIssue(
                        message:
                            "This draft comment was already sent or deleted. Refresh the conversation."
                    )
                }
                if moveAnchor {
                    guard let start = UInt32(start), let end = UInt32(end), start > 0, end >= start,
                        !path.isEmpty
                    else {
                        throw ConversationEditIssue(
                            message: "Enter a file path and a valid source-line range.")
                    }
                    try await service.moveComment(
                        id: comment.id, path: path, side: side, start: start, end: end)
                }
                try await service.saveComment(id: comment.id, body: body)
            }
            saved = true
            error = nil
        } catch { self.error = error.localizedDescription }
    }
}
