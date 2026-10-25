import DiffDocument
import DiffDocumentInterface
import Foundation
import Networking

struct ReviewSourceService: Sendable {
    let transport: any HTTPTransport
    let url: URL
    var cache: ReviewCache? = nil

    func load(project: String, file: ReviewFile, hunks: [DiffHunk]) async throws -> SourceContext {
        async let old = source(project: project, blob: file.oldBlob)
        async let new = source(project: project, blob: file.newBlob)
        return try await SourceContext(hunks: hunks, old: old, new: new)
    }

    private func source(project: String, blob: String?) async throws -> String {
        guard let blob else { return "" }
        let endpoint = url.appending(path: "api/projects").appending(component: project)
            .appending(path: "blobs").appending(component: blob)
        let data: Data
        if let saved = await cache?.read(endpoint.absoluteString) {
            data = saved.data
        } else {
            data = try await transport.get(endpoint)
            guard !data.contains(0), String(data: data, encoding: .utf8) != nil else {
                throw InvalidSource()
            }
            await cache?.save(data, key: endpoint.absoluteString)
        }
        guard !data.contains(0), let source = String(data: data, encoding: .utf8) else {
            throw InvalidSource()
        }
        return source
    }
}

private struct InvalidSource: LocalizedError {
    var errorDescription: String? {
        "This file’s source is not UTF-8 text. The changed lines remain available."
    }
}
