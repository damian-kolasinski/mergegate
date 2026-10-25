import DiffDocument
import DiffDocumentInterface
import Foundation
import Observation

@MainActor @Observable
final class ReviewSourceModel {
    private(set) var context: SourceContext?
    private(set) var loading = false
    private(set) var error: String?

    func load(service: ReviewSourceService, project: String, file: ReviewFile, hunks: [DiffHunk])
        async
    {
        guard !loading, context == nil else { return }
        loading = true
        error = nil
        defer { loading = false }
        do {
            let context = try await service.load(project: project, file: file, hunks: hunks)
            try Task.checkCancellation()
            self.context = context
        } catch {
            if !Task.isCancelled { self.error = error.localizedDescription }
        }
    }
}
