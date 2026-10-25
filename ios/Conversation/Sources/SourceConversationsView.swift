import SwiftUI

struct SourceConversationsView: View {
    @State var model: ConversationViewModel
    @State private var showing = false
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Button {
            showing = true
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Label("Conversations", systemImage: "bubble.left.and.bubble.right")
                    .labelStyle(.iconOnly)
                if model.loading {
                    ProgressView()
                } else if model.error != nil {
                    Text("Retry")
                } else {
                    Text("\(model.threads.count)")
                }
            }.appearanceText(Appearance.caption)
                .padding(.horizontal, 16).frame(minWidth: 44, minHeight: 44)
                .fixedSize(horizontal: true, vertical: false)
        }.tint(Appearance.brand).accessibilityLabel("File conversations")
            .accessibilityValue(model.loading ? "Loading" : model.error != nil ? "Retry" : "\(model.threads.count)")
            .accessibilityIdentifier("fileConversations")
            .sheet(isPresented: $showing) {
                ConversationScene(model: model) { showing = false }
            }
            .task(id: scenePhase == .active && !showing) {
                guard scenePhase == .active && !showing else { return }
                await model.follow()
            }
    }
}
