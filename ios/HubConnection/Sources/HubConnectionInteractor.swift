import Foundation
import HubConnectionInterface

@MainActor
struct HubConnectionInteractor {
    let service: HubConnectionWebService
    let selection: HubSelection

    var selectedURL: URL? { selection.url }

    func connect(to url: URL) async throws -> ConnectedHub {
        let hub = try await service.connect(to: url)
        try Task.checkCancellation()
        selection.save(url)
        return hub
    }

    func forget() { selection.forget() }
}
