import Foundation
import Networking

struct ReviewWebService: Sendable {
    let transport: any HTTPTransport
    let url: URL

    func markViewed(item: String, change: ReviewViewedChange) async throws {
        let endpoint = url.appending(path: "api/items").appending(component: item).appending(path: "viewed")
        _ = try await transport.put(endpoint, body: JSONEncoder().encode(change))
    }

    func load(item: String) async throws -> ReviewSnapshot {
        var endpoint = URLComponents(
            url: url.appending(path: "api/items").appending(component: item), resolvingAgainstBaseURL: false)!
        endpoint.queryItems = [URLQueryItem(name: "since", value: "all")]
        let snapshot = try await JSONDecoder().decode(ReviewSnapshot.self, from: transport.get(endpoint.url!))
        guard snapshot.item.id == item, snapshot.since == nil else { throw InvalidReview() }
        return snapshot
    }
}

private struct InvalidReview: LocalizedError {
    var errorDescription: String? {
        "The hub returned a different item or comparison. Return to the inbox and try again."
    }
}
