import ConversationInterface
import Foundation
import Networking

struct ConversationEditingService: Sendable {
    let scope: CommentDraftScope
    let transport: any HTTPTransport
    private var itemURL: URL {
        scope.hub.appending(path: "api/items").appending(component: scope.item)
    }
    private func commentURL(_ id: String) -> URL {
        scope.hub.appending(path: "api/comments").appending(component: id)
    }
    func current() async throws -> ConversationSnapshot {
        struct Item: Decodable {
            struct Revision: Decodable { let number: UInt32 }
            let id: String
            let state: String
            let revision: Revision
        }
        struct Envelope: Decodable { let item: Item }
        let data = try await transport.get(itemURL)
        let item = try JSONDecoder().decode(Envelope.self, from: data).item
        guard item.id == scope.item, item.revision.number == scope.revision,
            ["needs_review", "changes_requested"].contains(item.state)
        else {
            throw ConversationEditIssue(
                message:
                    "The item changed or no longer accepts feedback. Reopen its current revision. Your text is still here."
            )
        }
        return try JSONDecoder().decode(ConversationSnapshot.self, from: data)
    }
    func saveReply(comment: String, reply: UInt64?, body: String) async throws -> ThreadReply {
        struct Body: Encodable { let body: String }
        let endpoint = commentURL(comment).appending(path: "drafts")
        let data = try JSONEncoder().encode(Body(body: body))
        let response: Data
        if let reply {
            response = try await transport.put(
                endpoint.appending(component: String(reply)), body: data)
        } else {
            response = try await transport.post(endpoint, body: data)
        }
        let saved = try JSONDecoder().decode(ThreadReply.self, from: response)
        guard saved.comment == comment, saved.author == "operator", saved.body == body,
            reply == nil || saved.id == reply
        else {
            throw ConversationEditIssue(
                message: "The hub returned a different reply. Refresh to check the saved draft.")
        }
        return saved
    }
    func saveComment(id: String, body: String) async throws {
        struct Body: Encodable { let body: String }
        let response = try await transport.put(
            itemURL.appending(path: "drafts").appending(component: id),
            body: JSONEncoder().encode(Body(body: body)))
        let saved = try JSONDecoder().decode(ThreadComment.self, from: response)
        guard saved.id == id, saved.body == body else { throw URLError(.badServerResponse) }
    }
    func moveComment(id: String, path: String, side: String, start: UInt32, end: UInt32)
        async throws
    {
        struct Body: Encodable {
            let revision: UInt32
            let path: String
            let side: String
            let start_line: UInt32
            let line: UInt32
        }
        let response = try await transport.put(
            itemURL.appending(path: "drafts").appending(component: id).appending(path: "anchor"),
            body: JSONEncoder().encode(
                Body(revision: scope.revision, path: path, side: side, start_line: start, line: end)
            ))
        let saved = try JSONDecoder().decode(ThreadComment.self, from: response)
        guard saved.id == id, saved.revision == scope.revision, saved.path == path,
            saved.side == side,
            saved.start_line == start, saved.line == end
        else { throw URLError(.badServerResponse) }
    }
    func removeComment(_ id: String) async throws {
        _ = try await transport.delete(itemURL.appending(path: "drafts").appending(component: id))
    }
    func removeReply(_ reply: ThreadReply) async throws {
        _ = try await transport.delete(
            commentURL(reply.comment).appending(path: "drafts").appending(
                component: String(reply.id)))
    }
}

struct ConversationEditIssue: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
