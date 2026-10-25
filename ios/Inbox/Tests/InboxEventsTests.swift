import Foundation
import Networking
import Testing

@testable import Inbox

struct InboxEventsTests {
    @Test @MainActor func newItemsAndReconnectRefreshButKeepFiltersAndLastResponse() async {
        let transport = LiveInboxTransport()
        let model = InboxViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!))
        await model.refresh()
        model.project = "sample"
        model.filter = .review
        await transport.advance()
        await model.receive(.data(#"{"type":"highlights.changed"}"#))
        await model.receive(.data("bad frame"))
        #expect(model.snapshot?.needsYou.count == 1)
        await model.receive(.data(#"{"type":"inbox.changed"}"#))
        #expect(model.snapshot?.needsYou.count == 2)
        #expect(model.project == "sample")
        #expect(model.filter == .review)
        await model.receive(.disconnected)
        #expect(model.isReconnecting)
        await transport.fail()
        await model.receive(.connected)
        #expect(!model.isReconnecting)
        #expect(model.error != nil)
        #expect(model.snapshot?.needsYou.count == 2)
        await transport.advance()
        await model.receive(.connected)
        #expect(model.error == nil)
        #expect(model.snapshot?.needsYou.count == 3)
    }

    @Test @MainActor func foregroundRefreshCannotBeSwallowedByACancelledRead() async {
        let transport = LiveInboxTransport()
        let model = InboxViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!))
        await transport.holdNextRead()
        let old = Task { await model.refresh() }
        await transport.waitForRead()
        old.cancel()
        await transport.advance()
        await model.receive(.connected)
        await transport.releaseRead()
        await old.value
        #expect(model.snapshot?.needsYou.count == 2)
        #expect(!model.isLoading)
    }

    @Test @MainActor func eventDuringInFlightReadWinsOverTheOlderResponse() async {
        let transport = LiveInboxTransport()
        let model = InboxViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!))
        await transport.holdNextRead()
        let first = Task { await model.refresh() }
        await transport.waitForRead()
        await transport.advance()
        await model.receive(.data(#"{"type":"item.changed","item":"sample#1"}"#))
        await transport.releaseRead()
        await first.value
        #expect(model.snapshot?.needsYou.count == 2)
        #expect(await transport.reads == 2)
    }
}

private actor LiveInboxTransport: HTTPTransport {
    var count = 1
    var reads = 0
    var failNext = false
    var hold = false
    var pending: CheckedContinuation<Void, Never>?
    var waiting: CheckedContinuation<Void, Never>?
    func advance() { count += 1 }
    func fail() { failNext = true }
    func holdNextRead() { hold = true }
    func waitForRead() async {
        if pending != nil { return }
        await withCheckedContinuation { waiting = $0 }
    }
    func releaseRead() {
        pending?.resume()
        pending = nil
    }
    func get(_ url: URL) async throws -> Data {
        reads += 1
        if failNext {
            failNext = false
            throw URLError(.notConnectedToInternet)
        }
        let items = (1...count).map {
            """
            {"id":"sample#\($0)","name":"sample#\($0)","title":"Item","state":"needs_review","revision":1,"review_files":1,"additions":1,"deletions":0}
            """
        }.joined(separator: ",")
        let response = Data(
            "{\"needs_you\":[\(items)],\"with_agents\":[],\"merging\":[],\"closed\":[],\"weekly\":{},\"rules\":[]}"
                .utf8)
        if hold {
            hold = false
            await withCheckedContinuation {
                pending = $0
                waiting?.resume()
                waiting = nil
            }
        }
        return response
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
