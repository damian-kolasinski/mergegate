import ConversationInterface
import DiffDocumentInterface
import Foundation

// Sent threads only: hub and local drafts have their own presentation and lifecycle.
struct ConversationSnapshot: Decodable, Sendable {
    let comments: [ThreadComment]
    let replies: [ThreadReply]
    var drafts: [ThreadComment]? = nil
    var draft_replies: [ThreadReply]? = nil

    func threads(path: String? = nil, anchors: [SourceAnchor] = [], revision: UInt32)
        -> [ConversationThread]
    {
        comments.compactMap { comment in
            let thread = ConversationThread(
                comment: comment, replies: replies.filter { $0.comment == comment.id },
                revision: revision)
            guard path == nil || comment.filePath == path else { return nil }
            if !anchors.isEmpty {
                guard let range = thread.sourceRange,
                    anchors.contains(where: {
                        comment.side == $0.side.rawValue
                            && range.overlaps($0.startLine...$0.endLine)
                    })
                else { return nil }
            }
            return thread
        }
    }
}

struct ThreadComment: Decodable, Sendable {
    struct Placement: Decodable, Sendable {
        let kind: String
        let start_line: UInt32?
        let line: UInt32?
    }
    struct External: Decodable, Sendable {
        var url: URL? = nil
        let author: String
        let path: String?
        let pending: Bool
        let summary: Bool?
        let vote: String?
        let lapsed: Bool?
        let agreed_reply: UInt64?

        var status: String {
            if summary == true { return "Summary" }
            if vote == "down" { return "Dismissed" }
            if lapsed == true { return "Lapsed" }
            if pending { return vote == nil ? "Needs your vote" : "Not sent yet" }
            return "Sent to agent"
        }
    }
    let id: String
    let revision: UInt32
    let path: String?
    let side: String?
    let start_line: UInt32?
    let line: UInt32?
    let body: String
    let quote: String?
    let placement: Placement?
    let external: External?
    var filePath: String? { path ?? external?.path }
}

struct ThreadReply: Decodable, Identifiable, Sendable {
    let id: UInt64
    let comment: String
    let author: String
    let status: String?
    let body: String
    let addressed_in: UInt32?
    let overrides: Bool?
    let created_at: String

    var timestamp: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let fractional = formatter.date(from: created_at)
        formatter.formatOptions = [.withInternetDateTime]
        return (fractional ?? formatter.date(from: created_at))?.formatted(
            date: .abbreviated, time: .shortened) ?? "Sent"
    }

    var statusLabel: String? {
        switch status {
        case "addressed":
            addressed_in.map { "Addressed in r\($0)" } ?? "Addressed in the next revision"
        case "question": "Question"
        case "rejected": "Rejected"
        default: status
        }
    }
}

struct ConversationThread: Identifiable, Sendable {
    let comment: ThreadComment
    let replies: [ThreadReply]
    let revision: UInt32
    var id: String { comment.id }

    var sourceRange: ClosedRange<UInt32>? {
        guard comment.revision <= revision else { return nil }
        let start: UInt32?
        let end: UInt32?
        if comment.revision == revision {
            start = comment.start_line
            end = comment.line
        } else if let placement = comment.placement, ["moved", "changed"].contains(placement.kind) {
            start = placement.start_line
            end = placement.line
        } else {
            return nil
        }
        guard let start, let end, start > 0, end >= start else { return nil }
        return start...end
    }

    var location: String {
        guard comment.filePath != nil else { return "Whole item" }
        guard comment.side != nil else { return "File comment · No source anchor" }
        let side = comment.side == "old" ? "Before" : "After"
        if let range = sourceRange {
            let lines =
                range.lowerBound == range.upperBound
                ? "Line \(range.lowerBound)" : "Lines \(range.lowerBound)–\(range.upperBound)"
            return "\(side) · \(lines)"
        }
        return "\(side) · Original r\(comment.revision) anchor"
    }

    var placementLabel: String? {
        if comment.revision > revision { return "From later revision r\(comment.revision)" }
        guard comment.revision < revision else { return nil }
        switch comment.placement?.kind {
        case "moved": return "Moved from r\(comment.revision)"
        case "changed": return "Source changed since r\(comment.revision)"
        default: return "Outdated · Not on this diff"
        }
    }

    /// An agent question, an unsettled rejection of a GitHub comment, or a GitHub comment
    /// awaiting the vote that decides whether the agent gets it, as the inbox counts them.
    var needsYou: Bool {
        let last = replies.last
        if last?.author == "agent", last?.status == "question" { return true }
        guard let external = comment.external else { return false }
        if last?.author == "agent", last?.status == "rejected", external.agreed_reply != last?.id {
            return true
        }
        return external.pending && external.vote == nil && external.summary != true
    }

    var status: String {
        guard let answer = replies.last(where: { $0.author == "agent" && $0.status != nil }) else {
            return comment.external?.status ?? "Awaiting reply"
        }
        if let last = replies.last, last.author == "operator" {
            return last.overrides == true
                ? "Override sent" : "Operator replied · \(answer.statusLabel ?? "")"
        }
        if answer.status == "rejected", comment.external?.agreed_reply == answer.id {
            return "Rejection accepted"
        }
        return answer.statusLabel ?? "Awaiting reply"
    }
}
