import Foundation

// The inbox feature consumes only this subset of crates/api/src/item.rs.
struct InboxSnapshot: Decodable, Sendable {
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

struct InboxItem: Decodable, Identifiable, Sendable {
    let id: String
    let name: String
    let title: String
    let state: String
    let revision: UInt32
    let reviewFiles: UInt32
    let additions: UInt64
    let deletions: UInt64

    enum CodingKeys: String, CodingKey {
        case id, name, title, state, revision, additions, deletions
        case reviewFiles = "review_files"
    }

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
