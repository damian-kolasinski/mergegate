import Foundation

// The inbox feature consumes only this subset of crates/api/src/item.rs.
struct InboxSnapshot: Codable, Sendable {
    struct PendingRules: Codable, Sendable {
        let project: String
        let changes: UInt32
    }
    var rules: [PendingRules]? = nil
    var weekly: InboxWeek? = nil
    let needsYou: [InboxItem]
    let withAgents: [InboxItem]
    let merging: [InboxItem]
    let closed: [InboxItem]

    enum CodingKeys: String, CodingKey {
        case needsYou = "needs_you"
        case withAgents = "with_agents"
        case merging, closed, rules, weekly
    }

    var isEmpty: Bool {
        needsYou.isEmpty && withAgents.isEmpty && merging.isEmpty && closed.isEmpty
    }
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
    struct Link: Codable, Sendable { let url: URL }
    struct ChangedSince: Codable, Sendable {
        let revision: UInt32
        let files: UInt32
        let approved: Bool
    }
    struct Blocked: Codable, Sendable {
        let kind: String
        let reason: String?
        let parent: String?
        let paths: [String]?
    }
    var createdAt: String? = nil
    var updatedAt: String? = nil
    var link: Link? = nil
    var parent: String? = nil
    var behind: Bool? = nil
    var demo: Bool? = nil
    var blocked: Blocked? = nil
    var changedSince: ChangedSince? = nil
    var branch: String? = nil
    var files: UInt32? = nil
    var mergeable: Mergeable? = nil
    let reviewFiles: UInt32
    let additions: UInt64
    let deletions: UInt64

    enum CodingKeys: String, CodingKey {
        case id, name, title, state, revision, additions, deletions
        case questions, rejections, votes, target, note, priority
        case link, parent, behind, demo, blocked, branch, files, mergeable
        case changedSince = "changed_since"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case reviewFiles = "review_files"
    }

    // Whether the newest revision merges into its target, checked by the hub before approval.
    enum Mergeable: Codable, Equatable, Sendable {
        case clean
        case conflict(paths: [String])
        case unknown

        private enum Keys: String, CodingKey { case kind, paths }
        init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: Keys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "clean": self = .clean
            case "conflict":
                self = .conflict(paths: try container.decodeIfPresent([String].self, forKey: .paths) ?? [])
            default: self = .unknown
            }
        }
        func encode(to encoder: any Encoder) throws {
            var container = encoder.container(keyedBy: Keys.self)
            switch self {
            case .clean: try container.encode("clean", forKey: .kind)
            case .conflict(let paths):
                try container.encode("conflict", forKey: .kind)
                try container.encode(paths, forKey: .paths)
            case .unknown: try container.encode("unknown", forKey: .kind)
            }
        }
    }

    var project: String { String(id.split(separator: "#", maxSplits: 1)[0]) }
    var needsDecision: Bool { (questions ?? 0) + (rejections ?? 0) + (votes ?? 0) > 0 }
    var isClosed: Bool { state == "merged" || state == "abandoned" }
}
