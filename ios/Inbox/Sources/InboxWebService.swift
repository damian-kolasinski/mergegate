import Foundation
import Networking

struct InboxWebService: Sendable {
    let transport: any HTTPTransport
    let url: URL

    func triage(item: String) async throws -> InboxTriage {
        try await JSONDecoder().decode(
            InboxTriage.self, from: transport.get(url.appending(path: "api/items").appending(component: item))
        )
    }

    func load() async throws -> InboxSnapshot {
        try await JSONDecoder().decode(
            InboxSnapshot.self, from: transport.get(url.appending(path: "api/inbox"))
        )
    }
}
