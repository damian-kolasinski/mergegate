import Foundation

// The inbox feature consumes only this subset of crates/api/src/item.rs.
struct InboxSnapshot: Codable, Sendable {
    let needsYou: [InboxItem]
    let withAgents: [InboxItem]
    let merging: [InboxItem]
    let closed: [InboxItem]

    enum CodingKeys: String, CodingKey {
        case needsYou = "needs_you"
        case withAgents = "with_agents"
        case merging, closed
    }

    var isEmpty: Bool { needsYou.isEmpty && withAgents.isEmpty && merging.isEmpty && closed.isEmpty }
}

struct InboxItem: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let title: String
    let state: String
    let revision: UInt32
    let questions: UInt32?
    let rejections: UInt32?
    let votes: UInt32?
    let target: String?
    let note: String?
    let priority: String?
    let reviewFiles: UInt32
    let additions: UInt64
    let deletions: UInt64

    enum CodingKeys: String, CodingKey {
        case id, name, title, state, revision, additions, deletions
        case questions, rejections, votes, target, note, priority
        case reviewFiles = "review_files"
    }

    var project: String { String(id.split(separator: "#", maxSplits: 1)[0]) }
    var needsDecision: Bool { (questions ?? 0) + (rejections ?? 0) + (votes ?? 0) > 0 }
    var isClosed: Bool { state == "merged" || state == "abandoned" }

    var status: String {
        switch state {
        case "needs_review": "Ready for review"
        case "changes_requested": "Changes requested"
        case "merging": "Merging"
        case "merged": "Merged"
        case "abandoned": "Abandoned"
        default: "Status: \(state)"
        }
    }
}
