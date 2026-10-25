import Foundation
import Networking
import Testing

@testable import Review

struct ReviewComparisonHighlightTests {
    @Test func newlyInstalledGrammarRefreshesTheDisplayedInterdiff() async throws {
        let transport = ComparisonHighlightTransport()
        let service = ReviewHighlightService(
            web: .init(transport: transport, url: URL(string: "http://hub")!),
            events: ServerEvents(), since: 1)
        let snapshot = try await service.web.load(item: "sample#1", since: 1)
        let file = try #require(snapshot.files.first)
        #expect(file.grammar == nil)
        await transport.installGrammar()
        let painting = try await service.load(item: "sample#1", revision: 2, file: file)
        #expect(painting.old?.first?.first?.tag == "keyword")
        #expect(painting.new?.first?.first?.tag == "keyword")
        #expect(await transport.comparisons == ["1", "1"])
    }
}

private actor ComparisonHighlightTransport: HTTPTransport {
    var installed = false
    var comparisons: [String] = []
    func installGrammar() { installed = true }
    func get(_ url: URL) async throws -> Data {
        if url.path.contains("highlights") {
            return Data("{\"lines\":[[[0,3,\"keyword\"]]]}".utf8)
        }
        let since = URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems!.first!.value!
        comparisons.append(since)
        return Data("""
            {"item":{"id":"sample#1","name":"sample#1","title":"Read code","target":"main","state":"needs_review","revision":{"number":2}},
             "interdiffs":[1],"last_reviewed":1,"since":\(since == "all" ? "null" : "1"),
             "files":[{"path":"a.swift","grammar":\(installed ? "\"swift\"" : "null"),
             "old_blob":"\(since == "all" ? "base" : "replayed")","new_blob":"current",
             "tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}}]}
            """.utf8)
    }
    func put(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }
}
