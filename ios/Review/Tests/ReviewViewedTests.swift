import Foundation
import Networking
import Testing

@testable import Review

struct ReviewViewedTests {
    @Test @MainActor func confirmedMarksPersistAcrossReopeningAndCanBeRemoved() async throws {
        let transport = ViewedTransport()
        let model = makeModel(transport)
        await model.load()
        await model.setViewed(path: ViewedTransport.path, viewed: true)
        #expect(try snapshot(model).remaining == 1)
        #expect(try snapshot(model).files[0].viewed)
        #expect(model.selectedPath == ViewedTransport.path)
        let reopened = makeModel(transport)
        await reopened.load()
        #expect(try snapshot(reopened).remaining == 1)
        await reopened.setViewed(path: ViewedTransport.path, viewed: false)
        #expect(try snapshot(reopened).remaining == 2)
        #expect(try !snapshot(reopened).files[0].viewed)
        #expect(await transport.writes == 2)
    }

    @Test @MainActor func failedMarkLeavesProgressAndRetriesTheOriginalFile() async throws {
        let transport = ViewedTransport(failFirst: true)
        let model = makeModel(transport)
        await model.load()
        await model.setViewed(path: ViewedTransport.path, viewed: true)
        #expect(try snapshot(model).remaining == 2)
        #expect(model.viewedFailure?.change.path == ViewedTransport.path)
        #expect(!model.isSavingViewed)
        model.selectedPath = "other.swift"
        await model.retryViewed()
        #expect(try snapshot(model).files[0].viewed)
        #expect(try !snapshot(model).files[1].viewed)
        #expect(model.selectedPath == "other.swift")
        #expect(model.viewedFailure == nil)
    }

    @Test @MainActor func pendingMarkCannotRaceAnotherToggleOrMoveToAnotherFile() async throws {
        let transport = ViewedTransport(blockWrites: true)
        let model = makeModel(transport)
        await model.load()
        let save = Task { await model.setViewed(path: ViewedTransport.path, viewed: true) }
        await transport.waitForWrite()
        #expect(model.isSavingViewed)
        #expect(try snapshot(model).remaining == 2)
        model.selectedPath = "other.swift"
        await model.setViewed(path: "other.swift", viewed: true)
        await transport.releaseWrite()
        await save.value
        #expect(await transport.writes == 1)
        #expect(try snapshot(model).files[0].viewed)
        #expect(try !snapshot(model).files[1].viewed)
        #expect(!model.isSavingViewed)
    }

    @Test @MainActor func trustedUnknownAndUnchangedMarksDoNotWrite() async throws {
        let transport = ViewedTransport()
        let model = makeModel(transport)
        await model.load()
        await model.setViewed(path: "receipt.bin", viewed: false)
        await model.setViewed(path: "missing", viewed: true)
        await model.setViewed(path: ViewedTransport.path, viewed: false)
        #expect(await transport.writes == 0)
        #expect(try snapshot(model).reviewCount == 2)
        #expect(try snapshot(model).remaining == 2)
    }

    @MainActor private func makeModel(_ transport: ViewedTransport) -> ReviewViewModel {
        .init(service: .init(transport: transport, url: URL(string: "http://my-mac:7800")!), item: "sample#3")
    }

    @MainActor private func snapshot(_ model: ReviewViewModel) throws -> ReviewSnapshot {
        guard case .loaded(let snapshot) = model.state else { throw URLError(.badServerResponse) }
        return snapshot
    }
}

private actor ViewedTransport: HTTPTransport {
    static let path = "src/total + tax #é.swift"
    private var viewed = false
    private var failFirst: Bool
    private let blockWrites: Bool
    private var pending: CheckedContinuation<Void, Never>?
    private var started: CheckedContinuation<Void, Never>?
    private(set) var writes = 0

    init(failFirst: Bool = false, blockWrites: Bool = false) {
        self.failFirst = failFirst
        self.blockWrites = blockWrites
    }

    func waitForWrite() async {
        if pending != nil { return }
        await withCheckedContinuation { started = $0 }
    }

    func releaseWrite() { pending?.resume(); pending = nil }

    func get(_ url: URL) async throws -> Data {
        Data(
            """
            {"item":{"id":"sample#3","name":"sample#93","title":"Totals","target":"main","state":"needs_review","revision":{"number":2}},
             "since":null,"files":[
              {"path":"\(Self.path)","tier":"review","viewed":\(viewed),"content":{"kind":"text","hunks":[]}},
              {"path":"other.swift","tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}},
              {"path":"receipt.bin","tier":"trusted","viewed":true,"content":{"kind":"binary"}}]}
            """.utf8)
    }

    func put(_ url: URL, body: Data) async throws -> Data {
        #expect(url.path == "/api/items/sample#3/viewed")
        #expect(url.fragment == nil && url.query == nil)
        let payload = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(payload["revision"] as? Int == 2)
        #expect(payload["path"] as? String == Self.path)
        let value = try #require(payload["viewed"] as? Bool)
        writes += 1
        if blockWrites {
            await withCheckedContinuation {
                pending = $0
                started?.resume()
                started = nil
            }
        }
        if failFirst {
            failFirst = false
            throw URLError(.timedOut)
        }
        viewed = value
        return Data()
    }
}
