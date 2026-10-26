import Foundation

// The subset of the item's conversation the conversations button counts, as the inbox does:
// agent questions, unsettled rejections of GitHub comments, and GitHub comments awaiting a vote.
struct ReviewComment: Decodable, Sendable {
    struct External: Decodable, Sendable {
        let pending: Bool
        let vote: String?
        let agreed_reply: UInt64?
        var summary: Bool? = nil
    }
    let id: String
    let external: External?
}

struct ReviewReply: Decodable, Sendable {
    let id: UInt64
    let comment: String
    let author: String
    let status: String?
}

extension ReviewSnapshot {
    var needsYou: Int {
        (comments ?? []).filter { comment in
            let last = replies?.last { $0.comment == comment.id }
            if last?.author == "agent", last?.status == "question" { return true }
            guard let external = comment.external else { return false }
            if last?.author == "agent", last?.status == "rejected", external.agreed_reply != last?.id {
                return true
            }
            return external.pending && external.vote == nil && external.summary != true
        }.count
    }
}
