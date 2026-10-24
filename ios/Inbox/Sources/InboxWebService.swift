import Foundation
import Networking

struct InboxWebService: Sendable {
    let transport: any HTTPTransport
    let url: URL

    func load() async throws -> InboxSnapshot {
        try await JSONDecoder().decode(
            InboxSnapshot.self, from: transport.get(url.appending(path: "api/inbox"))
        )
    }
}
