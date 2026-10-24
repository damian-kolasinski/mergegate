import Foundation
import Networking
import Testing

@testable import Review

struct ReviewSourceTests {
    @Test @MainActor func failedExpansionRetriesPinnedBlobsWithoutReloadingReview() async throws {
        let file = try JSONDecoder().decode(
            ReviewFile.self,
            from: Data(
                """
                {"path":"new.swift","old_path":"old.swift","old_blob":"base123","new_blob":"head456",
                 "tier":"review","viewed":false,"content":{"kind":"text","hunks":[]}}
                """.utf8))
        let transport = SourceTransport()
        let service = ReviewSourceService(transport: transport, url: URL(string: "http://my-mac:7800")!)
        let model = ReviewSourceModel()
        await model.load(service: service, project: "sample", file: file, hunks: [])
        #expect(model.error != nil && model.context == nil && !model.loading)
        await model.load(service: service, project: "sample", file: file, hunks: [])
        #expect(model.error == nil)
        #expect(model.context?.gaps.first?.lines.first?.text == "unchanged é 🐙")
        #expect(model.context?.gaps.first?.lines.first?.hasNewline == false)
        let paths = await transport.paths
        #expect(paths.contains("/api/projects/sample/blobs/base123"))
        #expect(paths.contains("/api/projects/sample/blobs/head456"))
        #expect(paths.allSatisfy { $0.contains("/blobs/") })
    }
}

private actor SourceTransport: HTTPTransport {
    var failFirst = true
    var paths: [String] = []
    func get(_ url: URL) async throws -> Data {
        paths.append(url.path)
        if failFirst {
            failFirst = false
            throw URLError(.timedOut)
        }
        return Data("unchanged é 🐙".utf8)
    }
}
