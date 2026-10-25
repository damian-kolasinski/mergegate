import SwiftUI

struct ConversationEditorScene: View {
    @State var model: ConversationEditorModel
    let close: () -> Void
    @State private var confirmingDiscard = false
    var body: some View {
        NavigationStack {
            Form {
                Section("\(model.service.scope.name) · r\(model.service.scope.revision)") {
                    Text("Saved drafts stay on the hub until Complete review sends them.")
                    TextEditor(text: $model.text).frame(minHeight: 160).accessibilityLabel(
                        "Draft text"
                    )
                    .accessibilityIdentifier("conversationDraftText")
                }
                if case .comment = model.kind {
                    Section("Source anchor") {
                        Toggle("Re-place on this revision", isOn: $model.moveAnchor)
                        if model.moveAnchor {
                            TextField("File path", text: $model.path).textInputAutocapitalization(
                                .never
                            )
                            .autocorrectionDisabled()
                            Picker("Side", selection: $model.side) {
                                Text("Before").tag("old")
                                Text("After").tag("new")
                            }
                            TextField("First source line", text: $model.start).keyboardType(
                                .numberPad)
                            TextField("Last source line", text: $model.end).keyboardType(.numberPad)
                            Text(
                                "The hub validates these coordinates against this revision's diff.")
                        }
                    }
                }
                if let error = model.error {
                    Section { Text(error).foregroundStyle(Appearance.error) }
                }
            }.appearanceText(Appearance.body)
                .navigationTitle("Hub draft").navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { confirmingDiscard = true }.disabled(model.busy)
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button(model.busy ? "Saving…" : "Save draft") {
                            Task {
                                await model.save()
                                if model.saved { close() }
                            }
                        }.disabled(
                            model.busy
                                || model.text.trimmingCharacters(in: .whitespacesAndNewlines)
                                    .isEmpty
                        )
                    }
                }
                .confirmationDialog(
                    "Discard unsaved edits?", isPresented: $confirmingDiscard,
                    titleVisibility: .visible
                ) {
                    Button("Discard edits", role: .destructive, action: close)
                }
        }.interactiveDismissDisabled()
    }
}
