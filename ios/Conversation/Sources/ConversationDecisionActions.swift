import Foundation
import SwiftUI

struct ConversationDecisionActions: View {
    let model: ConversationViewModel
    let thread: ConversationThread
    @State private var busy = false
    @State private var error: String?
    @State private var note = ""
    @State private var confirmingOverride = false
    private var rejection: ThreadReply? {
        guard thread.comment.external != nil, let last = thread.replies.last,
            last.author == "agent", last.status == "rejected",
            thread.comment.external?.agreed_reply != last.id
        else { return nil }
        return last
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let external = thread.comment.external, external.summary != true,
                external.pending || external.lapsed == true || external.vote == "down"
            {
                Text("Your vote decides whether the agent receives this comment.").appearanceText(
                    Appearance.caption)
                HStack {
                    Button("Send to agent") { send(.vote("up")) }.frame(minHeight: 44)
                    Spacer()
                    Button("Dismiss") { send(.vote("down")) }.frame(minHeight: 44)
                }
            }
            if let rejection {
                Button("Accept rejection") { send(.agree(rejection.id)) }.frame(minHeight: 44)
                TextField("Override note (optional)", text: $note, axis: .vertical)
                Button("Override rejection…") { confirmingOverride = true }.frame(minHeight: 44)
            }
            if let error { Text(error).foregroundStyle(Appearance.error) }
        }.disabled(busy)
            .confirmationDialog(
                "Send this override to the agent now?", isPresented: $confirmingOverride,
                titleVisibility: .visible
            ) {
                if let rejection {
                    Button("Send override") {
                        send(
                            .override(
                                rejection.id, note.trimmingCharacters(in: .whitespacesAndNewlines)))
                    }
                }
            }
    }
    private func send(_ decision: ConversationDecision) {
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
}

enum ConversationDecision {
    case vote(String)
    case agree(UInt64)
    case override(UInt64, String)
}

extension ConversationEditingService {
    func decide(comment id: String, decision: ConversationDecision) async throws {
        let current = try await current()
        guard let comment = current.comments.first(where: { $0.id == id }),
            let external = comment.external
        else {
            throw ConversationEditIssue(message: "This external comment is no longer available.")
        }
        let path: String
        let body: Data
        switch decision {
        case .vote(let vote):
            guard external.summary != true,
                external.pending || external.lapsed == true || external.vote == "down"
            else {
                throw ConversationEditIssue(
                    message: "This comment has already been delivered. Refresh to see its status.")
            }
            struct Body: Encodable { let vote: String }
            path = "vote"
            body = try JSONEncoder().encode(Body(vote: vote))
        case .agree(let reply), .override(let reply, _):
            if case .agree = decision, external.agreed_reply == reply { return }
            let replies = current.replies.filter { $0.comment == id }
            if case .override(_, let note) = decision,
                replies.contains(where: {
                    $0.id > reply && $0.author == "operator" && $0.overrides == true
                        && $0.body == note
                })
            {
                return
            }
            guard let last = replies.last, last.id == reply, last.author == "agent",
                last.status == "rejected",
                external.agreed_reply != reply
            else {
                throw ConversationEditIssue(
                    message: "The rejection changed. Refresh before making another decision.")
            }
            if case .override(_, let note) = decision {
                struct Body: Encodable { let note: String }
                path = "override"
                body = try JSONEncoder().encode(Body(note: note))
            } else {
                path = "agree"
                body = Data("{}".utf8)
            }
        }
        let endpoint = scope.hub.appending(path: "api/comments").appending(component: id).appending(
            component: path)
        let response = try await transport.post(endpoint, body: body)
        switch decision {
        case .override(_, let note):
            let value = try JSONDecoder().decode(ThreadReply.self, from: response)
            guard value.comment == id, value.author == "operator", value.overrides == true,
                value.body == note
            else {
                throw URLError(.badServerResponse)
            }
        case .vote(let vote):
            let value = try JSONDecoder().decode(ThreadComment.self, from: response)
            guard value.id == id, value.external?.vote == vote else {
                throw URLError(.badServerResponse)
            }
        case .agree(let reply):
            let value = try JSONDecoder().decode(ThreadComment.self, from: response)
            guard value.id == id, value.external?.agreed_reply == reply else {
                throw URLError(.badServerResponse)
            }
        }
    }
}
