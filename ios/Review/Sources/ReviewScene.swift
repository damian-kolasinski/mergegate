import CodeStylingInterface
import ConversationInterface
import DiffView
import SwiftUI

struct ReviewScene: View {
    @State var model: ReviewViewModel
    let styling: any CodeStyling
    let hub: URL
    let drafts: (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> AnyView
    let conversations: (CommentDraftScope, ConversationLocation?, @escaping () -> Void) -> AnyView
    let fileConversations: (CommentDraftScope, String) -> AnyView
    private enum Sheet: Identifiable {
        case guide, files
        case drafts(CommentDraftLocation?)
        case conversations(ConversationLocation?)
        var id: String {
            switch self {
            case .guide: "guide"
            case .files: "files"
            case .drafts: "drafts"
            case .conversations: "conversations"
            }
        }
    }
    @State private var sheet: Sheet?

    var body: some View {
        Group {
            switch model.state {
            case .loading: ProgressView("Opening review…")
            case .failed(let message):
                ContentUnavailableView {
                    Label("Couldn’t open review", systemImage: "wifi.exclamationmark")
                } description: {
                    Text(message)
                } actions: {
                    Button("Retry") { Task { await model.load() } }.frame(minHeight: 44)
                }
            case .loaded(let snapshot): content(snapshot)
            }
        }
        .background(Appearance.page).foregroundStyle(Appearance.primary)
        .navigationTitle("Review").navigationBarTitleDisplayMode(.inline)
        // Returning triggers the inbox refresh, which must follow the pending write.
        .navigationBarBackButtonHidden(model.isSavingViewed)
        .task { await model.load() }
    }

    private func content(_ snapshot: ReviewSnapshot) -> some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(snapshot.item.title).appearanceText(Appearance.heading)
                                .accessibilityAddTraits(.isHeader)
                            HStack(alignment: .top, spacing: 12) {
                                Text("All changes → \(snapshot.item.target)")
                                Spacer(minLength: 0)
                                Text("\(snapshot.remaining) files left")
                            }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        }.padding(.horizontal, 20).padding(.vertical, 12).id("reviewTop")
                        Divider()
                        if let index = snapshot.files.firstIndex(where: { $0.path == model.selectedPath }) {
                            let file = snapshot.files[index]
                            ReviewFilePicker(file: file, position: index + 1, count: snapshot.files.count) {
                                sheet = .files
                            }
                            Divider()
                            fileConversations(scope(snapshot), file.path).id(file.path)
                            Divider()
                            ReviewFileReader(
                                file: file, project: snapshot.item.project,
                                sourceService: model.sourceService,
                                highlightService: model.highlightService,
                                item: snapshot.item.id, revision: snapshot.item.revision.number,
                                styling: styling,
                                onComment: { anchor in
                                    sheet = .drafts(.init(path: file.path, anchor: anchor))
                                },
                                onReadConversations: { anchors in
                                    sheet = .conversations(.init(path: file.path, anchors: anchors))
                                }
                            ).id(file.path)
                        } else {
                            ContentUnavailableView(
                                "No changed files", systemImage: "doc",
                                description: Text("This item has no files to display."))
                        }
                    }
                }
                .onChange(of: model.selectedPath) { _, _ in proxy.scrollTo("reviewTop", anchor: .top) }
            }
            if let file = snapshot.files.first(where: { $0.path == model.selectedPath }) {
                ReviewViewedBar(
                    file: file, viewed: snapshot.reviewCount - snapshot.remaining,
                    total: snapshot.reviewCount,
                    saving: model.isSavingViewed, failure: model.viewedFailure,
                    mark: { Task { await model.setViewed(path: file.path, viewed: !file.viewed) } },
                    retry: { Task { await model.retryViewed() } })
            }
        }
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text("\(snapshot.item.name) · r\(snapshot.item.revision.number)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary).lineLimit(1)
                    .accessibilityLabel(
                        "\(snapshot.item.name) · r\(snapshot.item.revision.number) → \(snapshot.item.target)"
                    )
                    .accessibilityIdentifier("reviewIdentity")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .conversations(nil)
                } label: {
                    Label("Conversations", systemImage: "bubble.left.and.bubble.right")
                }.tint(Appearance.brand).accessibilityIdentifier("reviewConversations")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .drafts(nil)
                } label: {
                    Label("Comment drafts", systemImage: "text.bubble")
                }.tint(Appearance.brand).accessibilityIdentifier("reviewDrafts")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .guide
                } label: {
                    Label("Review guide", systemImage: "book")
                }.tint(Appearance.brand).accessibilityIdentifier("reviewGuide")
            }
        }
        .sheet(item: $sheet) { selection in
            switch selection {
            case .conversations(let location):
                conversations(scope(snapshot), location) { sheet = nil }
            case .drafts(let location):
                drafts(
                    .init(
                        hub: hub, item: snapshot.item.id, name: snapshot.item.name,
                        revision: snapshot.item.revision.number), location
                ) { sheet = nil }
            case .guide: ReviewGuideScene(guide: snapshot.guide) { sheet = nil }
            case .files:
                ReviewFilesScene(
                    files: snapshot.files, selectedPath: model.selectedPath, close: { sheet = nil }
                ) {
                    path in
                    model.selectedPath = path
                    sheet = nil
                }
            }
        }
    }
    private func scope(_ snapshot: ReviewSnapshot) -> CommentDraftScope {
        .init(
            hub: hub, item: snapshot.item.id, name: snapshot.item.name,
            revision: snapshot.item.revision.number)
    }

}
