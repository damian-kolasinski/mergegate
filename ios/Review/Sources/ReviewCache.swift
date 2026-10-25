import CryptoKit
import Foundation

// Only Review reads these files. Mutable snapshots are never used to authorize a write.
actor ReviewCache {
    let directory: URL
    private(set) var failure: String?

    init(directory: URL = URL.applicationSupportDirectory.appending(path: "ReviewCache")) {
        self.directory = directory
    }

    func read(_ key: String) -> (data: Data, date: Date)? {
        let file = location(key)
        guard let data = try? Data(contentsOf: file),
            let values = try? file.resourceValues(forKeys: [.contentModificationDateKey]),
            let date = values.contentModificationDate
        else { return nil }
        return (data, date)
    }

    func save(_ data: Data, key: String) {
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: location(key), options: .atomic)
            failure = nil
        } catch {
            failure = "Couldn’t save this review for offline use. \(error.localizedDescription)"
        }
    }

    func markViewed(_ change: ReviewViewedChange, key: String) {
        guard let saved = read(key),
            var object = try? JSONSerialization.jsonObject(with: saved.data) as? [String: Any],
            var files = object["files"] as? [[String: Any]],
            let index = files.firstIndex(where: { $0["path"] as? String == change.path })
        else { return }
        files[index]["viewed"] = change.viewed
        object["files"] = files
        guard let data = try? JSONSerialization.data(withJSONObject: object) else { return }
        save(data, key: key)
    }

    func snapshotKey(hub: URL, item: String, revision: UInt32, since: UInt32?) -> String {
        "\(hub.absoluteString)|\(item)|r\(revision)|\(since.map(String.init) ?? "all")"
    }

    private func location(_ key: String) -> URL {
        let digest = SHA256.hash(data: Data(key.utf8)).map { String(format: "%02x", $0) }.joined()
        return directory.appending(component: digest)
    }
}
