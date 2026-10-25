import Foundation
import Networking
import Testing

@testable import Review

struct ReviewEventsTests {
    @Test @MainActor func newRevisionAndReconnectNeverReplacePinnedCodeOrSelection() async throws {
        let transport = LiveReviewTransport()
        let model = ReviewViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!),
            item: "sample#3")
        await model.load()
        model.selectedPath = "checkout.swift"
        await transport.advance()
        await model.receive(.data(#"{"type":"item.changed","item":"other#1"}"#))
        await model.receive(.data(#"{"type":"highlights.changed"}"#))
        await model.receive(.data("malformed"))
        #expect(await transport.reads == 1)
        await model.receive(.data(#"{"type":"item.changed","item":"sample#3"}"#))
        #expect(model.latestRevision == 2)
        guard case .loaded(let pinned) = model.state else {
            Issue.record("Missing review")
            return
        }
        #expect(pinned.item.revision.number == 1)
        #expect(!pinned.files[0].viewed)
        #expect(model.selectedPath == "checkout.swift")
        await model.receive(.disconnected)
        #expect(model.isReconnecting)
        await transport.advance()
        await model.receive(.connected)
        #expect(!model.isReconnecting)
        #expect(model.latestRevision == 3)
        guard case .loaded(let restored) = model.state else {
            Issue.record("Missing review")
            return
        }
        #expect(restored.item.revision.number == 1)
        #expect(model.selectedPath == "checkout.swift")
        await transport.fail()
        await model.receive(.connected)
        #expect(model.refreshFailure != nil)
        #expect(model.latestRevision == 3)
        await model.refreshCurrent()
        #expect(model.refreshFailure == nil)
    }

    @Test @MainActor func staleLiveReadCannotUndoACompletedViewedWrite() async throws {
        let transport = LiveReviewTransport()
        let model = ReviewViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!),
            item: "sample#3")
        await model.load()
        await transport.holdNextRead()
        let refresh = Task { await model.refreshCurrent() }
        await transport.waitForRead()
        await model.setViewed(path: "checkout.swift", viewed: true)
        await transport.releaseRead()
        await refresh.value
        guard case .loaded(let snapshot) = model.state else {
            Issue.record("Missing review")
            return
        }
        #expect(snapshot.files[0].viewed)
        #expect(snapshot.remaining == 0)
    }

    @Test @MainActor func reconnectRecoversAFailedInitialRead() async throws {
        let transport = LiveReviewTransport()
        let model = ReviewViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!),
            item: "sample#3")
        await transport.fail()
        await model.refreshCurrent()
        guard case .failed = model.state else {
            Issue.record("Expected opening failure")
            return
        }
        await model.receive(.connected)
        guard case .loaded(let snapshot) = model.state else {
            Issue.record("Reconnect did not reopen")
            return
        }
        #expect(snapshot.item.revision.number == 1)
    }

    @Test @MainActor func sameRevisionRefetchUpdatesProgressAndClosedState() async throws {
        let transport = LiveReviewTransport()
        let model = ReviewViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac")!),
            item: "sample#3")
        await model.load()
        await transport.close()
        await model.receive(.data(#"{"type":"inbox.changed"}"#))
        #expect(model.latestState == "merged")
        guard case .loaded(let snapshot) = model.state else {
            Issue.record("Missing review")
            return
        }
        #expect(snapshot.files[0].viewed)
        #expect(model.selectedPath == "checkout.swift")
    }
}

private actor LiveReviewTransport: HTTPTransport {
    var revision = 1
    var viewed = false
    var state = "needs_review"
    var reads = 0
    var failNext = false
    var hold = false
    var pending: CheckedContinuation<Void, Never>?
    var waiting: CheckedContinuation<Void, Never>?
    func advance() {
        revision += 1
        viewed = true
    }
    func close() {
        state = "merged"
        viewed = true
    }
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
        #expect(url.path == "/api/items/sample#3")
        #expect(
            URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first?.value
                == "all")
        if failNext {
            failNext = false
            throw URLError(.timedOut)
        }
        let response = Data(
            """
            {"item":{"id":"sample#3","name":"sample#93","title":"Checkout","target":"main","state":"\(state)","revision":{"number":\(revision)}},
            "since":null,"interdiffs":[],"files":[{"path":"checkout.swift","tier":"review","viewed":\(viewed),"content":{"kind":"text","hunks":[]}}]}
            """.utf8)
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
    func put(_ url: URL, body: Data) async throws -> Data {
        viewed = true
        return Data()
    }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
