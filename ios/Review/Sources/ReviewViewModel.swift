import Foundation
import Networking
import Observation

@MainActor @Observable
final class ReviewViewModel {
    enum State {
        case loading
        case failed(String)
        case loaded(ReviewSnapshot)
    }
    private(set) var state: State = .loading
    var selectedPath: String?
    var sourceService: ReviewSourceService { .init(transport: service.transport, url: service.url) }
    let events = ServerEvents()
    var highlightService: ReviewHighlightService { .init(web: service, events: events) }
    private let service: ReviewWebService
    private let item: String

    init(service: ReviewWebService, item: String) {
        self.service = service
        self.item = item
    }

    func load() async {
        state = .loading
        do {
            let snapshot = try await service.load(item: item)
            try Task.checkCancellation()
            selectedPath =
                snapshot.files.first(where: { $0.tier == .review })?.path ?? snapshot.files.first?.path
            state = .loaded(snapshot)
        } catch {
            if !Task.isCancelled { state = .failed(error.localizedDescription) }
        }
    }
}
