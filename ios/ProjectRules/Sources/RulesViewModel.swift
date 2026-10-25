import Foundation
import Networking
import Observation

@MainActor @Observable
final class RulesViewModel {
    let service: RulesWebService
    var snapshot: RulesSnapshot?
    var error: String?
    private(set) var busy = false
    private var version = 0
    init(service: RulesWebService) { self.service = service }

    func load() async {
        guard !busy else { return }
        version += 1
        let request = version
        do {
            let next = try await service.load()
            try Task.checkCancellation()
            guard request == version else { return }
            snapshot = next
            error = nil
        } catch {
            if !Task.isCancelled && request == version { self.error = error.localizedDescription }
        }
    }

    func decide(_ decision: RulesDecision) async {
        guard !busy else { return }
        busy = true
        version += 1
        defer { busy = false }
        do {
            snapshot = try await service.decide(decision)
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    func follow() async {
        let stream = await ServerEvents.shared.subscribe(
            service.hub.appending(path: "api/events/ui"))
        await load()
        for await event in stream {
            guard !Task.isCancelled else { return }
            switch event {
            case .connected: await load()
            case .disconnected: break
            case .data(let data):
                struct Invalidation: Decodable {
                    let type: String
                    let project: String?
                }
                if let value = try? JSONDecoder().decode(Invalidation.self, from: Data(data.utf8)),
                    value.type == "rules.changed",
                    value.project == nil || value.project == service.project
                {
                    await load()
                }
            }
        }
    }
}
