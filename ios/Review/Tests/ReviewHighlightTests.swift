import DiffDocumentInterface
import Foundation
import Networking
import Testing

@testable import Review

struct ReviewHighlightTests {
    @Test func hunkTuplesExcludePrefixesAndFollowTheirOriginalSide() throws {
        let data = Data(
            """
            {"path":"a.swift","grammar":"swift","old_blob":"old","new_blob":"new","tier":"review","viewed":false,
            "content":{"kind":"text","hunks":[{"old_start":4,"old_lines":2,"new_start":8,"new_lines":2,
            "lines":[" é 🐙 let", "-old", "+new", "\\\\ No newline at end of file"],
            "spans":[[[5,3,"keyword"]],[[0,3,"string"]],[[0,3,"function"]],[]]}]}}
            """.utf8)
        let file = try JSONDecoder().decode(ReviewFile.self, from: data)
        guard case .text(let hunks) = file.content else {
            Issue.record("Expected text")
            return
        }
        let lines = try #require(hunks.first?.lines)
        #expect(lines[0].spans == [CodeSpan(start: 5, length: 3, tag: "keyword")])
        #expect(lines[1].oldLine == 5 && lines[1].spans.first?.tag == "string")
        #expect(lines[2].newLine == 9 && lines[2].spans.first?.tag == "function")
        let painting = ReviewPainting(
            old: [[], [], [], [], [CodeSpan(start: 0, length: 3, tag: "constant")]],
            new: Array(repeating: [], count: 8) + [[CodeSpan(start: 0, length: 3, tag: "type")]])
        let painted = painting.paint(lines)
        #expect(painted[1].spans.first?.tag == "constant")
        #expect(painted[2].spans.first?.tag == "type")
        #expect(painted[3].spans.isEmpty)
        #expect(painted.map(\.text) == lines.map(\.text))
        #expect(ReviewPainting(old: nil, new: nil).paint(lines) == lines)
    }

    @Test @MainActor func lateSpansAndPackRemovalDoNotChangeSourceOrRevealedContext() async throws {
        let transport = HighlightTransport()
        let service = ReviewHighlightService(
            web: .init(transport: transport, url: URL(string: "http://hub")!), events: ServerEvents())
        let snapshot = try await service.web.load(item: "sample#1")
        let file = try #require(snapshot.files.first)
        let model = ReviewHighlightModel()
        await model.refresh(service: service, item: "sample#1", revision: 1, file: file)
        #expect(model.painting?.new == nil)
        await transport.setReady()
        await model.refresh(service: service, item: "sample#1", revision: 1, file: file)
        #expect(model.painting?.new?.first?.first?.tag == "keyword")
        await transport.advanceRevision()
        await model.refresh(service: service, item: "sample#1", revision: 1, file: file)
        #expect(model.painting?.new?.first?.first?.tag == "keyword")
        #expect(
            await transport.paths.filter { $0.contains("highlights") }.allSatisfy {
                $0 == "/api/highlights/swift/original"
            })
        await transport.removePack()
        await model.refresh(service: service, item: "sample#1", revision: 1, file: file)
        #expect(model.painting?.new == [])
    }
}

private actor HighlightTransport: HTTPTransport {
    func delete(_ url: URL) async throws -> Data { throw URLError(.unsupportedURL) }

    func post(_ url: URL, body: Data) async throws -> Data { throw URLError(.unsupportedURL) }

    func put(_ url: URL, body: Data) async throws -> Data {
        throw URLError(.unsupportedURL)
    }

    var ready = false
    var revision = 1
    var grammar = "\"swift\""
    var paths: [String] = []
    func setReady() { ready = true }
    func advanceRevision() { revision = 2 }
    func removePack() {
        grammar = "null"
        revision = 1
    }
    func get(_ url: URL) async throws -> Data {
        paths.append(url.path)
        if url.path.contains("highlights") {
            if !ready { throw URLError(.resourceUnavailable) }
            return Data("{\"lines\":[[[0,3,\"keyword\"]]]}".utf8)
        }
        return Data(
            """
            {"item":{"id":"sample#1","name":"sample#1","title":"Read code","target":"main","state":"needs_review","revision":{"number":\(revision)}},"since":null,
            "files":[{"path":"a.swift","grammar":\(grammar),"new_blob":"\(revision == 1 ? "original" : "other-revision")","tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}}]}
            """.utf8)
    }
}
