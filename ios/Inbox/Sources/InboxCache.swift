import CryptoKit
import Foundation

actor InboxCache {
    let directory: URL
    init(directory: URL = URL.applicationSupportDirectory.appending(path: "InboxCache")) {
        self.directory = directory
    }

    func save(_ snapshot: InboxSnapshot, hub: URL) {
        let saved = Saved(snapshot: snapshot, date: .now)
        guard let data = try? JSONEncoder().encode(saved) else { return }
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: location(hub), options: .atomic)
        } catch { /* A missing cache leaves the normal reconnect screen available. */  }
    }

    func load(hub: URL) -> Saved? {
        guard let data = try? Data(contentsOf: location(hub)) else { return nil }
        return try? JSONDecoder().decode(Saved.self, from: data)
    }

    struct Saved: Codable, Sendable {
        let snapshot: InboxSnapshot
        let date: Date
    }

    private func location(_ hub: URL) -> URL {
        let digest = SHA256.hash(data: Data(hub.absoluteString.utf8))
            .map { String(format: "%02x", $0) }.joined()
        return directory.appending(component: digest)
    }
}
