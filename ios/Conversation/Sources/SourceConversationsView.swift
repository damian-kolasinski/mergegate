import SwiftUI

struct SourceConversationsView: View {
    @State var model: ConversationViewModel
    @State private var showing = false

    var body: some View {
        Button {
            showing = true
        } label: {
            HStack(alignment: .firstTextBaseline) {
                Label("Conversations", systemImage: "bubble.left.and.bubble.right")
                Spacer(minLength: 8)
                if model.loading {
                    ProgressView()
                } else if model.error != nil {
                    Text("Retry")
                } else {
                    Text("\(model.threads.count)")
                }
            }.appearanceText(Appearance.caption)
                .padding(.horizontal, 20).frame(minHeight: 44)
        }.tint(Appearance.brand).accessibilityIdentifier("fileConversations")
            .sheet(isPresented: $showing, onDismiss: { Task { await model.load() } }) {
                ConversationScene(model: model) { showing = false }
            }
            .task { await model.load() }
    }
}
