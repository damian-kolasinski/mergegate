import DiffDocumentInterface
import Foundation

enum ReviewInterdiff: String, Decodable, Sendable {
    case changed, added, deleted, unchanged, unseparated, dropped

    var label: String {
        switch self {
        case .changed: "Changed again"
        case .added: "Added since earlier revision"
        case .deleted: "Deleted since earlier revision"
        case .unchanged: "Unchanged · showing full diff"
        case .unseparated: "Couldn’t separate changes · showing full diff"
        case .dropped: "Change dropped · restoring base"
        }
    }

    // The interdiff's old blob is a replay, not this revision's base.
    var sourceSides: [SourceAnchor.Side] {
        switch self {
        case .unchanged, .unseparated: [.old, .new]
        case .dropped: []
        default: [.new]
        }
    }
}

struct ReviewComparisonChanged: LocalizedError {
    var errorDescription: String? {
        "A newer revision is available. Return to the inbox and reopen the review to read it. Your drafts are retained."
    }
}
