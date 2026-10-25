import ConversationInterface
import Foundation
import SwiftData

struct FeedbackAttempt: Codable {
    let drafts: [FeedbackDraft]
    // Missing in records saved before outcome selection; those deliveries used comment.
    var verdict: FeedbackVerdict?
    let summary: String
    let cursor: UInt64
    let hubDrafts: [FeedbackComment]
    let hubReplies: [FeedbackReply]
    var uploaded: [UUID: String] = [:]
    var reviewStarted = false
    var target: String?
    var approval: ApprovalOutcome?
    var confirmed = false
}

struct FeedbackRecord: Codable {
    var summary = ""
    var verdict: FeedbackVerdict?
    var attempt: FeedbackAttempt?
}

@MainActor
protocol FeedbackOutbox {
    func load(scope: CommentDraftScope) throws -> FeedbackRecord
    func save(_ record: FeedbackRecord, scope: CommentDraftScope) throws
}

@MainActor
final class SwiftDataFeedbackOutbox: FeedbackOutbox {
    private let container: ModelContainer
    private let context: ModelContext

    init(configuration: ModelConfiguration = ModelConfiguration("ReviewSubmission")) throws {
        container = try ModelContainer(for: StoredFeedbackRecord.self, configurations: configuration)
        context = ModelContext(container)
        context.autosaveEnabled = false
    }

    func load(scope: CommentDraftScope) throws -> FeedbackRecord {
        guard let stored = try record(scope) else { return FeedbackRecord() }
        return try JSONDecoder().decode(FeedbackRecord.self, from: stored.payload)
    }

    func save(_ value: FeedbackRecord, scope: CommentDraftScope) throws {
        let payload = try JSONEncoder().encode(value)
        if let existing = try record(scope) {
            existing.payload = payload
        } else {
            context.insert(StoredFeedbackRecord(scope: key(scope), payload: payload))
        }
        do { try context.save() } catch {
            context.rollback()
            throw error
        }
    }

    private func record(_ scope: CommentDraftScope) throws -> StoredFeedbackRecord? {
        let key = key(scope)
        var descriptor = FetchDescriptor<StoredFeedbackRecord>(predicate: #Predicate { $0.scope == key })
        descriptor.fetchLimit = 1
        return try context.fetch(descriptor).first
    }

    private func key(_ scope: CommentDraftScope) -> String {
        var parts = URLComponents(url: scope.hub, resolvingAgainstBaseURL: false)!
        parts.host = parts.host?.lowercased()
        if (parts.scheme == "http" && parts.port == 80) || (parts.scheme == "https" && parts.port == 443) {
            parts.port = nil
        }
        while parts.path.hasSuffix("/") { parts.path.removeLast() }
        return "\(parts.string!)\n\(scope.item)\n\(scope.revision)"
    }
}
