import ConversationInterface
import SwiftUI

struct CommentDraftScene: View {
    let scope: CommentDraftScope
    let location: CommentDraftLocation?
    /// An existing draft to edit, as from its card in the reader.
    var draftID: UUID? = nil
    let close: () -> Void
    @State private var model: CommentDraftViewModel?
    @State private var failure: String?

    var body: some View {
        Group {
            if let model {
                CommentDraftsView(model: model, close: close)
            } else if let failure {
                VStack(spacing: 16) {
                    ContentUnavailableView(
                        "Couldn’t open drafts", systemImage: "externaldrive.badge.exclamationmark",
                        description: Text(failure))
                    Button("Retry", action: open).frame(minHeight: 44)
                    Button("Close", action: close).frame(minHeight: 44)
                }
            } else {
                ProgressView("Opening drafts…")
            }
        }
        .background(Appearance.page).foregroundStyle(Appearance.primary).tint(Appearance.brand)
        .task { open() }
    }

    private func open() {
        do {
            let model = try CommentDraftViewModel(
                scope: scope, store: CommentDraftStore(), location: location)
            if let draftID, let draft = model.drafts.first(where: { $0.id == draftID }) {
                model.edit(draft)
            }
            self.model = model
            failure = nil
        } catch { failure = error.localizedDescription }
    }
}
