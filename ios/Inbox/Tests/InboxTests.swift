import Foundation
import Networking
import Testing

@testable import Inbox

struct InboxTests {
    @Test @MainActor
    func refreshKeepsLastInboxOnFailureAndRecovers() async throws {
        let transport = InboxTransport()
        let model = InboxViewModel(service: .init(transport: transport, url: URL(string: "http://my-mac:7800")!))
        await model.refresh()
        #expect(model.snapshot?.needsYou.first?.name == "sample#1")
        await transport.fail(true)
        await model.refresh()
        #expect(model.error != nil)
        #expect(model.snapshot?.needsYou.count == 1)
        #expect(!model.isLoading)
        await transport.fail(false)
        await model.refresh()
        #expect(model.error == nil)
    }
}

private actor InboxTransport: HTTPTransport {
    func put(_ url: URL, body: Data) async throws -> Data {
        throw URLError(.unsupportedURL)
    }

    private var failing = false
    func fail(_ value: Bool) { failing = value }
    func get(_ url: URL) async throws -> Data {
        #expect(url.path == "/api/inbox")
        if failing { throw URLError(.notConnectedToInternet) }
        return Data(
            """
            {"needs_you":[{"id":"sample#1","name":"sample#1","title":"First item","state":"needs_review","revision":1,"review_files":1,"additions":2,"deletions":0}],"with_agents":[],"merging":[],"closed":[],"weekly":{},"rules":[]}
            """.utf8)
    }
}
