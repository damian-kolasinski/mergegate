import Foundation

@MainActor
struct HubSelection {
    let defaults: UserDefaults
    private let key = "selectedHubURL"

    var url: URL? {
        defaults.string(forKey: key).flatMap { try? HubAddress($0).url }
    }

    func save(_ url: URL) { defaults.set(url.absoluteString, forKey: key) }
    func forget() { defaults.removeObject(forKey: key) }
}
