import Foundation
import Networking

struct DecisionWebService: Sendable {
    let transport: any HTTPTransport
    let hub: URL

    func load(item: String) async throws -> DecisionSnapshot {
        try await JSONDecoder().decode(
            DecisionSnapshot.self,
            from: transport.get(
                hub.appending(path: "api/items").appending(component: item)))
    }

    func saveAnswer(comment: String, reply: UInt64?, body: String) async throws {
        struct Body: Encodable { let body: String }
        let url = endpoint(comment).appending(path: "drafts")
        let data = try JSONEncoder().encode(Body(body: body))
        let result: Data
        if let reply {
            result = try await transport.put(url.appending(component: String(reply)), body: data)
        } else {
            result = try await transport.post(url, body: data)
        }
        let saved = try JSONDecoder().decode(DecisionSnapshot.DraftReply.self, from: result)
        guard saved.comment == comment && saved.body == body else {
            throw DecisionIssue(
                message:
                    "The hub returned a different answer. Check the saved answer before continuing."
            )
        }
    }

    func settle(comment: String, reply: UInt64, override: Bool, note: String) async throws {
        struct Body: Encodable { let note: String }
        let data = try await transport.post(
            endpoint(comment).appending(path: override ? "override" : "agree"),
            body: override ? JSONEncoder().encode(Body(note: note)) : Data("{}".utf8))
        if override {
            let sent = try JSONDecoder().decode(InboxTriage.Reply.self, from: data)
            guard sent.comment == comment, sent.author == "operator", sent.overrides == true,
                sent.body == note
            else {
                throw DecisionIssue(
                    message: "The hub returned a different override. Refresh before continuing.")
            }
        } else {
            let accepted = try JSONDecoder().decode(InboxTriage.Comment.self, from: data)
            guard accepted.id == comment, accepted.external?.agreed_reply == reply else {
                throw DecisionIssue(
                    message:
                        "The hub did not confirm this rejection was accepted. Refresh before continuing."
                )
            }
        }
    }

    private func endpoint(_ comment: String) -> URL {
        hub.appending(path: "api/comments").appending(component: comment)
    }
}
