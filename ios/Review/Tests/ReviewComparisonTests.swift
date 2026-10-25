import Foundation
import Networking
import Testing

@testable import Review

struct ReviewComparisonTests {
    @Test @MainActor func opensSinceLastReviewAndCarriesViewedState() async throws {
        let transport = ComparisonTransport()
        let model = model(transport)
        await model.load()
        let snapshot = try snapshot(model)
        #expect(snapshot.since == 1)
        #expect(snapshot.item.revision.number == 2)
        #expect(snapshot.remaining == 1)
        #expect(snapshot.reviewCount == 2)
        #expect(snapshot.files[1].viewed)
        #expect(model.selectedPath == "checkout.swift")
        #expect(await transport.comparisons == ["all", "1"])
        // Dropped files have no viewed mark in the newest revision.
        await model.setViewed(path: "obsolete.txt", viewed: true)
        #expect(await transport.writes == 0)
        model.selectedPath = "policy.txt"
        await model.compare(since: nil)
        #expect(try self.snapshot(model).since == nil)
        #expect(model.selectedPath == "policy.txt")
        await model.compare(since: 1)
        #expect(try self.snapshot(model).since == 1)
        #expect(model.selectedPath == "policy.txt")
    }

    @Test @MainActor func failedSwitchRetainsCodeSelectionAndCanRetry() async throws {
        let transport = ComparisonTransport()
        let model = model(transport)
        await model.load()
        model.selectedPath = "policy.txt"
        await transport.failNext()
        await model.compare(since: nil)
        #expect(try snapshot(model).since == 1)
        #expect(model.selectedPath == "policy.txt")
        #expect(model.comparisonFailure != nil)
        #expect(!model.isComparing)
        await model.compare(since: nil)
        #expect(try snapshot(model).since == nil)
        #expect(model.comparisonFailure == nil)
    }

    @Test @MainActor func newerRevisionCannotReplacePinnedReview() async throws {
        let transport = ComparisonTransport()
        let model = model(transport)
        await model.load()
        await transport.advance()
        await model.compare(since: nil)
        #expect(try snapshot(model).item.revision.number == 2)
        #expect(try snapshot(model).since == 1)
        #expect(model.comparisonFailure?.contains("newer revision") == true)
    }

    @Test @MainActor func unavailableComparisonsAndRepeatedChoicesDoNotLoad() async throws {
        let transport = ComparisonTransport()
        let model = model(transport)
        await model.load()
        await model.compare(since: 1)
        await model.compare(since: 8)
        #expect(await transport.comparisons == ["all", "1"])
    }

    @Test func interdiffSourceActionsOnlyUseRevisionCoordinates() {
        #expect(ReviewInterdiff.changed.sourceSides == [.new])
        #expect(ReviewInterdiff.added.sourceSides == [.new])
        #expect(ReviewInterdiff.deleted.sourceSides == [.new])
        #expect(ReviewInterdiff.dropped.sourceSides.isEmpty)
        #expect(ReviewInterdiff.unchanged.sourceSides == [.old, .new])
        #expect(ReviewInterdiff.unseparated.sourceSides == [.old, .new])
    }

    @MainActor private func model(_ transport: ComparisonTransport) -> ReviewViewModel {
        .init(service: .init(transport: transport, url: URL(string: "http://my-mac")!), item: "sample#3")
    }

    @MainActor private func snapshot(_ model: ReviewViewModel) throws -> ReviewSnapshot {
        guard case .loaded(let snapshot) = model.state else { throw URLError(.badServerResponse) }
        return snapshot
    }
}

private actor ComparisonTransport: HTTPTransport {
    var comparisons: [String] = []
    var writes = 0
    var revision = 2
    var fail = false
    func failNext() { fail = true }
    func advance() { revision = 3 }
    func get(_ url: URL) async throws -> Data {
        #expect(url.path == "/api/items/sample#3")
        #expect(url.fragment == nil)
        let since = URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems!.first!.value!
        comparisons.append(since)
        if fail { fail = false; throw URLError(.timedOut) }
        let interdiff = since == "all" ? "null" : "1"
        return Data("""
            {"item":{"id":"sample#3","name":"sample#93","title":"Checkout","target":"main","state":"needs_review","revision":{"number":\(revision)}},
             "since":\(interdiff),"last_reviewed":1,"interdiffs":[1],"files":[
              {"path":"checkout.swift","tier":"review","viewed":false,"interdiff":"changed","content":{"kind":"text","hunks":[]}},
              {"path":"policy.txt","tier":"review","viewed":true,"interdiff":"unchanged","content":{"kind":"text","hunks":[]}},
              {"path":"obsolete.txt","tier":"review","viewed":false,"interdiff":"dropped","content":{"kind":"text","hunks":[]}}]}
            """.utf8)
    }
    func put(_ url: URL, body: Data) async throws -> Data { writes += 1; return Data() }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
