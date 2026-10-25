import ConversationInterface
import DiffDocumentInterface
import Foundation
import SwiftData

@Model
final class StoredCommentDraft {
    @Attribute(.unique) var id: UUID
    var hub: String
    var item: String
    var revision: Int
    var path: String
    var side: String
    var startLine: Int
    var endLine: Int
    var quote: String
    var body: String
    var updatedAt: Date

    init(scope: CommentDraftScope, draft: CommentDraft) {
        id = draft.id
        hub = scope.storageHub
        item = scope.item
        revision = Int(scope.revision)
        path = draft.location.path
        side = draft.location.anchor.side.rawValue
        startLine = Int(draft.location.anchor.startLine)
        endLine = Int(draft.location.anchor.endLine)
        quote = draft.location.anchor.quote
        body = draft.body
        updatedAt = draft.updatedAt
    }

    var value: CommentDraft? {
        guard let side = SourceAnchor.Side(rawValue: side), let first = UInt32(exactly: startLine),
            let last = UInt32(exactly: endLine), first > 0, last >= first
        else { return nil }
        return CommentDraft(
            id: id,
            location: .init(
                path: path,
                anchor: .init(
                    side: side, startLine: first, endLine: last, quote: quote)), body: body,
            updatedAt: updatedAt)
    }
}

extension CommentDraftScope {
    var storageHub: String {
        var parts = URLComponents(url: hub, resolvingAgainstBaseURL: false)!
        parts.host = parts.host?.lowercased()
        if (parts.scheme == "http" && parts.port == 80)
            || (parts.scheme == "https" && parts.port == 443)
        {
            parts.port = nil
        }
        while parts.path.hasSuffix("/") { parts.path.removeLast() }
        return parts.string!
    }
}
