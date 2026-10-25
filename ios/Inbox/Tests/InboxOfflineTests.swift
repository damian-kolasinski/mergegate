import Foundation
import Networking
import Testing

@testable import Inbox

struct InboxOfflineTests {
    @Test @MainActor func offlineRelaunchUsesOnlyThisHubsSavedInboxThenRefreshes() async throws {
        let directory = URL.temporaryDirectory.appending(component: UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = InboxCache(directory: directory)
        let hub = OfflineInboxHub()
        let url = URL(string: "http://my-mac")!
        let service = InboxWebService(transport: hub, url: url)
        let first = InboxViewModel(service: service, cache: cache)
        await first.refresh()
        #expect(first.snapshot?.needsYou.first?.revision == 1)
        await hub.disconnect()
        let reopened = InboxViewModel(service: service, cache: InboxCache(directory: directory))
        await reopened.refresh()
        #expect(reopened.snapshot?.needsYou.first?.revision == 1 && reopened.cachedAt != nil)
        let other = InboxViewModel(
            service: .init(transport: hub, url: URL(string: "http://other-mac")!), cache: cache)
        await other.refresh()
        #expect(other.snapshot == nil)
        await hub.reconnect()
        await reopened.receive(.connected)
        #expect(reopened.snapshot?.needsYou.first?.revision == 2 && reopened.cachedAt == nil)
    }
}

private actor OfflineInboxHub: HTTPTransport {
    var offline = false
    var revision = 1
    func disconnect() { offline = true }
    func reconnect() {
        offline = false
        revision = 2
    }
    func get(_ url: URL) async throws -> Data {
        if offline { throw URLError(.cannotConnectToHost) }
        return Data(
            """
            {"needs_you":[{"id":"sample#1","name":"sample#1","title":"Totals","state":"needs_review","revision":\(revision),"review_files":2,"additions":1,"deletions":1}],"with_agents":[],"merging":[],"closed":[]}
            """.utf8)
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
