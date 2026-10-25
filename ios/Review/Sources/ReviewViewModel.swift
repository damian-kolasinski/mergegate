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
    private(set) var isComparing = false
    private(set) var comparisonFailure: String?
    private(set) var isSavingViewed = false
    private(set) var viewedFailure: ReviewViewedFailure?
    var sourceService: ReviewSourceService { .init(transport: service.transport, url: service.url) }
    let events: ServerEvents
    var latestRevision: UInt32?
    var latestState: String?
    var refreshFailure: String?
    var isReconnecting = false
    var viewedVersion = 0
    private var openingVersion = 0
    private var refreshVersion = 0
    var highlightService: ReviewHighlightService {
        let since: UInt32?
        if case .loaded(let snapshot) = state { since = snapshot.since } else { since = nil }
        return .init(web: service, events: events, since: since)
    }
    let service: ReviewWebService
    let item: String

    init(service: ReviewWebService, item: String, events: ServerEvents = .shared) {
        self.service = service
        self.item = item
        self.events = events
    }

    func load() async {
        openingVersion += 1
        let version = openingVersion
        state = .loading
        viewedFailure = nil
        do {
            var snapshot = try await service.load(item: item)
            if let reviewed = snapshot.last_reviewed, reviewed < snapshot.item.revision.number,
                snapshot.interdiffs.contains(reviewed) {
                let comparison = try await service.load(item: item, since: reviewed)
                guard comparison.item.revision.number == snapshot.item.revision.number else {
                    throw ReviewComparisonChanged()
                }
                snapshot = comparison
            }
            try Task.checkCancellation()
            guard version == openingVersion else { return }
            selectedPath =
                snapshot.reviewFiles.first(where: {
                    snapshot.since == nil || (!$0.viewed && $0.interdiff != .unchanged)
                })?.path
                ?? snapshot.reviewFiles.first?.path ?? snapshot.files.first?.path
            state = .loaded(snapshot)
        } catch {
            if !Task.isCancelled && version == openingVersion { state = .failed(error.localizedDescription) }
        }
    }

    func compare(since: UInt32?) async {
        guard !isComparing, !isSavingViewed, case .loaded(let before) = state,
            since != before.since,
            since.map(before.interdiffs.contains) ?? true else { return }
        isComparing = true
        comparisonFailure = nil
        defer { isComparing = false }
        do {
            let snapshot = try await service.load(item: item, since: since)
            try Task.checkCancellation()
            guard snapshot.item.revision.number == before.item.revision.number else {
                throw ReviewComparisonChanged()
            }
            if !snapshot.files.contains(where: { $0.path == selectedPath }) {
                selectedPath = snapshot.reviewFiles.first(where: { !$0.viewed })?.path
                    ?? snapshot.files.first?.path
            }
            viewedFailure = nil
            state = .loaded(snapshot)
        } catch {
            if !Task.isCancelled { comparisonFailure = error.localizedDescription }
        }
    }

    func refreshCurrent() async {
        guard case .loaded(let before) = state else {
            await load()
            return
        }
        refreshVersion += 1
        let request = refreshVersion
        let version = viewedVersion
        do {
            let current = try await service.load(item: item, since: before.since)
            try Task.checkCancellation()
            guard request == refreshVersion else { return }
            latestRevision = current.item.revision.number
            latestState = current.item.state
            refreshFailure = nil
            // Preserve the immutable diff and reader identity. Only progress for this same revision changes.
            guard current.item.revision.number == before.item.revision.number,
                !isSavingViewed, viewedVersion == version,
                case .loaded(var displayed) = state, displayed.since == before.since
            else { return }
            for index in displayed.files.indices {
                if let file = current.files.first(where: { $0.path == displayed.files[index].path }) {
                    displayed.files[index].viewed = file.viewed
                }
            }
            state = .loaded(displayed)
        } catch {
            if !Task.isCancelled && request == refreshVersion { refreshFailure = error.localizedDescription }
        }
    }

    func setViewed(path: String, viewed: Bool) async {
        guard case .loaded(let snapshot) = state,
            let file = snapshot.files.first(where: { $0.path == path }),
            file.tier == .review, file.interdiff != .dropped, file.viewed != viewed else { return }
        await updateViewed(.init(revision: snapshot.item.revision.number, path: path, viewed: viewed))
    }

    func retryViewed() async {
        guard let failure = viewedFailure else { return }
        await updateViewed(failure.change)
    }

    private func updateViewed(_ change: ReviewViewedChange) async {
        guard !isSavingViewed, !isComparing, case .loaded(let before) = state,
            before.item.revision.number == change.revision else { return }
        isSavingViewed = true
        viewedVersion += 1
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
