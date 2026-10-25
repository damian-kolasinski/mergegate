import SwiftUI

struct ConversationThreadScene: View {
    let model: ConversationViewModel
    let threadID: String

    var body: some View {
        if let thread = model.threads.first(where: { $0.id == threadID }) {
            ConversationThreadView(thread: thread)
        } else {
            ContentUnavailableView("Conversation unavailable", systemImage: "bubble.left")
        }
    }
}
