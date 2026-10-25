import Foundation
import Networking
import Observation

@MainActor @Observable
final class InboxViewModel {
    private(set) var snapshot: InboxSnapshot?
    private(set) var isLoading = false
    private(set) var error: String?
    var isReconnecting = false
    private(set) var cachedAt: Date?
    let cache: InboxCache?
    var project: String?
    var filter: InboxFilter = .all
    private(set) var generation = 0
    private(set) var triage: [String: InboxTriage] = [:]
    private(set) var triageErrors: [String: String] = [:]
    var groups: [InboxGroup] { snapshot?.groups(project: project, filter: filter) ?? [] }
    let service: InboxWebService
    let events: ServerEvents
    private var refreshVersion = 0

    init(service: InboxWebService, events: ServerEvents = .shared, cache: InboxCache? = nil) {
        self.service = service
        self.events = events
        self.cache = cache
    }

    func loadTriage(_ item: InboxItem) async {
        guard cachedAt == nil, !item.isClosed, triage[item.id] == nil else { return }
        let requestedGeneration = generation
        do {
            let detail = try await service.triage(item: item.id)
            try Task.checkCancellation()
            guard generation == requestedGeneration else { return }
            guard detail.item.revision.number == item.revision else {
                triageErrors[item.id] = "This item has a newer revision. Refresh the inbox."
                return
            }
            triage[item.id] = detail
            triageErrors[item.id] = nil
        } catch {
            if !Task.isCancelled && generation == requestedGeneration {
                triageErrors[item.id] = error.localizedDescription
            }
        }
    }

    func refresh() async {
        refreshVersion += 1
        let version = refreshVersion
        isLoading = true
        defer { if version == refreshVersion { isLoading = false } }
        do {
            let snapshot = try await service.load()
            try Task.checkCancellation()
            guard version == refreshVersion else { return }
            self.snapshot = snapshot
            cachedAt = nil
            await cache?.save(snapshot, hub: service.url)
            if let project, !snapshot.projects.contains(project) { self.project = nil }
            triage = [:]
            triageErrors = [:]
            generation += 1
            error = nil
        } catch {
            if !Task.isCancelled && version == refreshVersion {
                self.error = error.localizedDescription
                if self.snapshot == nil, error is URLError,
                    let saved = await cache?.load(hub: service.url), version == refreshVersion
                {
                    self.snapshot = saved.snapshot
                    cachedAt = saved.date
                }
            }
        }
    }
}
