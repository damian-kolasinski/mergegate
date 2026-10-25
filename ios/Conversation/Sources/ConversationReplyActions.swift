import ConversationInterface
import DiffDocumentInterface
import SwiftUI

struct ConversationReplyActions: View {
    let model: ConversationViewModel
    let thread: ConversationThread
    let showSource: (ConversationLocation) -> Void
    @State private var editor: ConversationEditorModel?
    @State private var deleting: ThreadReply?
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let path = thread.comment.filePath, let range = thread.sourceRange,
                let rawSide = thread.comment.side, let side = SourceAnchor.Side(rawValue: rawSide)
            {
                Button("Show in diff") {
                    showSource(
                        .init(
                            path: path,
                            anchors: [
                                .init(
                                    side: side, startLine: range.lowerBound,
                                    endLine: range.upperBound, quote: thread.comment.quote ?? "")
                            ]))
                }.frame(minHeight: 44).accessibilityIdentifier("showConversationSource")
            }
            ConversationDecisionActions(model: model, thread: thread)
            ForEach((model.snapshot?.draft_replies ?? []).filter { $0.comment == thread.id }) {
                reply in
                VStack(alignment: .leading, spacing: 8) {
                    Text("Draft reply · Not sent").appearanceText(Appearance.rowHeading)
                    Text(reply.body).appearanceText(Appearance.body)
                    HStack {
                        Button("Edit reply") { edit(reply) }.frame(minHeight: 44)
                        Spacer()
                        Button("Delete reply", role: .destructive) { deleting = reply }.frame(
                            minHeight: 44)
                    }.disabled(busy)
                }
            }
            if thread.comment.external?.vote != "down" && thread.comment.external?.summary != true {
                Button("Reply") {
                    editor = ConversationEditorModel(
                        kind: .reply(comment: thread.id, id: nil), text: "",
                        service: model.editingService)
                }.frame(minHeight: 44).accessibilityIdentifier("replyToThread")
            }
            if let error { Text(error).foregroundStyle(Appearance.error) }
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
            "Delete this draft reply?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible
        ) {
            if let reply = deleting {
                Button("Delete reply", role: .destructive) {
                    Task {
                        busy = true
                        defer { busy = false }
                        do {
                            let current = try await model.editingService.current()
                            if (current.draft_replies ?? []).contains(where: { $0.id == reply.id })
                            {
                                try await model.editingService.removeReply(reply)
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
    private func edit(_ reply: ThreadReply) {
        editor = ConversationEditorModel(
            kind: .reply(comment: reply.comment, id: reply.id), text: reply.body,
            service: model.editingService)
    }
}
