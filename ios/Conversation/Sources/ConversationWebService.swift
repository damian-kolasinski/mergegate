import ConversationInterface
import Foundation
import Networking

struct ConversationWebService: Sendable {
    let transport: any HTTPTransport

    func load(scope: CommentDraftScope) async throws -> ConversationSnapshot {
        var endpoint = URLComponents(
            url: scope.hub.appending(path: "api/items").appending(component: scope.item)
                .appending(path: "conversation"), resolvingAgainstBaseURL: false)!
        endpoint.queryItems = [URLQueryItem(name: "onto", value: String(scope.revision))]
        return try await JSONDecoder().decode(
            ConversationSnapshot.self, from: transport.get(endpoint.url!))
    }
}
