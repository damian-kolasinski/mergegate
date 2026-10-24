import Foundation
import Observation

@MainActor @Observable
final class InboxViewModel {
    private(set) var snapshot: InboxSnapshot?
    private(set) var isLoading = false
    private(set) var error: String?
    private let service: InboxWebService

    init(service: InboxWebService) { self.service = service }

    func refresh() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            let snapshot = try await service.load()
            try Task.checkCancellation()
            self.snapshot = snapshot
            error = nil
        } catch {
            if !Task.isCancelled { self.error = error.localizedDescription }
        }
    }
}
