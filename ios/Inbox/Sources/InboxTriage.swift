import Foundation

// Consumed fields of ItemDiff in crates/api/src/diff.rs and review.rs.
struct InboxTriage: Decodable, Sendable {
    struct Item: Decodable, Sendable {
        struct Revision: Decodable, Sendable { let number: UInt32 }
        let revision: Revision
    }
    struct File: Decodable, Sendable {
        let viewed: Bool
        let tier: String
    }
    struct Comment: Decodable, Sendable {
        struct External: Decodable, Sendable {
            let pending: Bool
            let vote: String?
            let agreed_reply: UInt64?
        }
        let id: String
        let body: String
        let path: String?
        let external: External?
    }
    struct Reply: Decodable, Sendable {
        let id: UInt64
        let comment: String
        let author: String
        let status: String?
        let body: String
    }
    struct Decision: Identifiable, Sendable {
        let id: String
        let kind: String
        let comment: String
        let response: String?
        let path: String?

        var headline: String {
            switch kind {
            case "Question": "An answer unblocks the work"
            case "Disagreement": "A rejection needs your call"
            default: "Feedback needs your vote"
            }
        }
        var preview: String { kind == "Question" ? response ?? comment : comment }
    }

    let item: Item
    let files: [File]
    let comments: [Comment]
    let replies: [Reply]

    var remainingFiles: Int { files.filter { $0.tier == "review" && !$0.viewed }.count }
    var reviewFiles: Int { files.filter { $0.tier == "review" }.count }

    // Only sent replies count; an unsent operator draft cannot answer a question.
    var decisions: [Decision] {
        comments.compactMap { comment in
            let last = replies.last { $0.comment == comment.id }
            let kind: String
            if last?.author == "agent", last?.status == "question" {
                kind = "Question"
            } else if last?.author == "agent", last?.status == "rejected",
                      let external = comment.external, external.agreed_reply != last?.id {
                kind = "Disagreement"
            } else if let external = comment.external,
                      external.pending && external.vote == nil {
                kind = "Vote needed"
            } else { return nil }
            return Decision(
                id: comment.id, kind: kind, comment: comment.body,
                response: kind == "Vote needed" ? nil : last?.body, path: comment.path)
        }
    }
}
