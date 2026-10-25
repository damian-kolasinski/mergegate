import Foundation

// The decision workflow's subset of ItemDiff and Reply in crates/api.
struct DecisionSnapshot: Decodable, Sendable {
    struct Item: Decodable, Sendable {
        let id: String
        let state: String
        let revision: InboxTriage.Item.Revision
    }
    struct DraftReply: Decodable, Sendable {
        let id: UInt64
        let comment: String
        let body: String
    }
    let item: Item
    let comments: [InboxTriage.Comment]
    let replies: [InboxTriage.Reply]
    let draft_replies: [DraftReply]

    var decisions: [InboxTriage.Decision] {
        InboxTriage(
            item: .init(revision: item.revision), files: [], comments: comments, replies: replies
        ).decisions
    }
}

struct DecisionIssue: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
