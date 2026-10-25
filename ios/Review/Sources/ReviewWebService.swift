import Foundation
import Networking

struct ReviewWebService: Sendable {
    let transport: any HTTPTransport
    let url: URL
    var cache: ReviewCache? = nil

    func markViewed(item: String, change: ReviewViewedChange) async throws {
        let endpoint = url.appending(path: "api/items").appending(component: item).appending(
            path: "viewed")
        _ = try await transport.put(endpoint, body: JSONEncoder().encode(change))
    }

    func load(item: String, since: UInt32? = nil) async throws -> ReviewSnapshot {
        var endpoint = URLComponents(
            url: url.appending(path: "api/items").appending(component: item),
            resolvingAgainstBaseURL: false)!
        endpoint.queryItems = [URLQueryItem(name: "since", value: since.map(String.init) ?? "all")]
        let data = try await transport.get(endpoint.url!)
        let snapshot = try JSONDecoder().decode(ReviewSnapshot.self, from: data)
        guard snapshot.item.id == item, snapshot.since == since else { throw InvalidReview() }
        try Task.checkCancellation()
        if let cache {
            await cache.save(
                data,
                key: cache.snapshotKey(
                    hub: url, item: item, revision: snapshot.item.revision.number, since: since))
        }
        return snapshot
    }

    func cached(item: String, position: ReviewReadingPosition) async -> (ReviewSnapshot, Date)? {
        guard let cache,
            let saved = await cache.read(
                cache.snapshotKey(
                    hub: url, item: item, revision: position.revision, since: position.since)),
            let snapshot = try? JSONDecoder().decode(ReviewSnapshot.self, from: saved.data),
            snapshot.item.id == item, snapshot.item.revision.number == position.revision,
            snapshot.since == position.since
        else { return nil }
        return (snapshot, saved.date)
    }

}

private struct InvalidReview: LocalizedError {
    var errorDescription: String? {
        "The hub returned a different item or comparison. Return to the inbox and try again."
    }
}
