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
    var selectedPath: String? {
        didSet {
            guard selectedPath != oldValue else { return }
            readingOffset = 0
            contextLoaded = false
            revealed = [:]
            rememberPosition()
        }
    }
    var readingOffset: Double = 0
    var contextLoaded = false
    var revealed: [Int: Int] = [:]
    var positionFailure: String?
    private(set) var cachedAt: Date?
    private let readingStore: ReviewReadingStore?
    private var savedPosition: ReviewReadingPosition?
    var cacheFailure: String?
    private(set) var isComparing = false
    private(set) var comparisonFailure: String?
    private(set) var isSavingViewed = false
    private(set) var viewedFailure: ReviewViewedFailure?
    var sourceService: ReviewSourceService {
        .init(transport: service.transport, url: service.url, cache: service.cache)
    }
    let events: ServerEvents
    var currentDetails: ReviewSnapshot?
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
    let reviewTime: ReviewTimeModel
    let service: ReviewWebService
    let item: String

    init(
        service: ReviewWebService, item: String, events: ServerEvents = .shared,
        readingStore: ReviewReadingStore? = nil
    ) {
        self.reviewTime = ReviewTimeModel(service: service, item: item)
        self.service = service
        self.item = item
        self.events = events
        self.readingStore = readingStore
        do { savedPosition = try readingStore?.load(hub: service.url, item: item) } catch {
            positionFailure =
                "Couldn’t restore your reading position. \(error.localizedDescription)"
        }
    }

    func load(allowCached: Bool = true) async {
        guard !isComparing, !isSavingViewed else { return }
        refreshVersion += 1
        rememberPosition()
        openingVersion += 1
        let version = openingVersion
        let openingPosition = savedPosition
        state = .loading
        viewedFailure = nil
        do {
            var snapshot = try await service.load(item: item)
            let since: UInt32?
            if let position = openingPosition, position.revision == snapshot.item.revision.number,
                position.since.map(snapshot.interdiffs.contains) ?? true
            {
                since = position.since
            } else {
                since = snapshot.last_reviewed.flatMap {
                    $0 < snapshot.item.revision.number && snapshot.interdiffs.contains($0)
                        ? $0 : nil
                }
            }
            if let since {
                let comparison = try await service.load(item: item, since: since)
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
            readingOffset = 0
            contextLoaded = false
            revealed = [:]
            state = .loaded(snapshot)
            currentDetails = snapshot
            savedPosition = openingPosition
            restorePosition(snapshot)
            cachedAt = nil
            latestRevision = snapshot.item.revision.number
            latestState = snapshot.item.state
            refreshFailure = nil
            cacheFailure = await service.cache?.failure
            rememberPosition()
        } catch {
            guard !Task.isCancelled, version == openingVersion else { return }
            if allowCached, error is URLError, let position = savedPosition,
                let (snapshot, date) = await service.cached(item: item, position: position),
                version == openingVersion, !Task.isCancelled
            {
                state = .loaded(snapshot)
                currentDetails = snapshot
                selectedPath = position.path
                savedPosition = openingPosition
                restorePosition(snapshot)
                cachedAt = date
                refreshFailure = nil
            } else {
                state = .failed(error.localizedDescription)
            }
        }
    }

    func compare(since: UInt32?) async {
        guard !isComparing, !isSavingViewed, case .loaded(let before) = state,
            since != before.since,
            since.map(before.interdiffs.contains) ?? true
        else { return }
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
                selectedPath =
                    snapshot.reviewFiles.first(where: { !$0.viewed })?.path
                    ?? snapshot.files.first?.path
            }
            viewedFailure = nil
            state = .loaded(snapshot)
            readingOffset = 0
            contextLoaded = false
            revealed = [:]
            cachedAt = nil
            rememberPosition()
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
            currentDetails = current
            latestRevision = current.item.revision.number
            latestState = current.item.state
            refreshFailure = nil
            cachedAt = nil
            // Preserve the immutable diff and reader identity. Only progress for this same revision changes.
            guard current.item.revision.number == before.item.revision.number,
                !isSavingViewed, viewedVersion == version,
                case .loaded(var displayed) = state, displayed.since == before.since,
                displayed.item.revision.number == before.item.revision.number
            else { return }
            displayed.files = current.files
            state = .loaded(displayed)
        } catch {
            if !Task.isCancelled && request == refreshVersion {
                refreshFailure = error.localizedDescription
            }
        }
    }

    func setViewed(path: String, viewed: Bool) async {
        guard cachedAt == nil, case .loaded(let snapshot) = state,
            let file = snapshot.files.first(where: { $0.path == path }),
            file.tier == .review, file.interdiff != .dropped, file.viewed != viewed
        else { return }
        await updateViewed(
            .init(revision: snapshot.item.revision.number, path: path, viewed: viewed))
    }

    func retryViewed() async {
        guard let failure = viewedFailure else { return }
        await updateViewed(failure.change)
    }

    private func updateViewed(_ change: ReviewViewedChange) async {
        guard !isSavingViewed, !isComparing, case .loaded(let before) = state,
            before.item.revision.number == change.revision
        else { return }
        isSavingViewed = true
        viewedVersion += 1
        viewedFailure = nil
        defer { isSavingViewed = false }
        do {
            try await service.markViewed(item: item, change: change)
            guard case .loaded(var snapshot) = state,
                snapshot.item.revision.number == change.revision,
                let index = snapshot.files.firstIndex(where: { $0.path == change.path })
            else { return }
            snapshot.files[index].viewed = change.viewed
            state = .loaded(snapshot)
            if let cache = service.cache {
                await cache.markViewed(
                    change,
                    key: cache.snapshotKey(
                        hub: service.url, item: item, revision: change.revision,
                        since: snapshot.since))
            }
        } catch {
            if !Task.isCancelled {
                viewedFailure = .init(change: change, message: error.localizedDescription)
            }
        }
    }

    func rememberPosition() {
        guard case .loaded(let snapshot) = state else { return }
        let position = ReviewReadingPosition(
            revision: snapshot.item.revision.number, since: snapshot.since, path: selectedPath,
            offset: readingOffset, contextLoaded: contextLoaded, revealed: revealed)
        savedPosition = position
        guard let readingStore else { return }
        do {
            try readingStore.save(position, hub: service.url, item: item)
            positionFailure = nil
        } catch {
            positionFailure = "Couldn’t save your reading position. \(error.localizedDescription)"
        }
    }

    private func restorePosition(_ snapshot: ReviewSnapshot) {
        guard let position = savedPosition, position.revision == snapshot.item.revision.number,
            position.since == snapshot.since,
            snapshot.files.contains(where: { $0.path == position.path })
        else { return }
        selectedPath = position.path
        readingOffset = position.offset
        contextLoaded = position.contextLoaded
        revealed = position.revealed
    }

}
