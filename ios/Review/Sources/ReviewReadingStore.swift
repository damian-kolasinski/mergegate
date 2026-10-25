import Foundation
import SwiftData

struct ReviewReadingPosition: Codable, Sendable {
    var revision: UInt32
    var since: UInt32?
    var path: String?
    var offset: Double = 0
    var contextLoaded = false
    var revealed: [Int: Int] = [:]
}

@Model
final class StoredReviewPosition {
    @Attribute(.unique) var key: String
    var data: Data

    init(key: String, data: Data) {
        self.key = key
        self.data = data
    }
}

@MainActor
final class ReviewReadingStore {
    private let container: ModelContainer
    private let context: ModelContext

    init(configuration: ModelConfiguration = ModelConfiguration("ReviewPosition")) throws {
        container = try ModelContainer(for: StoredReviewPosition.self, configurations: configuration)
        context = ModelContext(container)
        context.autosaveEnabled = false
    }

    func load(hub: URL, item: String) throws -> ReviewReadingPosition? {
        let key = "\(hub.absoluteString)|\(item)"
        return try record(key).map {
            try JSONDecoder().decode(ReviewReadingPosition.self, from: $0.data)
        }
    }

    func save(_ value: ReviewReadingPosition, hub: URL, item: String) throws {
        let key = "\(hub.absoluteString)|\(item)"
        let data = try JSONEncoder().encode(value)
        do {
            if let existing = try record(key) {
                existing.data = data
            } else {
                context.insert(StoredReviewPosition(key: key, data: data))
            }
            try context.save()
        } catch {
            context.rollback()
            throw error
        }
    }

    private func record(_ key: String) throws -> StoredReviewPosition? {
        try context.fetch(
            FetchDescriptor<StoredReviewPosition>(predicate: #Predicate { $0.key == key })
        ).first
    }
}
