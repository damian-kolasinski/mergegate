import Foundation
import Networking
import Observation

@MainActor @Observable
final class ReviewActionsModel {
    let service: ReviewWebService
    let item: String
    private(set) var snapshot: ReviewSnapshot?
    private(set) var busy = false
    var priority: String? = nil
    var note = ""
    var reason = ""
    var error: String?
    var result: String?
    /// GitHub couldn't close the pull request, as when the repository is gone;
    /// the operator may abandon the item in mergegate only instead.
    private(set) var githubUnreachable = false
    private var version = 0
    private let beforeLeaving: () async -> Void

    init(service: ReviewWebService, item: String, beforeLeaving: @escaping () async -> Void = {}) {
        self.service = service
        self.item = item
        self.beforeLeaving = beforeLeaving
    }

    func load() async {
        guard !busy else { return }
        version += 1
        let request = version
        do {
            let value = try await service.load(item: item)
            try Task.checkCancellation()
            guard request == version else { return }
            if snapshot == nil {
                priority = value.item.priority
                note = value.item.note ?? ""
            }
            snapshot = value
            error = nil
            githubUnreachable = false
        } catch {
            if !Task.isCancelled && request == version { self.error = error.localizedDescription }
        }
    }

    enum Action { case priority, sync, abandon, abandonLeavingGithub, retryMerge, cancelMerge }
    func send(_ action: Action) async {
        guard !busy, snapshot != nil else { return }
        busy = true
        version += 1
        error = nil
        result = nil
        githubUnreachable = false
        defer { busy = false }
        do {
            // Item state can change while a confirmation is open. Reconcile before another attempt.
            let current = try await service.load(item: item)
            snapshot = current
            let endpoint = service.url.appending(path: "api/items").appending(component: item)
            switch action {
            case .priority:
                struct Body: Encodable {
                    let priority: String?
                    let note: String?
                }
                let note = note.trimmingCharacters(in: .whitespacesAndNewlines)
                let data = try await service.transport.post(
                    endpoint.appending(path: "priority"),
                    body: JSONEncoder().encode(
                        Body(priority: priority, note: note.isEmpty ? nil : note)))
                let saved = try JSONDecoder().decode(ReviewSnapshot.Item.self, from: data)
                guard saved.id == item, saved.priority == priority,
                    saved.note == (note.isEmpty ? nil : note)
                else {
                    throw ReviewActionIssue(
                        message: "The hub did not confirm this priority. Refresh before retrying.")
                }
                snapshot?.item = saved
                result = "Priority saved"
            case .sync:
                struct Synced: Decodable {
                    let new: UInt32
                    let edited: UInt32
                    let released: UInt32
                }
                let value = try await JSONDecoder().decode(
                    Synced.self,
                    from: service.transport.post(
                        endpoint.appending(path: "sync"), body: Data("{}".utf8)))
                result =
                    "\(value.new) new, \(value.edited) edited, \(value.released) sent to the agent"
            case .abandon, .abandonLeavingGithub:
                if current.item.isClosed {
                    result = "Item already \(current.item.state)"
                    return
                }
                await beforeLeaving()
                struct Body: Encodable {
                    let source = "operator"
                    let reason: String?
                    let leave_github: Bool
                }
                let reason = reason.trimmingCharacters(in: .whitespacesAndNewlines)
                let body = Body(
                    reason: reason.isEmpty ? nil : reason,
                    leave_github: action == .abandonLeavingGithub)
                let saved: ReviewSnapshot.Item
                do {
                    saved = try await JSONDecoder().decode(
                        ReviewSnapshot.Item.self,
                        from: service.transport.post(
                            endpoint.appending(path: "abandon"), body: JSONEncoder().encode(body)))
                } catch let failure as HTTPFailure where failure.code == "github_no_access" {
                    githubUnreachable = true
                    throw failure
                }
                guard saved.id == item, saved.state == "abandoned" else {
                    throw ReviewActionIssue(
                        message: "Abandon was not confirmed. Refresh before retrying.")
                }
                snapshot?.item = saved
                result = "Abandoned"
            case .retryMerge:
                guard current.blocked?.kind == "held" else {
                    throw ReviewActionIssue(
                        message: "The merge is no longer held. Its current status is shown above.")
                }
                struct Merge: Decodable {
                    let item: ReviewSnapshot.Item
                    let outcome: ReviewBlocked
                }
                let value = try await JSONDecoder().decode(
                    Merge.self,
                    from: service.transport.post(
                        endpoint.appending(path: "merge"), body: Data("{}".utf8)))
                guard value.item.id == item else { throw URLError(.badServerResponse) }
                snapshot?.item = value.item
                snapshot?.blocked = value.outcome.kind == "merged" ? nil : value.outcome
                result =
                    value.outcome.kind == "merged"
                    ? "Merged into \(value.item.target)" : value.outcome.label
            case .cancelMerge:
                guard ["held", "waiting", "behind"].contains(current.blocked?.kind ?? "") else {
                    throw ReviewActionIssue(message: "There is no pending merge to cancel.")
                }
                _ = try await service.transport.delete(endpoint.appending(path: "merge"))
                snapshot = try await service.load(item: item)
                result = "Merge cancelled"
            }
        } catch { self.error = error.localizedDescription }
    }
}

private struct ReviewActionIssue: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
