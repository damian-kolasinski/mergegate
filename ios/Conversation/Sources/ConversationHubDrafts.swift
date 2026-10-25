import SwiftUI

struct ConversationHubDrafts: View {
    let model: ConversationViewModel
    @State private var editing: ThreadComment?
    @State private var deleting: ThreadComment?
    @State private var busy = false
    @State private var error: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(
                (model.snapshot?.drafts ?? []).filter {
                    model.path == nil || $0.filePath == model.path
                },
                id: \.id
            ) { draft in
                VStack(alignment: .leading, spacing: 8) {
                    Text("Hub draft · r\(draft.revision)").appearanceText(Appearance.rowHeading)
                    Text(draft.path ?? "Whole item").appearanceText(Appearance.caption)
                    if let placement = draft.placement, placement.kind != "moved" {
                        Text(
                            "\(placement.kind.capitalized) anchor · Re-place or delete before sending"
                        )
                        .foregroundStyle(Appearance.error)
                    }
                    Text(draft.body).appearanceText(Appearance.body)
                    HStack {
                        Button("Edit / re-place") { editing = draft }.frame(minHeight: 44)
                        Spacer()
                        Button("Delete", role: .destructive) { deleting = draft }.frame(
                            minHeight: 44)
                    }.disabled(busy)
                }.padding(20)
                Divider()
            }
            if let error { Text(error).foregroundStyle(Appearance.error).padding(20) }
        }
        .sheet(isPresented: Binding(get: { editing != nil }, set: { if !$0 { editing = nil } })) {
            if let draft = editing {
                ConversationEditorScene(
                    model: ConversationEditorModel(
                        kind: .comment(draft), text: draft.body,
                        service: model.editingService)
                ) {
                    editing = nil
                    Task { await model.load() }
                }
            }
        }
        .confirmationDialog(
            "Delete this hub draft?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible
        ) {
            if let draft = deleting {
                Button("Delete draft", role: .destructive) {
                    Task {
                        busy = true
                        defer { busy = false }
                        do {
                            let current = try await model.editingService.current()
                            if (current.drafts ?? []).contains(where: { $0.id == draft.id }) {
                                try await model.editingService.removeComment(draft.id)
                            }
                            error = nil
                            await model.load()
                        } catch { self.error = error.localizedDescription }
                        deleting = nil
                    }
                }
            }
        }
    }
}
