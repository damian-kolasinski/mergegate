import Foundation
import Networking

struct RulesWebService: Sendable {
    let hub: URL
    let project: String
    let transport: any HTTPTransport
    private var endpoint: URL {
        hub.appending(path: "api/projects").appending(component: project).appending(path: "rules")
    }
    func load() async throws -> RulesSnapshot {
        try decode(await transport.get(endpoint))
    }
    func decide(_ decision: RulesDecision) async throws -> RulesSnapshot {
        let url = decision.components.reduce(endpoint) { $0.appending(component: $1) }
        return try decode(await transport.post(url, body: Data("{}".utf8)))
    }
    private func decode(_ data: Data) throws -> RulesSnapshot {
        let result = try JSONDecoder().decode(RulesSnapshot.self, from: data)
        guard result.project == project else { throw URLError(.badServerResponse) }
        return result
    }
}
