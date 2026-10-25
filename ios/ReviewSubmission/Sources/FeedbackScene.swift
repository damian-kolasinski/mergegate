import ConversationInterface
import SwiftUI

struct FeedbackScene: View {
    @State var model: FeedbackViewModel
    let close: () -> Void
    @State private var confirmResend = false
    @State private var confirmEdit = false
    @State private var confirmApproval = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("\(model.scope.name) · r\(model.scope.revision)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("feedbackScope")
                Spacer(minLength: 12)
                Button("Done", action: close).appearanceText(Appearance.label)
                    .frame(minWidth: 44, minHeight: 44).buttonStyle(.glass)
                    .disabled(model.busy || !model.summarySaved).accessibilityIdentifier("closeFeedback")
            }.padding(.horizontal, 20).padding(.top, 12).padding(.bottom, 12)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text(
                        model.sent
                            ? (model.approval?.title
                                ?? (model.sentVerdict == .changesRequested
                                    ? "Changes requested" : "Feedback sent"))
                            : (model.verdict == .approved ? "Approve & merge" : "Send feedback")
                    )
                    .appearanceText(Appearance.heading).accessibilityAddTraits(.isHeader)
                    .fixedSize(horizontal: false, vertical: true)
                    if model.sent {
                        Label("The hub received your feedback", systemImage: "checkmark.circle")
                            .appearanceText(Appearance.label).accessibilityIdentifier("feedbackSent")
                        if let approval = model.approval {
                            Text(
                                "\(model.scope.name) · r\(model.scope.revision) → \(model.approvalTarget ?? "target")"
                            )
                            .appearanceText(Appearance.label).fixedSize(horizontal: false, vertical: true)
                            Text(approval.detail).appearanceText(Appearance.body)
                                .textSelection(.enabled).accessibilityIdentifier("approvalResult")
                        } else {
                            Text(
                                model.sentVerdict == .changesRequested
                                    ? "Return to the inbox to follow this item in In progress. Your feedback is in Conversations."
                                    : "Your comments are now in Conversations."
                            ).appearanceText(Appearance.body)
                        }
                    } else {
                        Picker("Review outcome", selection: $model.verdict) {
                            ForEach(FeedbackVerdict.allCases) { verdict in
                                Text(verdict.title).tag(verdict)
                            }
                        }.pickerStyle(.menu).appearanceText(Appearance.label).frame(minHeight: 44)
                            .disabled(model.frozen || model.busy).accessibilityIdentifier("feedbackVerdict")
                        if model.verdict == .approved, let target = model.approvalTarget {
                            Text("r\(model.scope.revision) → \(target)")
                                .appearanceText(Appearance.label).accessibilityIdentifier("approvalTarget")
                            Text("Summary optional.")
                                .appearanceText(Appearance.body).fixedSize(horizontal: false, vertical: true)
                        } else {
                            Text(
                                model.verdict == .changesRequested
                                    ? "Describe what needs to change. Requesting changes returns this revision to In progress."
                                    : "Send comments and a summary for this revision."
                            )
                            .appearanceText(Appearance.body).fixedSize(horizontal: false, vertical: true)
                        }
                        Text("Summary").appearanceText(Appearance.label)
                        TextEditor(text: $model.summary).appearanceText(Appearance.body)
                            .scrollContentBackground(.hidden).frame(minHeight: 120).padding(8)
                            .background(Appearance.surface).disabled(model.frozen || model.busy)
                            .accessibilityLabel("Review summary").accessibilityIdentifier("feedbackSummary")
                        Text(
                            model.frozen
                                ? "Feedback retained for this delivery" : "Saved on this iPhone until sent"
                        )
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        Text("\(model.drafts.count) local comments").appearanceText(Appearance.label)
                        ForEach(model.drafts) { draft in
                            VStack(alignment: .leading, spacing: 8) {
                                Text(draft.location.path).appearanceText(Appearance.label)
                                    .accessibilityLabel("File \(draft.location.path)")
                                Text(anchorLabel(draft)).appearanceText(Appearance.caption)
                                    .foregroundStyle(Appearance.secondary)
                                Text(draft.location.anchor.quote).font(.system(.body, design: .monospaced))
                                Text(draft.body).appearanceText(Appearance.body)
                            }.fixedSize(horizontal: false, vertical: true)
                            Divider()
                        }
                        if !model.hubDrafts.isEmpty || !model.hubReplies.isEmpty {
                            Text("Also sends feedback saved on the hub").appearanceText(Appearance.label)
                            ForEach(model.hubDrafts) { draft in
                                Text("\(draft.path ?? "Item") · r\(draft.revision)\n\(draft.body)")
                                    .appearanceText(Appearance.body)
                            }
                            ForEach(model.hubReplies) { reply in
                                Text("Reply to \(reply.comment): \(reply.body)").appearanceText(
                                    Appearance.body)
                            }
                        }
                    }
                    if let error = model.error {
                        Text(error).appearanceText(Appearance.body).foregroundStyle(Appearance.error)
                            .accessibilityIdentifier("feedbackError")
                        if !model.summarySaved {
                            Button("Retry saving summary") { model.saveSummary() }.frame(minHeight: 44)
                        }

                    }
                    if !model.isCurrent && !model.sent {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(
                                "Your feedback is saved on this iPhone. Check the current revision before sending."
                            )
                            .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                            Button("Reconnect & check revision") { Task { await model.load() } }
                                .frame(minHeight: 44).disabled(model.busy)
                                .accessibilityIdentifier("recheckFeedbackRevision")
                        }
                    }

                }.padding(20)
            }.scrollDismissesKeyboard(.immediately)
            if !model.sent {
                Divider()
                VStack(alignment: .leading, spacing: 8) {
                    Button {
                        if model.verdict == .approved && !model.uncertain {
                            confirmApproval = true
                        } else {
                            Task { await model.send() }
                        }
                    } label: {
                        HStack {
                            if model.busy { ProgressView() }
                            Text(
                                model.uncertain
                                    ? "Check delivery" : model.frozen ? "Retry sending" : model.verdict.title)
                        }.appearanceText(Appearance.label)
                            .foregroundStyle(model.canSend ? Appearance.onFill : Appearance.primary)
                            .frame(maxWidth: .infinity, minHeight: 44)
                    }.buttonStyle(.glassProminent).disabled(!model.canSend)
                        .accessibilityIdentifier("sendFeedback")
                    if model.frozen && !model.uncertain && !model.busy {
                        Button("Edit feedback") { confirmEdit = true }.frame(minHeight: 44)
                    }
                    if model.uncertain && !model.busy {
                        Button("Send again") {
                            if model.verdict == .approved {
                                confirmApproval = true
                            } else {
                                confirmResend = true
                            }
                        }.frame(minHeight: 44)
                    }
                }.padding(.horizontal, 20).padding(.vertical, 12)
            }
        }
        .background(Appearance.page).foregroundStyle(Appearance.primary).tint(Appearance.brand)
        .interactiveDismissDisabled(model.busy || !model.summarySaved)
        .task { await model.load() }
        .onChange(of: model.summary) { _, _ in if !model.sent { model.saveSummary() } }
        .confirmationDialog(
            "Approve \(model.scope.name) r\(model.scope.revision)?",
            isPresented: $confirmApproval, titleVisibility: .visible
        ) {
            Button("Approve & merge into \(model.approvalTarget ?? "target")", role: .destructive) {
                let target = model.approvalTarget
                Task { await model.send(allowResend: model.uncertain, confirmedTarget: target) }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(
                "Squash-merge exactly revision \(model.scope.revision) into \(model.approvalTarget ?? "target") and send the feedback shown here. This changes the target branch."
            )
        }
        .confirmationDialog(
            "Edit retained feedback?", isPresented: $confirmEdit, titleVisibility: .visible
        ) {
            Button("Edit feedback") { Task { await model.editFeedback() } }
        } message: {
            Text(
                "Uploaded copies from this attempt are removed from hub drafts. Local comments and the summary stay on this iPhone."
            )
        }
        .confirmationDialog(
            "Send this feedback again?", isPresented: $confirmResend, titleVisibility: .visible
        ) {
            Button("Send again") { Task { await model.send(allowResend: true) } }
        } message: {
            Text(
                "The previous delivery is unconfirmed. A second send could duplicate the summary. The app checks for a receipt first."
            )
        }
    }

    private func anchorLabel(_ draft: FeedbackDraft) -> String {
        let anchor = draft.location.anchor
        let lines =
            anchor.startLine == anchor.endLine
            ? "Line \(anchor.startLine)" : "Lines \(anchor.startLine)–\(anchor.endLine)"
        return "\(anchor.side == .old ? "Before" : "After") · \(lines)"
    }
}
