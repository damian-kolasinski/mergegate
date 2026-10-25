import CodeStylingInterface
import ConversationInterface
import DiffView
import SwiftUI

struct ReviewScene<Drafts: View, Conversations: View, FileConversations: View, Feedback: View>: View {
    @State var model: ReviewViewModel
    @Environment(\.scenePhase) private var scenePhase
    let styling: any CodeStyling
    let hub: URL
    let drafts: (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> Drafts
    let conversations: (CommentDraftScope, ConversationLocation?, @escaping () -> Void) -> Conversations
    let fileConversations: (CommentDraftScope, String) -> FileConversations
    let feedback: (CommentDraftScope, @escaping () -> Void) -> Feedback
    private enum Sheet: Identifiable {
        case guide, files, feedback
        case drafts(CommentDraftLocation?)
        case conversations(ConversationLocation?)
        var id: String {
            switch self {
            case .guide: "guide"
            case .files: "files"
            case .feedback: "feedback"
            case .drafts: "drafts"
            case .conversations: "conversations"
            }
        }
    }
    @State private var sheet: Sheet?
    @State private var scrollPosition = ScrollPosition()
    @State private var visibleOffset: Double = 0
    @State private var restoreOffset: Double?
    @State private var readerMoved = false

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
        .navigationBarBackButtonHidden(model.isSavingViewed || model.isComparing)
        .task(id: scenePhase) {
            guard scenePhase == .active else { return }
            await model.follow()
        }
    }

    private func content(_ snapshot: ReviewSnapshot) -> some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(snapshot.item.title).appearanceText(Appearance.heading)
                            .accessibilityAddTraits(.isHeader)
                        ReviewScopeBar(
                            snapshot: snapshot, loading: model.isComparing,
                            disabled: model.isSavingViewed || model.cachedAt != nil,
                            compare: { since in Task { await model.compare(since: since) } })
                        ReviewUpdatesView(model: model, revision: snapshot.item.revision.number)
                    }.padding(.horizontal, 20).padding(.vertical, 12).id("reviewTop")
                    if let failure = model.comparisonFailure {
                        Text(failure).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                            .padding(.horizontal, 20).padding(.bottom, 8)
                            .accessibilityIdentifier("comparisonError")
                    }
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
                            contextLoaded: $model.contextLoaded, revealed: $model.revealed,
                            onReady: {
                                if !readerMoved { restoreOffset = model.readingOffset }
                            },
                            onComment: { anchor in
                                sheet = .drafts(.init(path: file.path, anchor: anchor))
                            },
                            onReadConversations: { anchors in
                                sheet = .conversations(.init(path: file.path, anchors: anchors))
                            }
                        ).id(
                            "\(snapshot.item.revision.number)-\(snapshot.since.map(String.init) ?? "all")-\(file.path)"
                        )
                    } else {
                        ContentUnavailableView(
                            "No changed files", systemImage: "doc",
                            description: Text("This item has no files to display."))
                    }
                }
            }
            .accessibilityIdentifier("reviewScroll")
            .scrollPosition($scrollPosition)
            .onScrollGeometryChange(for: ReviewScrollGeometry.self) { geometry in
                ReviewScrollGeometry(
                    offset: max(0, geometry.contentOffset.y + geometry.contentInsets.top),
                    contentHeight: geometry.contentSize.height,
                    viewportHeight: geometry.containerSize.height)
            } action: { _, value in
                visibleOffset = value.offset
                // TextKit supplies its height after source context has been restored.
                if let offset = restoreOffset,
                    value.contentHeight >= offset + value.viewportHeight
                {
                    if abs(value.offset - offset) < 1 {
                        restoreOffset = nil
                    } else {
                        scrollPosition.scrollTo(y: offset)
                    }
                }
            }
            .onChange(of: restoreOffset) { _, offset in
                if let offset { scrollPosition.scrollTo(y: offset) }
            }
            .onScrollPhaseChange { previous, phase in
                if phase == .interacting {
                    readerMoved = true
                    restoreOffset = nil
                }
                if phase == .idle && (previous == .interacting || previous == .decelerating) {
                    model.readingOffset = visibleOffset
                    model.rememberPosition()
                }
            }
            .onChange(
                of:
                    "\(snapshot.item.revision.number)-\(snapshot.since.map(String.init) ?? "all")-\(model.selectedPath ?? "")"
            ) { _, _ in
                readerMoved = false
                restoreOffset = model.readingOffset
                scrollPosition.scrollTo(y: model.readingOffset)
            }
            .onChange(of: model.contextLoaded) { _, _ in model.rememberPosition() }
            .onChange(of: model.revealed) { _, _ in model.rememberPosition() }
            .onDisappear { model.rememberPosition() }
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { model.rememberPosition() }
            }
            if let file = snapshot.files.first(where: { $0.path == model.selectedPath }) {
                ReviewViewedBar(
                    file: file, viewed: snapshot.reviewCount - snapshot.remaining,
                    total: snapshot.reviewCount,
                    saving: model.isSavingViewed || model.isComparing,
                    disabled: model.cachedAt != nil, failure: model.viewedFailure,
                    mark: { Task { await model.setViewed(path: file.path, viewed: !file.viewed) } },
                    retry: { Task { await model.retryViewed() } })
            }
        }
        .safeAreaInset(edge: .bottom) {
            Button("Complete review") { sheet = .feedback }
                .appearanceText(Appearance.label).frame(maxWidth: .infinity, minHeight: 44)
                .buttonStyle(.glass).padding(.horizontal, 20).padding(.bottom, 8)
                .disabled(model.isSavingViewed || model.isComparing).accessibilityIdentifier(
                    "reviewFeedback")
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
            case .feedback: feedback(scope(snapshot)) { sheet = nil }
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
