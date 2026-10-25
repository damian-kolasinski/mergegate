import ConversationInterface
import Foundation

struct FeedbackComment: Codable, Equatable, Identifiable, Sendable {
    let id: String
    let revision: UInt32
    let path: String?
    let side: String?
    let start_line: UInt32?
    let line: UInt32?
    let body: String

    func matches(_ draft: FeedbackDraft, revision: UInt32) -> Bool {
        self.revision == revision && path == draft.location.path
            && side == draft.location.anchor.side.rawValue
            && start_line == draft.location.anchor.startLine && line == draft.location.anchor.endLine
            && body == draft.body.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

struct FeedbackReply: Codable, Equatable, Identifiable, Sendable {
    let id: UInt64
    let comment: String
    let body: String
}

struct FeedbackSnapshot: Decodable, Sendable {
    struct Item: Decodable, Sendable {
        struct Revision: Decodable, Sendable { let number: UInt32 }
        let id: String
        let state: String
        let target: String
        let revision: Revision
    }
    let item: Item
    let drafts: [FeedbackComment]
    let draft_replies: [FeedbackReply]
}

struct FeedbackReceipt: Decodable, Sendable {
    let revision: UInt32
    let verdict: String
    let summary: String
    let comments: [FeedbackComment]
    let replies: [FeedbackReply]
}

struct FeedbackLog: Decodable, Sendable {
    struct Event: Decodable, Sendable {
        let item: String
        let revision: UInt32
        let type: String
        let verdict: String?
        let summary: String?
        let comments: [FeedbackComment]?
        let replies: [FeedbackReply]?
        let commit: String?
        let reason: String?
        let paths: [String]?
    }
    let cursor: UInt64
    let events: [Event]
}

struct FeedbackIssue: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
