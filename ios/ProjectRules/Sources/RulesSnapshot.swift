import Foundation

// ProjectRules owns its subset of crates/api/src/rules.rs.
struct RulesSnapshot: Decodable, Sendable {
    struct Rule: Decodable, Identifiable, Sendable {
        let id: String
        let tier: String
        let summary: String
        let reason: String
    }
    struct Preview: Decodable, Sendable {
        struct HiddenFile: Decodable, Sendable {
            let item: String
            let name: String
            let title: String
            let path: String
            let rule: String
        }
        let items: UInt32
        let hidden: [HiddenFile]
        var summary: String {
            "Would have hidden \(hidden.count) \(hidden.count == 1 ? "file" : "files") in \(Set(hidden.map(\.item)).count) of \(items) recent \(items == 1 ? "item" : "items")."
        }
    }
    struct Proposal: Decodable, Sendable {
        let rule: Rule
        let preview: Preview?
    }
    struct Edit: Decodable, Sendable {
        let rules: [Rule]
        let preview: Preview
    }
    let project: String
    let location: String
    let rules: [Rule]
    let error: String?
    let pending: [Proposal]
    let edit: Edit?
}

enum RulesDecision: Equatable, Sendable {
    case proposal(String, approve: Bool)
    case edit(approve: Bool)
    var components: [String] {
        switch self {
        case .proposal(let id, let approve): ["proposals", id, approve ? "approve" : "reject"]
        case .edit(let approve): ["edit", approve ? "approve" : "reject"]
        }
    }
}
