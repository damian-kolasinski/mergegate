import Foundation
import HubConnectionInterface

@MainActor
struct HubSelection {
    let defaults: UserDefaults
    private let key = "selectedHubURL"

    var url: URL? {
        defaults.string(forKey: key).flatMap { try? HubAddress($0).url }
    }

    var hub: ConnectedHub? {
        guard let data = defaults.data(forKey: "selectedHubIdentity"),
            let hub = try? JSONDecoder().decode(ConnectedHub.self, from: data), hub.url == url
        else { return nil }
        return hub
    }

    func save(_ hub: ConnectedHub) {
        defaults.set(hub.url.absoluteString, forKey: key)
        defaults.set(try? JSONEncoder().encode(hub), forKey: "selectedHubIdentity")
    }
    func forget() {
        defaults.removeObject(forKey: key)
        defaults.removeObject(forKey: "selectedHubIdentity")
    }
}
