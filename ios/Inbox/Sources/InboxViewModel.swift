import Foundation
import Observation

@MainActor @Observable
final class InboxViewModel {
    private(set) var snapshot: InboxSnapshot?
    private(set) var isLoading = false
    private(set) var error: String?
    var project: String?
    var filter: InboxFilter = .all
    private(set) var generation = 0
    private(set) var triage: [String: InboxTriage] = [:]
    private(set) var triageErrors: [String: String] = [:]
    var groups: [InboxGroup] { snapshot?.groups(project: project, filter: filter) ?? [] }
    private let service: InboxWebService

    init(service: InboxWebService) { self.service = service }

    func loadTriage(_ item: InboxItem) async {
        guard !item.isClosed, triage[item.id] == nil else { return }
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
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            let snapshot = try await service.load()
            try Task.checkCancellation()
            self.snapshot = snapshot
            if let project, !snapshot.projects.contains(project) { self.project = nil }
            triage = [:]
            triageErrors = [:]
            generation += 1
            error = nil
        } catch {
            if !Task.isCancelled { self.error = error.localizedDescription }
        }
    }
}
