import ConversationInterface
import DiffDocumentInterface
import Foundation
import Observation

@MainActor @Observable
final class ConversationViewModel {
    let scope: CommentDraftScope
    let path: String?
    let anchors: [SourceAnchor]
    let service: ConversationWebService
    var snapshot: ConversationSnapshot?
    var loading = false
    var error: String?

    init(
        scope: CommentDraftScope, path: String? = nil, anchors: [SourceAnchor] = [],
        service: ConversationWebService
    ) {
        self.scope = scope
        self.path = path
        self.anchors = anchors
        self.service = service
    }

    var threads: [ConversationThread] {
        snapshot?.threads(path: path, anchors: anchors, revision: scope.revision) ?? []
    }

    func load() async {
        guard !loading else { return }
        loading = true
        defer { loading = false }
        do {
            let response = try await service.load(scope: scope)
            try Task.checkCancellation()
            snapshot = response
            error = nil
        } catch is CancellationError {
        } catch { self.error = error.localizedDescription }
    }
}
