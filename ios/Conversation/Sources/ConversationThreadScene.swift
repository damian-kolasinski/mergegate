import ConversationInterface
import SwiftUI

struct ConversationThreadScene: View {
    let model: ConversationViewModel
    let threadID: String
    var showSource: (ConversationLocation) -> Void = { _ in }
    var onActivity: () -> Void = {}

    var body: some View {
        if let thread = model.threads.first(where: { $0.id == threadID }) {
            ConversationThreadView(
                thread: thread, onActivity: onActivity,
                editor: AnyView(
                    ConversationReplyActions(model: model, thread: thread, showSource: showSource)))
        } else {
            ContentUnavailableView("Conversation unavailable", systemImage: "bubble.left")
        }
    }
}
