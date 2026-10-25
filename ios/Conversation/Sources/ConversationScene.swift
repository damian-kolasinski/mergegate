import ConversationInterface
import SwiftUI

struct ConversationScene: View {
    @State var model: ConversationViewModel
    @Environment(\.scenePhase) private var scenePhase
    let close: () -> Void
    var showSource: (ConversationLocation) -> Void = { _ in }
    var onActivity: () -> Void = {}

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("\(model.scope.name) · r\(model.scope.revision)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                Spacer(minLength: 12)
                Button("Done", action: close).appearanceText(Appearance.label)
                    .frame(minWidth: 44, minHeight: 44).buttonStyle(.glass)
                    .accessibilityIdentifier("closeConversations")
            }.padding(.horizontal, 20).padding(.top, 12)
            NavigationStack {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        VStack(alignment: .leading, spacing: 6) {
                            if let path = model.path {
                                Text(path).appearanceText(Appearance.rowHeading)
                                    .accessibilityLabel("File: \(path)")
                            }
                            if !model.anchors.isEmpty {
                                Text("Threads on selected source lines").appearanceText(
                                    Appearance.caption)
                            }
                        }.padding(20)
                        if model.loading && model.snapshot == nil {
                            ProgressView("Loading conversations…").padding(20)
                        }
                        if model.isReconnecting {
                            Label(
                                "Reconnecting… Showing the last conversations.",
                                systemImage: "wifi.exclamationmark"
                            )
                            .appearanceText(Appearance.caption).foregroundStyle(
                                Appearance.secondary
                            )
                            .padding(20).accessibilityIdentifier("conversationsReconnecting")
                        }
                        if let error = model.error {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(error).appearanceText(Appearance.body).foregroundStyle(
                                    Appearance.error)
                                Button("Retry") { Task { await model.load() } }.frame(minHeight: 44)
                                    .accessibilityIdentifier("retryConversations")
                            }.padding(20)
                        }
                        if model.snapshot != nil && model.threads.isEmpty {
                            ContentUnavailableView(
                                "No conversations", systemImage: "bubble.left.and.bubble.right",
                                description: Text("No sent threads match this scope."))
                        }
                        ConversationHubDrafts(model: model)
                        ForEach(model.threads) { thread in
                            NavigationLink {
                                ConversationThreadScene(
                                    model: model, threadID: thread.id, showSource: showSource,
                                    onActivity: onActivity)
                            } label: {
                                ConversationThreadRow(thread: thread)
                            }.buttonStyle(.plain).accessibilityIdentifier(
                                "conversation-\(thread.id)")
                            Divider()
                        }
                    }
                }.onScrollPhaseChange { _, phase in
                    if phase == .interacting || phase == .idle { onActivity() }
                }
                .accessibilityIdentifier("conversationList")
                .refreshable { await model.load() }
                .background(Appearance.page)
                .navigationTitle("Conversations").navigationBarTitleDisplayMode(.inline)
            }
        }.background(Appearance.page)
            .foregroundStyle(Appearance.primary).tint(Appearance.brand)
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                await model.follow()
            }
    }
}

struct ConversationThreadRow: View {
    let thread: ConversationThread

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(thread.location).appearanceText(Appearance.label).foregroundStyle(Appearance.brand)
            if let path = thread.comment.filePath {
                Text(path).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    .accessibilityLabel("File: \(path)")
            }
            Text(thread.comment.body).appearanceText(Appearance.body).lineLimit(3)
            Text("\(thread.status) · \(thread.replies.count) replies")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            if let placement = thread.placementLabel {
                Text(placement).appearanceText(Appearance.caption).foregroundStyle(
                    Appearance.secondary)
            }
        }.frame(maxWidth: .infinity, minHeight: 44, alignment: .leading).padding(20)
            .contentShape(Rectangle())
    }
}
