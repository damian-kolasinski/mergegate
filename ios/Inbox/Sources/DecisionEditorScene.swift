import SwiftUI

struct DecisionEditorScene<Feedback: View>: View {
    @State var model: DecisionViewModel
    let feedback: (@escaping () -> Void) -> Feedback
    @State private var showingFeedback = false
    @State private var confirmingAcceptance = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("\(model.item.name) · r\(model.item.revision)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                if let path = model.decision.path {
                    Text(path).appearanceText(Appearance.caption).foregroundStyle(
                        Appearance.secondary
                    )
                    .accessibilityLabel("File \(path)")
                }
                Text("Original comment").appearanceText(Appearance.label)
                Text(model.decision.comment).appearanceText(Appearance.body)
                if let response = model.decision.response {
                    Text(model.decision.kind == "Question" ? "Question" : "Rejection reason")
                        .appearanceText(Appearance.label).foregroundStyle(Appearance.brand)
                    Text(response).appearanceText(Appearance.body)
                }
                Divider()
                if let result = model.result {
                    Label(result, systemImage: "checkmark.circle").appearanceText(
                        Appearance.rowHeading
                    )
                    .accessibilityIdentifier("decisionResult")
                    Text("Return to the inbox to see the remaining decisions.").appearanceText(
                        Appearance.body)
                } else {
                    if model.busy { ProgressView("Contacting hub…") }
                    Text(
                        model.decision.kind == "Question"
                            ? "Your answer" : "Override note (optional)"
                    )
                    .appearanceText(Appearance.label)
                    TextEditor(text: $model.text).appearanceText(Appearance.body)
                        .scrollContentBackground(.hidden).frame(minHeight: 140).padding(8)
                        .background(Appearance.surface)
                        .accessibilityLabel(
                            model.decision.kind == "Question" ? "Your answer" : "Override note"
                        )
                        .accessibilityIdentifier("decisionText")
                        .disabled(model.busy || !model.loaded)
                    if model.decision.kind == "Question" {
                        Text(
                            "Save your answer on the hub, then review and send it with your feedback."
                        )
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        Button {
                            Task { await model.saveAnswer() }
                        } label: {
                            Text("Save answer").appearanceText(Appearance.label)
                                .frame(minHeight: 44)
                        }.buttonStyle(.glass).foregroundStyle(Appearance.brand)
                            .disabled(
                                !canAct
                                    || model.text.trimmingCharacters(in: .whitespacesAndNewlines)
                                        .isEmpty
                            )
                            .accessibilityIdentifier("saveDecisionAnswer")
                        if model.answerSaved {
                            Label(
                                "Answer saved on hub · Not sent yet",
                                systemImage: "checkmark.circle"
                            )
                            .appearanceText(Appearance.caption).accessibilityIdentifier(
                                "decisionAnswerSaved")
                            Button {
                                showingFeedback = true
                            } label: {
                                Text("Review & send feedback").appearanceText(Appearance.label)
                                    .foregroundStyle(
                                        canAct ? Appearance.onFill : Appearance.primary
                                    )
                                    .frame(minHeight: 44)
                            }.buttonStyle(.glassProminent).disabled(!canAct)
                                .accessibilityIdentifier("sendDecisionFeedback")
                        }
                    } else {
                        Text(
                            "An override sends this comment and your note back immediately. Accepting closes this rejection."
                        )
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        Button {
                            Task { await model.settle(override: true) }
                        } label: {
                            Text("Override — must fix").appearanceText(Appearance.label)
                                .foregroundStyle(canAct ? Appearance.onFill : Appearance.primary)
                                .frame(minHeight: 44)
                        }.buttonStyle(.glassProminent).disabled(!canAct)
                            .accessibilityIdentifier("overrideDecision")
                        Button {
                            confirmingAcceptance = true
                        } label: {
                            Text("Accept rejection").appearanceText(Appearance.label).frame(
                                minHeight: 44)
                        }.buttonStyle(.glass).foregroundStyle(Appearance.brand).disabled(!canAct)
                            .accessibilityIdentifier("acceptDecision")
                    }
                }
                if let error = model.error {
                    Text(error).appearanceText(Appearance.body).foregroundStyle(Appearance.error)
                        .accessibilityIdentifier("decisionError")
                    if !model.loaded {
                        Button("Retry") { Task { await model.load() } }.frame(minHeight: 44)
                    }
                }
            }.padding(24).frame(maxWidth: 680).frame(maxWidth: .infinity)
        }.scrollDismissesKeyboard(.immediately).background(Appearance.page).foregroundStyle(
            Appearance.primary
        ).tint(Appearance.brand)
            .navigationTitle(
                model.decision.kind == "Question" ? "Answer question" : "Resolve rejection"
            )
            .navigationBarTitleDisplayMode(.inline)
            .toolbar(.visible, for: .navigationBar)
            .navigationBarBackButtonHidden(model.busy)
            .interactiveDismissDisabled(model.busy)
            .task { await model.load() }
            .sheet(
                isPresented: $showingFeedback, onDismiss: { Task { await model.feedbackClosed() } }
            ) {
                feedback { showingFeedback = false }
            }
            .confirmationDialog(
                "Accept this rejection?", isPresented: $confirmingAcceptance,
                titleVisibility: .visible
            ) {
                Button("Accept and close") { Task { await model.settle(override: false) } }
            } message: {
                Text("This closes the pending rejection without requesting a change.")
            }
    }

    private var canAct: Bool { model.loaded && !model.busy }
}
