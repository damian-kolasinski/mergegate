import ConversationInterface
import DiffDocumentInterface
import Foundation

struct CommentDraft: Identifiable, Equatable {
    let id: UUID
    let location: CommentDraftLocation
    var body: String
    var updatedAt: Date

    init(id: UUID = UUID(), location: CommentDraftLocation, body: String = "", updatedAt: Date = .now) {
        self.id = id
        self.location = location
        self.body = body
        self.updatedAt = updatedAt
    }
}

extension CommentDraftLocation {
    var label: String {
        let lines =
            anchor.startLine == anchor.endLine
            ? "Line \(anchor.startLine)" : "Lines \(anchor.startLine)–\(anchor.endLine)"
        return "\(anchor.side == .old ? "Before" : "After") · \(lines)"
    }
}
