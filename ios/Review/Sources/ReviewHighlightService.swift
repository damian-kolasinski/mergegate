import DiffDocumentInterface
import Foundation
import Networking

struct ReviewHighlightService: Sendable {
    let web: ReviewWebService
    let events: ServerEvents
    var since: UInt32? = nil

    func load(item: String, revision: UInt32, file: ReviewFile) async throws -> ReviewPainting {
        let snapshot = try await web.load(item: item, since: since)
        // A highlight refresh must never replace the displayed revision's code or source coordinates.
        let current =
            snapshot.item.revision.number == revision
            ? snapshot.files.first(where: {
                $0.path == file.path && $0.oldBlob == file.oldBlob && $0.newBlob == file.newBlob
            })
            : nil
        let grammar = current?.grammar ?? (current == nil ? file.grammar : nil)
        guard let grammar else { return ReviewPainting(old: [], new: []) }
        async let old = spans(grammar: grammar, blob: file.oldBlob)
        async let new = spans(grammar: grammar, blob: file.newBlob)
        return try await ReviewPainting(old: old, new: new)
    }

    private func spans(grammar: String, blob: String?) async throws -> [[CodeSpan]]? {
        guard let blob else { return [] }
        let endpoint = web.url.appending(path: "api/highlights").appending(component: grammar)
            .appending(component: blob)
        do {
            let result = try await JSONDecoder().decode(Blob.self, from: web.transport.get(endpoint))
            return result.lines.map { $0.map(\.value) }
        } catch let error as HTTPFailure where error.status == 404 {
            return nil
        }
    }

    private struct Blob: Decodable { let lines: [[ReviewCodeSpan]] }
}

struct ReviewPainting: Sendable {
    // nil means pending/unavailable; retain the hunk's original spans until a response arrives.
    let old: [[CodeSpan]]?
    let new: [[CodeSpan]]?

    func paint(_ lines: [DiffLine]) -> [DiffLine] {
        lines.map { line in
            let side = line.kind == .removed ? old : new
            let number = line.kind == .removed ? line.oldLine : line.newLine
            let spans: [CodeSpan]
            if let side, let number, number > 0 {
                spans = Int(number) <= side.count ? side[Int(number) - 1] : []
            } else {
                spans = line.spans
            }
            return DiffLine(
                kind: line.kind, oldLine: line.oldLine, newLine: line.newLine, text: line.text,
                hasNewline: line.hasNewline, spans: line.kind == .note ? [] : spans)
        }
    }

    func paint(_ hunks: [DiffHunk]) -> [DiffHunk] {
        hunks.map {
            DiffHunk(
                header: $0.header, lines: paint($0.lines), oldRange: $0.oldRange, newRange: $0.newRange)
        }
    }
}
