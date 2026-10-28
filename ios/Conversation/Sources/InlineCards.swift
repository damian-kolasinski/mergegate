import ConversationInterface
import SwiftUI

// The threads and drafts on one source line, inline in the reader. A card that waits on the
// operator carries its decision, one tap each: send a GitHub comment to the agent or dismiss it,
// accept or override a rejection, answer a question.
struct InlineCards: View {
    let model: ConversationViewModel
    let threads: [ConversationThread]
    let hubDrafts: [ThreadComment]
    let drafts: [CommentDraft]
    let draftsChanged: () -> Void
    @State private var opened: Opened?
    @State private var editor: ConversationEditorModel?
    @State private var overriding: ConversationThread?
    @State private var busy = false
    @State private var error: String?

    private enum Opened: Identifiable {
        case thread(String)
        case draft(CommentDraft)
        var id: String {
            switch self {
            case .thread(let id): "thread-\(id)"
            case .draft(let draft): "draft-\(draft.id)"
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(threads) { thread in threadCard(thread) }
            ForEach(hubDrafts, id: \.id) { draft in
                card(dashed: true) {
                    badge("Draft on hub", tone: Appearance.tintBrand, text: Appearance.onTintBrand)
                    Text(draft.body).appearanceText(Appearance.body).lineLimit(3)
                }.accessibilityIdentifier("inlineHubDraft-\(draft.id)")
            }
            ForEach(drafts) { draft in
                Button {
                    opened = .draft(draft)
                } label: {
                    card(dashed: true) {
                        HStack(spacing: 6) {
                            badge("Draft", tone: Appearance.tintBrand, text: Appearance.onTintBrand)
                            Text(draft.location.label).appearanceText(Appearance.caption)
                                .foregroundStyle(Appearance.secondary)
                            Spacer(minLength: 0)
                            Text("Edit").appearanceText(Appearance.caption)
                                .foregroundStyle(Appearance.brand)
                        }
                        Text(draft.body.isEmpty ? "Empty draft" : draft.body)
                            .appearanceText(Appearance.body).lineLimit(3)
                            .multilineTextAlignment(.leading)
                    }
                }.buttonStyle(.plain).accessibilityIdentifier("inlineDraft-\(draft.id)")
            }
            if let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
            }
        }
        .padding(.leading, 28).padding(.trailing, 12).padding(.vertical, 6)
        .foregroundStyle(Appearance.primary).tint(Appearance.brand)
        .disabled(busy)
        .sheet(item: $opened, onDismiss: draftsChanged) { opened in
            switch opened {
            case .thread(let id):
                NavigationStack {
                    ConversationThreadScene(model: model, threadID: id)
                        .toolbar { Button("Done") { self.opened = nil } }
                }
            case .draft(let draft):
                CommentDraftScene(scope: model.scope, location: nil, draftID: draft.id) {
                    self.opened = nil
                }
            }
        }
        .sheet(isPresented: Binding(get: { editor != nil }, set: { if !$0 { editor = nil } })) {
            if let editor {
                ConversationEditorScene(model: editor) {
                    self.editor = nil
                    Task { await model.load() }
                }
            }
        }
        .confirmationDialog(
            "Override the rejection?",
            isPresented: Binding(get: { overriding != nil }, set: { if !$0 { overriding = nil } }),
            titleVisibility: .visible
        ) {
            if let thread = overriding, let rejection = thread.replies.last {
                Button("Override — must fix") { decide(thread, .override(rejection.id, "")) }
            }
        } message: {
            Text("The agent gets the comment again and must address it. Add a note from the thread.")
        }
    }

    private func threadCard(_ thread: ConversationThread) -> some View {
        card(dashed: false) {
            Button {
                opened = .thread(thread.id)
            } label: {
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 6) {
                        Text(thread.comment.external?.author ?? "You").appearanceText(Appearance.label)
                        if thread.needsYou {
                            badge(thread.status, tone: Appearance.tintWarning, text: Appearance.onTintWarning)
                        } else {
                            badge(thread.status, tone: Appearance.tintNeutral, text: Appearance.onTintNeutral)
                        }
                        Spacer(minLength: 0)
                        if !thread.replies.isEmpty {
                            Text("\(thread.replies.count) \(thread.replies.count == 1 ? "reply" : "replies")")
                                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        }
                    }
                    Text(thread.comment.body).appearanceText(Appearance.body)
                        .lineLimit(thread.needsYou ? nil : 2).multilineTextAlignment(.leading)
                    if let reply = thread.replies.last, reply.author == "agent", !reply.body.isEmpty {
                        Text("Agent: \(reply.body)").appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.secondary).lineLimit(thread.needsYou ? nil : 2)
                            .multilineTextAlignment(.leading)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
            }.buttonStyle(.plain)
            if thread.needsYou { actions(thread) }
        }.accessibilityElement(children: .contain).accessibilityIdentifier("inlineThread-\(thread.id)")
    }

    @ViewBuilder private func actions(_ thread: ConversationThread) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { decisionButtons(thread) }
            VStack(alignment: .leading, spacing: 8) { decisionButtons(thread) }
        }
    }

    @ViewBuilder private func decisionButtons(_ thread: ConversationThread) -> some View {
        let last = thread.replies.last
        if last?.author == "agent", last?.status == "question" {
            action("Answer", id: "inlineAnswer-\(thread.id)") {
                editor = ConversationEditorModel(
                    kind: .reply(comment: thread.id, id: nil), text: "",
                    service: model.editingService)
            }
        } else if last?.author == "agent", last?.status == "rejected", let last {
            action("Accept", id: "inlineAccept-\(thread.id)") { decide(thread, .agree(last.id)) }
            action("Override…", id: "inlineOverride-\(thread.id)") { overriding = thread }
        } else {
            action("👍 Send to agent", id: "inlineSendToAgent-\(thread.id)") {
                decide(thread, .vote("up"))
            }
            action("👎 Dismiss", id: "inlineDismiss-\(thread.id)") { decide(thread, .vote("down")) }
        }
    }

    private func decide(_ thread: ConversationThread, _ decision: ConversationDecision) {
        Task {
            busy = true
            defer { busy = false }
            do {
                try await model.editingService.decide(comment: thread.id, decision: decision)
                error = nil
                await model.load()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func action(_ title: String, id: String, perform: @escaping () -> Void) -> some View {
        Button(action: perform) {
            Text(title)
        }.buttonStyle(AppearanceButtonStyle()).accessibilityIdentifier(id)
    }

    private func badge(_ title: String, tone: Color, text: Color) -> some View {
        Text(title).appearanceText(Appearance.caption).fontWeight(.semibold).lineLimit(1)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .foregroundStyle(text).background(tone, in: Capsule())
    }

    private func card<Content: View>(dashed: Bool, @ViewBuilder content: () -> Content)
        -> some View
    {
        VStack(alignment: .leading, spacing: 6, content: content)
            .padding(10).frame(maxWidth: .infinity, alignment: .leading)
            .background(Appearance.surface, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
            .overlay(
                RoundedRectangle(cornerRadius: Appearance.controlRadius).strokeBorder(
                    dashed ? Appearance.brand : Appearance.border,
                    style: StrokeStyle(lineWidth: dashed ? 1 : 0.5, dash: dashed ? [4, 3] : [])))
    }
}
