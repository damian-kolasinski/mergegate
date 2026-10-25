import Foundation
import HubConnectionInterface
import Observation

@MainActor @Observable
final class HubConnectionViewModel {
    enum State {
        case disconnected
        case connecting
        case connected(ConnectedHub)
    }
    var address: String
    private(set) var state: State = .disconnected
    private(set) var error: String?
    private let interactor: HubConnectionInteractor

    init(interactor: HubConnectionInteractor) {
        self.interactor = interactor
        address = interactor.selectedURL?.absoluteString ?? ""
    }

    func restore() async {
        if let hub = interactor.selectedHub {
            state = .connected(hub)
            return
        }
        guard interactor.selectedURL != nil else { return }
        await connect()
    }

    func connect() async {
        guard case .disconnected = state else { return }
        error = nil
        do {
            let url = try HubAddress(address).url
            state = .connecting
            let hub = try await interactor.connect(to: url)
            state = .connected(hub)
        } catch {
            state = .disconnected
            if !Task.isCancelled { self.error = error.localizedDescription }
        }
    }

    func forget() {
        interactor.forget()
        state = .disconnected
        address = ""
        error = nil
    }
}
