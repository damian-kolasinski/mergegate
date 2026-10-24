import DiffDocument
import DiffDocumentInterface
import Foundation

// Review owns only the subset it reads from crates/api/src/{item,diff}.rs.
struct ReviewSnapshot: Decodable, Sendable {
    struct Item: Decodable, Sendable {
        struct Revision: Decodable, Sendable { let number: UInt32 }
        let id: String
        var project: String { String(id.prefix { $0 != "#" }) }
        let name: String
        let title: String
        let target: String
        let state: String
        let revision: Revision
    }
    let item: Item
    let guide: String?
    let files: [ReviewFile]
    let since: UInt32?

    var remaining: Int { files.filter { $0.tier == .review && !$0.viewed }.count }
}

struct ReviewFile: Decodable, Identifiable, Sendable {
    enum Tier: String, Decodable, Sendable { case review, trusted }
    let path: String
    let oldBlob: String?
    let newBlob: String?
    let oldPath: String?
    let oldMode: String?
    let newMode: String?
    let additions: UInt64?
    let deletions: UInt64?
    let tier: Tier
    let viewed: Bool
    let reason: String?
    let judgeReason: String?
    let why: String?
    let hint: String?
    let content: ReviewContent
    var id: String { path }
    var rankingReason: String? { reason ?? judgeReason ?? why ?? hint }
    var change: String {
        if oldPath != nil { return "Renamed" }
        if oldMode == nil && newMode != nil { return "Added" }
        if oldMode != nil && newMode == nil { return "Deleted" }
        return "Modified"
    }
    enum CodingKeys: String, CodingKey {
        case path, additions, deletions, tier, viewed, reason, why, hint, content
        case oldBlob = "old_blob"
        case newBlob = "new_blob"
        case oldPath = "old_path"
        case oldMode = "old_mode"
        case newMode = "new_mode"
        case judgeReason = "judge_reason"
    }
}

enum ReviewContent: Decodable, Sendable {
    case text([DiffHunk])
    case binary, tooLarge, gone

    private struct Hunk: Decodable {
        let old_start: UInt32
        let old_lines: UInt32
        let new_start: UInt32
        let new_lines: UInt32
        let lines: [String]
    }
    private enum Keys: CodingKey { case kind, hunks }
    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "text":
            self = .text(
                try container.decode([Hunk].self, forKey: .hunks).map {
                    try UnifiedHunk.build(
                        oldStart: $0.old_start, oldLines: $0.old_lines,
                        newStart: $0.new_start, newLines: $0.new_lines, lines: $0.lines)
                })
        case "binary": self = .binary
        case "too_large": self = .tooLarge
        case "gone": self = .gone
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "Unknown diff content")
        }
    }
}
