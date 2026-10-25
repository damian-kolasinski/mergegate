import ConversationInterface
import Foundation

struct ApprovalOutcome: Codable, Sendable {
    let kind: String
    var commit: String?
    var reason: String?
    var parent: String?
    var paths: [String]?

    static func recover(events: [FeedbackLog.Event], item: String, revision: UInt32) -> Self {
        let event = events.last {
            $0.item == item && $0.revision == revision
                && ["item.merged", "merge.held", "merge.conflict"].contains($0.type)
        }
        switch event?.type {
        case "item.merged" where event?.commit?.isEmpty == false:
            return .init(kind: "merged", commit: event?.commit)
        case "merge.held": return .init(kind: "held", reason: event?.reason)
        case "merge.conflict": return .init(kind: "conflict", paths: event?.paths)
        default: return .init(kind: "recorded")
        }
    }

    var title: String {
        switch kind {
        case "merged": "Merged"
        case "held": "Merge held"
        case "conflict": "Rebase needed"
        case "waiting": "Waiting for parent"
        default: "Approval recorded"
        }
    }

    var detail: String {
        switch kind {
        case "merged": "Target commit: \(commit ?? "unavailable")"
        case "held": reason ?? "The hub retained this merge. Check the Merging inbox."
        case "conflict":
            "The target has conflicting changes. Check the inbox for the required rebase.\n\((paths ?? []).joined(separator: "\n"))"
        case "waiting": "This item will land after \(parent ?? "its parent"). Follow it in Merging."
        default: "The hub received your approval. Return to the inbox to check the merge result."
        }
    }
}

extension FeedbackWebService {
    func approve(scope: CommentDraftScope, summary: String, target: String) async throws
        -> ApprovalOutcome
    {
        struct Body: Encodable {
            let revision: UInt32
            let summary: String
        }
        struct Response: Decodable {
            let item: FeedbackSnapshot.Item
            let outcome: ApprovalOutcome
        }
        let response = try await JSONDecoder().decode(
            Response.self,
            from: transport.post(
                scope.hub.appending(path: "api/items").appending(component: scope.item).appending(
                    path: "approve"),
                body: JSONEncoder().encode(Body(revision: scope.revision, summary: summary))))
        guard response.item.id == scope.item, response.item.revision.number == scope.revision,
            response.item.target == target,
            response.outcome.kind != "merged" || response.outcome.commit?.isEmpty == false
        else {
            throw FeedbackIssue(
                message: "The hub returned a different merge result. Check delivery before continuing.")
        }
        return response.outcome
    }
}
