import SwiftUI

struct CommentDraftEditor: View {
    @Bindable var model: CommentDraftViewModel
    let draft: CommentDraft

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text(draft.location.path).appearanceText(Appearance.label)
                    .accessibilityLabel("File \(draft.location.path)")
                Text(draft.location.label).appearanceText(Appearance.caption)
                    .foregroundStyle(Appearance.secondary).accessibilityIdentifier("draftAnchor")
                Text(draft.location.anchor.quote)
                    .font(.system(.body, design: .monospaced)).textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(12)
                    .background(Appearance.surface).accessibilityIdentifier("draftQuote")
                    .accessibilityLabel("Source quote: \(draft.location.anchor.quote)")
                Text("Your feedback").appearanceText(Appearance.label)
                TextEditor(text: $model.body).appearanceText(Appearance.body)
                    .scrollContentBackground(.hidden).frame(minHeight: 160)
                    .padding(8).background(Appearance.surface)
                    .accessibilityLabel("Your feedback").accessibilityIdentifier("draftBody")
                Text(saveStatus)
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    .accessibilityIdentifier("draftSaveStatus")
            }.padding(20)
        }
    }

    private var saveStatus: String {
        if model.hasUnsavedChanges { return "Changes haven’t been saved" }
        return model.drafts.contains(where: { $0.id == draft.id })
            ? "Saved on this iPhone · Not sent" : "Type feedback to save a draft"
    }
}
