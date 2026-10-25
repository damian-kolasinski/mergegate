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
    private(set) var isSavingViewed = false
    private(set) var viewedFailure: ReviewViewedFailure?
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
        viewedFailure = nil
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

    func setViewed(path: String, viewed: Bool) async {
        guard case .loaded(let snapshot) = state,
            let file = snapshot.files.first(where: { $0.path == path }),
            file.tier == .review, file.viewed != viewed else { return }
        await updateViewed(.init(revision: snapshot.item.revision.number, path: path, viewed: viewed))
    }

    func retryViewed() async {
        guard let failure = viewedFailure else { return }
        await updateViewed(failure.change)
    }

    private func updateViewed(_ change: ReviewViewedChange) async {
        guard !isSavingViewed, case .loaded(let before) = state,
            before.item.revision.number == change.revision else { return }
        isSavingViewed = true
        viewedFailure = nil
        defer { isSavingViewed = false }
        do {
            try await service.markViewed(item: item, change: change)
            guard case .loaded(var snapshot) = state,
                snapshot.item.revision.number == change.revision,
                let index = snapshot.files.firstIndex(where: { $0.path == change.path }) else { return }
            snapshot.files[index].viewed = change.viewed
            state = .loaded(snapshot)
        } catch {
            if !Task.isCancelled {
                viewedFailure = .init(change: change, message: error.localizedDescription)
            }
        }
    }
}
