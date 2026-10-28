import ConversationInterface
import SwiftUI

struct ConversationScene: View {
    @State var model: ConversationViewModel
    @Environment(\.scenePhase) private var scenePhase
    let close: () -> Void
    var showSource: (ConversationLocation) -> Void = { _ in }
    var onActivity: () -> Void = {}
    @State private var showingRest = false

    private func group(_ title: String, count: Int) -> some View {
        Text("\(title.uppercased()) · \(count)").appearanceText(Appearance.caption)
            .fontWeight(.semibold).foregroundStyle(Appearance.secondary)
            .padding(.horizontal, 20).padding(.top, 12).padding(.bottom, 4)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityAddTraits(.isHeader)
    }

    private func row(_ thread: ConversationThread) -> some View {
        VStack(spacing: 0) {
            NavigationLink {
                ConversationThreadScene(
                    model: model, threadID: thread.id, showSource: showSource,
                    onActivity: onActivity)
            } label: {
                ConversationThreadRow(thread: thread)
            }.buttonStyle(.plain).accessibilityIdentifier("conversation-\(thread.id)")
            Divider()
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("\(model.scope.name) · r\(model.scope.revision)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                Spacer(minLength: 12)
                Button("Done", action: close).appearanceText(Appearance.label)
                    .frame(minWidth: 28).buttonStyle(AppearanceButtonStyle())
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
                        // The web panel's groups: what waits on the operator, then the rest.
                        let needsYou = model.threads.filter(\.needsYou)
                        let rest = model.threads.filter { !$0.needsYou }
                        if !needsYou.isEmpty {
                            group("Needs you", count: needsYou.count)
                            ForEach(needsYou) { thread in row(thread) }
                        }
                        if !rest.isEmpty {
                            Button {
                                showingRest.toggle()
                            } label: {
                                HStack {
                                    group("Everything else", count: rest.count)
                                    Image(
                                        systemName: showingRest || needsYou.isEmpty
                                            ? "chevron.down" : "chevron.right"
                                    ).foregroundStyle(Appearance.secondary).padding(.trailing, 20)
                                }.frame(minHeight: 44).contentShape(Rectangle())
                            }.buttonStyle(.plain)
                                .accessibilityIdentifier("conversationEverythingElse")
                            if showingRest || needsYou.isEmpty {
                                ForEach(rest) { thread in row(thread) }
                            }
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
