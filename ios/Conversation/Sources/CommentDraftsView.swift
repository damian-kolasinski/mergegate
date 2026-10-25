import SwiftUI

struct CommentDraftsView: View {
    @Bindable var model: CommentDraftViewModel
    let close: () -> Void
    @State private var confirmDelete = false
    @State private var confirmDiscard = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text(model.editing == nil ? "Comment drafts" : "Draft comment")
                    .appearanceText(Appearance.heading).accessibilityAddTraits(.isHeader)
                Spacer(minLength: 12)
                Button("Done") {
                    model.persist()
                    if !model.hasUnsavedChanges { close() }
                }
                .appearanceText(Appearance.label).frame(minWidth: 44, minHeight: 44)
                .buttonStyle(.glass).accessibilityIdentifier("closeDrafts")
            }.padding(.horizontal, 20).padding(.top, 12)
            Text("\(model.scope.name) · r\(model.scope.revision)")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                .padding(.horizontal, 20).padding(.bottom, 12).accessibilityIdentifier("draftScope")
            Divider()
            if let draft = model.editing {
                CommentDraftEditor(model: model, draft: draft)
                HStack {
                    Button("All drafts") { model.showList() }.accessibilityIdentifier("allDrafts")
                        .frame(minHeight: 44)
                    Spacer()
                    Button("Delete draft", role: .destructive) { confirmDelete = true }
                        .accessibilityIdentifier("deleteDraft")
                        .frame(minHeight: 44)
                }.appearanceText(Appearance.label).frame(minHeight: 44).padding(.horizontal, 20)
            } else {
                list
            }
            if let error = model.error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                    .padding(.horizontal, 20).accessibilityIdentifier("draftError")
                Button("Retry") { model.retry() }.frame(minHeight: 44).padding(.horizontal, 20)
                if model.hasUnsavedChanges {
                    Button("Discard unsaved changes", role: .destructive) { confirmDiscard = true }
                        .frame(minHeight: 44).padding(.horizontal, 20)
                }
            }
        }
        .padding(.bottom, 12)
        .interactiveDismissDisabled(model.hasUnsavedChanges)
        .onChange(of: model.body) { _, _ in model.persist() }
        .confirmationDialog(
            "Discard unsaved changes?", isPresented: $confirmDiscard, titleVisibility: .visible
        ) {
            Button("Discard unsaved changes", role: .destructive) { model.discardChanges() }
        }
        .confirmationDialog(
            "Delete this draft?", isPresented: $confirmDelete, titleVisibility: .visible
        ) {
            Button("Delete permanently", role: .destructive) { model.delete() }
        }
    }

    private var list: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                Text("Saved on this iPhone · Not sent")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary).padding(20)
                if model.drafts.isEmpty {
                    ContentUnavailableView(
                        "No comment drafts", systemImage: "text.bubble",
                        description: Text("Select source text and choose Comment to start a draft."))
                }
                ForEach(model.drafts) { draft in
                    Button {
                        model.edit(draft)
                    } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(draft.location.path).appearanceText(Appearance.label)
                            Text(draft.location.label).appearanceText(Appearance.caption).foregroundStyle(
                                Appearance.secondary)
                            Text(draft.body.isEmpty ? "Empty draft" : draft.body)
                                .appearanceText(Appearance.body).lineLimit(3)
                        }.frame(maxWidth: .infinity, minHeight: 44, alignment: .leading).padding(20)
                    }.buttonStyle(.plain).accessibilityIdentifier("commentDraft-\(draft.id)")
                    Divider()
                }
            }
        }.accessibilityIdentifier("draftList")
    }
}
