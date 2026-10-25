import ConversationInterface
import Foundation
import Observation

@MainActor @Observable
final class CommentDraftViewModel {
    let scope: CommentDraftScope
    private let store: any CommentDraftStoring
    private(set) var drafts: [CommentDraft] = []
    private(set) var editing: CommentDraft?
    var body = ""
    private(set) var error: String?
    private var retryDeletion = false

    init(scope: CommentDraftScope, store: any CommentDraftStoring, location: CommentDraftLocation?)
        throws
    {
        self.scope = scope
        self.store = store
        drafts = try store.load(scope: scope)
        if let location { edit(CommentDraft(location: location)) }
    }

    func edit(_ draft: CommentDraft) {
        guard error == nil else { return }
        editing = draft
        body = draft.body
    }

    var hasUnsavedChanges: Bool {
        guard let editing else { return false }
        if let saved = drafts.first(where: { $0.id == editing.id }) { return saved.body != body }
        return !body.isEmpty
    }

    func persist() {
        guard var draft = editing else { return }
        guard hasUnsavedChanges else { return }
        draft.body = body
        draft.updatedAt = .now
        do {
            try store.save(draft, scope: scope)
            editing = draft
            drafts.removeAll { $0.id == draft.id }
            drafts.insert(draft, at: 0)
            error = nil
        } catch {
            retryDeletion = false
            self.error = "Couldn’t save on this iPhone. \(error.localizedDescription)"
        }
    }

    func showList() {
        persist()
        guard !hasUnsavedChanges else { return }
        error = nil
        editing = nil
    }

    func delete() {
        guard let draft = editing else { return }
        do {
            try store.delete(draft, scope: scope)
            drafts.removeAll { $0.id == draft.id }
            editing = nil
            body = ""
            error = nil
        } catch {
            retryDeletion = true
            self.error = "Couldn’t delete this draft. \(error.localizedDescription)"
        }
    }

    func discardChanges() {
        body = editing.flatMap { current in drafts.first(where: { $0.id == current.id })?.body } ?? ""
        editing = nil
        error = nil
    }

    func retry() {
        if retryDeletion { delete() } else { persist() }
    }
}
