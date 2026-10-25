import ConversationInterface
import Foundation
import Networking

struct FeedbackWebService: Sendable {
    let transport: any HTTPTransport

    func load(scope: CommentDraftScope) async throws -> FeedbackSnapshot {
        let value = try await JSONDecoder().decode(
            FeedbackSnapshot.self, from: transport.get(endpoint(scope)))
        guard value.item.id == scope.item else {
            throw FeedbackIssue(message: "The hub returned a different item.")
        }
        return value
    }

    func log(scope: CommentDraftScope, since: UInt64? = nil) async throws -> FeedbackLog {
        var url = URLComponents(
            url: scope.hub.appending(path: "api/events"), resolvingAgainstBaseURL: false)!
        if let since { url.queryItems = [.init(name: "since", value: String(since))] }
        // The operator log reads receipts without acknowledging feedback or waking the workflow.
        return try await JSONDecoder().decode(FeedbackLog.self, from: transport.get(url.url!))
    }

    func upload(_ draft: FeedbackDraft, scope: CommentDraftScope) async throws -> FeedbackComment {
        struct Body: Encodable {
            let revision: UInt32
            let path: String
            let side: String
            let start_line: UInt32
            let line: UInt32
            let body: String
        }
        let body = Body(
            revision: scope.revision, path: draft.location.path,
            side: draft.location.anchor.side.rawValue, start_line: draft.location.anchor.startLine,
            line: draft.location.anchor.endLine, body: draft.body)
        let comment = try await JSONDecoder().decode(
            FeedbackComment.self,
            from: transport.post(
                endpoint(scope).appending(path: "drafts"), body: JSONEncoder().encode(body)))
        guard comment.matches(draft, revision: scope.revision) else {
            throw FeedbackIssue(
                message: "The hub returned a different comment. Your local draft is retained.")
        }
        return comment
    }

    func delete(_ id: String, scope: CommentDraftScope) async throws {
        _ = try await transport.delete(endpoint(scope).appending(path: "drafts").appending(component: id))
    }

    func submit(scope: CommentDraftScope, summary: String, verdict: FeedbackVerdict) async throws
        -> FeedbackReceipt
    {
        struct Body: Encodable {
            let revision: UInt32
            let verdict: FeedbackVerdict
            let summary: String
        }
        return try await JSONDecoder().decode(
            FeedbackReceipt.self,
            from: transport.post(
                endpoint(scope).appending(path: "reviews"),
                body: JSONEncoder().encode(Body(revision: scope.revision, verdict: verdict, summary: summary))))
    }

    private func endpoint(_ scope: CommentDraftScope) -> URL {
        scope.hub.appending(path: "api/items").appending(component: scope.item)
    }
}
