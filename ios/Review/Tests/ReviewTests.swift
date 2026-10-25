import Foundation
import Networking
import Testing

@testable import Review

struct ReviewTests {
    @Test @MainActor func realWireSubsetPreservesOrderGuideAndRetry() async throws {
        let transport = ReviewTransport()
        let model = ReviewViewModel(
            service: .init(transport: transport, url: URL(string: "http://my-mac:7800")!), item: "sample#3")
        await model.load()
        guard case .failed = model.state else {
            Issue.record("Expected recoverable transport error")
            return
        }
        await model.load()
        guard case .loaded(let snapshot) = model.state else {
            Issue.record("Expected loaded review")
            return
        }
        #expect(snapshot.item.revision.number == 2)
        #expect(
            snapshot.files.map(\.path) == ["z.swift", "a.swift", "image.png", "large.txt", "gone.txt", "renamed.swift"])
        #expect(model.selectedPath == "z.swift")
        #expect(snapshot.remaining == 4)
        #expect(snapshot.guide == "Read the workflow first.")
        #expect(snapshot.files.first?.rankingReason == "Workflow before helpers")
        guard case .text(let hunks) = snapshot.files[0].content else {
            Issue.record("Expected real text content")
            return
        }
        #expect(hunks.first?.lines.last?.text == "let value = 2")
        #expect(snapshot.files.last?.change == "Renamed")
        if case .binary = snapshot.files[2].content {} else { Issue.record("Expected binary") }
        if case .tooLarge = snapshot.files[3].content {} else { Issue.record("Expected too large") }
        if case .gone = snapshot.files[4].content {} else { Issue.record("Expected gone") }
    }

    @Test func differentComparisonIsRejected() async {
        let service = ReviewWebService(
            transport: ReviewTransport(failFirst: false, since: "1"), url: URL(string: "http://my-mac")!)
        await #expect(throws: (any Error).self) { try await service.load(item: "sample#3") }
    }

    @Test func mismatchedItemIsRejected() async {
        let service = ReviewWebService(
            transport: ReviewTransport(failFirst: false, id: "wrong#1"), url: URL(string: "http://my-mac")!)
        await #expect(throws: (any Error).self) { try await service.load(item: "sample#3") }
    }
}

private actor ReviewTransport: HTTPTransport {
    func put(_ url: URL, body: Data) async throws -> Data {
        throw URLError(.unsupportedURL)
    }

    var failFirst: Bool
    let id: String
    let since: String
    init(failFirst: Bool = true, id: String = "sample#3", since: String = "null") {
        self.failFirst = failFirst
        self.id = id
        self.since = since
    }
    func get(_ url: URL) async throws -> Data {
        #expect(url.path == "/api/items/sample#3")
        #expect(url.fragment == nil)
        #expect(
            URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems == [
                URLQueryItem(name: "since", value: "all")
            ])
        if failFirst {
            failFirst = false
            throw URLError(.timedOut)
        }
        return Data(
            """
            {"item":{"id":"\(id)","name":"sample#93","title":"Workflow","target":"main","state":"needs_review","revision":{"number":2}},
             "since":\(since),"guide":"Read the workflow first.","files":[
              {"path":"z.swift","tier":"review","viewed":false,"reason":"Workflow before helpers","content":{"kind":"text","hunks":[{"old_start":1,"old_lines":1,"new_start":1,"new_lines":1,"lines":["-let value = 1","+let value = 2"]}]}},
              {"path":"a.swift","tier":"review","viewed":true,"content":{"kind":"text","hunks":[]}},
              {"path":"image.png","tier":"trusted","viewed":true,"content":{"kind":"binary"}},
              {"path":"large.txt","tier":"review","viewed":false,"content":{"kind":"too_large"}},
              {"path":"gone.txt","tier":"review","viewed":false,"content":{"kind":"gone"}},
              {"path":"renamed.swift","old_path":"old.swift","tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}}]}
            """.utf8)
    }
}
