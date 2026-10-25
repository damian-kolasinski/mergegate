import ConversationInterface
import DiffDocumentInterface
import Foundation
import Networking
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
    var isReconnecting = false
    let events: ServerEvents
    private var loadVersion = 0

    init(
        scope: CommentDraftScope, path: String? = nil, anchors: [SourceAnchor] = [],
        service: ConversationWebService, events: ServerEvents = .shared
    ) {
        self.scope = scope
        self.path = path
        self.anchors = anchors
        self.service = service
        self.events = events
    }

    var editingService: ConversationEditingService {
        .init(scope: scope, transport: service.transport)
    }

    var threads: [ConversationThread] {
        snapshot?.threads(path: path, anchors: anchors, revision: scope.revision) ?? []
    }

    func load() async {
        loadVersion += 1
        let version = loadVersion
        loading = true
        defer { if version == loadVersion { loading = false } }
        do {
            let response = try await service.load(scope: scope)
            try Task.checkCancellation()
            guard version == loadVersion else { return }
            snapshot = response
            error = nil
        } catch {
            if !Task.isCancelled && version == loadVersion {
                self.error = error.localizedDescription
            }
        }
    }
}
