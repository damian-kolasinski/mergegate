import Foundation

// Additional review metadata follows crates/api/src/{diff,item,merge,attachment,bot}.rs.
struct ReviewLink: Decodable, Sendable {
    let label: String
    let title: String
    let url: URL
    var webURL: URL? { ["http", "https"].contains(url.scheme?.lowercased() ?? "") ? url : nil }
}
struct ReviewBlocked: Decodable, Sendable {
    let kind: String
    let reason: String?
    let parent: String?
    let paths: [String]?
    let target_commit: String?
    var label: String {
        switch kind {
        case "held": "Merge held"
        case "waiting": "Approved · Waiting for parent"
        case "behind": "Approved · Rebase needed"
        case "conflict": "Conflicts with target"
        default: "Merge blocked"
        }
    }
    var detail: String {
        switch kind {
        case "held": reason ?? ""
        case "waiting": "Merges after \(parent ?? "its parent") lands."
        case "behind":
            "Waiting for a rebase after \(parent ?? "its parent") merged. The approval stands if the diff stays unchanged."
        case "conflict": "Sent back for a rebase.\n" + (paths ?? []).joined(separator: "\n")
        default: ""
        }
    }
}
// Whether the newest revision merges into its target, checked by the hub before approval.
enum ReviewMergeable: Decodable, Sendable, Equatable {
    case clean
    case conflict(paths: [String])
    case unknown

    private enum Keys: String, CodingKey { case kind, paths }
    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "clean": self = .clean
        case "conflict":
            self = .conflict(
                paths: try container.decodeIfPresent([String].self, forKey: .paths) ?? [])
        default: self = .unknown
        }
    }
}

struct ReviewStackEntry: Decodable, Identifiable, Sendable {
    let id: String
    let name: String
    let title: String
    let state: String
}
struct ReviewBot: Decodable, Sendable {
    let login: String
    let name: String
    let state: String
}
struct ReviewDemo: Decodable, Sendable {
    struct Attachment: Decodable, Identifiable, Sendable {
        let revision: UInt32
        let name: String
        let media_type: String
        let video: Bool
        let size: UInt64
        let url: String
        var id: String { url + name }
    }
    let revision: UInt32
    let zip_url: String
    let attachments: [Attachment]
}
