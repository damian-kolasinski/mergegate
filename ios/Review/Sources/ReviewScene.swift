import CodeStylingInterface
import ConversationInterface
import DiffDocumentInterface
import DiffView
import SwiftUI

struct ReviewScene<
    Drafts: View, Conversations: View, FileConversations: View, Feedback: View, Rules: View
>: View {
    @State var model: ReviewViewModel
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let styling: any CodeStyling
    let hub: URL
    let drafts: (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> Drafts
    let conversations:
        (
            CommentDraftScope, ConversationLocation?, @escaping (ConversationLocation) -> Void,
            @escaping () -> Void,
            @escaping () -> Void
        ) -> Conversations
    let fileConversations: (CommentDraftScope, String) -> FileConversations
    let feedback:
        (
            CommentDraftScope, @escaping () async -> Void, @escaping () -> Void,
            @escaping () -> Void,
            @escaping () -> Void
        ) -> Feedback
    let rules: (String, @escaping (String) -> Void) -> Rules
    @State private var feedbackSent = false
    @State private var sourceFocus: SourceAnchor?
    @State private var sourceRequest = UUID()
    @State private var fileHeaderHeight: CGFloat = 0
    let openItem: (String) -> Void
    private enum Sheet: Identifiable {
        case guide, files, feedback, details
        case demo(ReviewDemo)
        case drafts(CommentDraftLocation?)
        case conversations(ConversationLocation?)
        var id: String {
            switch self {
            case .details: "details"
            case .demo: "demo"
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
                LazyVStack(
                    alignment: .leading, spacing: 0,
                    pinnedViews: dynamicTypeSize.isAccessibilitySize ? [] : .sectionHeaders
                ) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(snapshot.item.title).appearanceText(Appearance.heading)
                            .accessibilityAddTraits(.isHeader)
                        ReviewScopeBar(
                            snapshot: snapshot, loading: model.isComparing,
                            disabled: model.isSavingViewed || model.cachedAt != nil,
                            compare: { since in Task { await model.compare(since: since) } })
                        ReviewUpdatesView(model: model, revision: snapshot.item.revision.number)
                        ReviewStatusView(snapshot: model.currentDetails ?? snapshot) {
                            sheet = .details
                        }
                        if let demo = (model.currentDetails ?? snapshot).demo {
                            Button("Demo (\(demo.attachments.count)) · from r\(demo.revision)") {
                                sheet = .demo(demo)
                            }
                            .frame(minHeight: 44).accessibilityIdentifier("reviewDemo")
                        }
                    }.padding(.horizontal, 20).padding(.vertical, 12).id("reviewTop")
                    if let failure = model.comparisonFailure {
                        Text(failure).appearanceText(Appearance.caption).foregroundStyle(
                            Appearance.error
                        )
                        .padding(.horizontal, 20).padding(.bottom, 8)
                        .accessibilityIdentifier("comparisonError")
                    }
                    if let index = snapshot.files.firstIndex(where: {
                        $0.path == model.selectedPath
                    }) {
                        let file = snapshot.files[index]
                        Section {
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
                                }, focus: sourceFocus, revealRequest: sourceRequest,
                                revealTopInset: dynamicTypeSize.isAccessibilitySize ? 0 : fileHeaderHeight,
                                didReveal: { sourceFocus = nil }
                            ).id(
                                "\(snapshot.item.revision.number)-\(snapshot.since.map(String.init) ?? "all")-\(file.path)"
                            )
                        } header: {
                            fileControlsLayout {
                                filePicker(file, index: index, count: snapshot.files.count)
                                fileConversations(scope(snapshot), file.path).id(file.path)
                            }
                            .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 24))
                            .padding(.horizontal, 12).padding(.vertical, 4)
                            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                                fileHeaderHeight = $0
                            }
                        }
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
                    model.reviewTime.input()
                    model.readingOffset = visibleOffset
                    model.rememberPosition()
                }
            }
            .onChange(
                of:
                    "\(snapshot.item.revision.number)-\(snapshot.since.map(String.init) ?? "all")-\(model.selectedPath ?? "")"
            ) { _, _ in
                readerMoved = sourceFocus != nil
                restoreOffset = sourceFocus == nil ? model.readingOffset : nil
                if sourceFocus == nil { scrollPosition.scrollTo(y: model.readingOffset) }
            }
            .onChange(of: model.contextLoaded) { _, _ in model.rememberPosition() }
            .onChange(of: model.revealed) { _, _ in model.rememberPosition() }
            .onDisappear { model.rememberPosition() }
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { model.rememberPosition() }
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            VStack(spacing: 0) {
                if (model.currentDetails ?? snapshot).item.isClosed {
                    ReviewNextItem(after: snapshot.item.id, service: model.service) {
                        openItem($0)
                    }
                }
                if let file = snapshot.files.first(where: { $0.path == model.selectedPath }) {
                    ReviewViewedBar(
                        file: file, viewed: snapshot.reviewCount - snapshot.remaining,
                        total: snapshot.reviewCount,
                        saving: model.isSavingViewed || model.isComparing,
                        disabled: model.cachedAt != nil
                            || !(model.currentDetails ?? snapshot).item.isReviewable,
                        failure: model.viewedFailure,
                        mark: { Task { await model.setViewed(path: file.path, viewed: !file.viewed) } },
                        retry: { Task { await model.retryViewed() } }
                    ) { completionActions(snapshot) }
                } else {
                    completionActions(snapshot).padding(.horizontal, 16).padding(.bottom, 8)
                }
            }
        }
        .task(
            id:
                "\(scenePhase)-\((model.currentDetails ?? snapshot).item.state)-\(snapshot.item.revision.number)"
        ) {
            guard scenePhase == .active, (model.currentDetails ?? snapshot).item.isReviewable,
                model.cachedAt == nil
            else {
                await model.reviewTime.stop()
                return
            }
            let session = model.reviewTime.start(revision: snapshot.item.revision.number)
            while !Task.isCancelled {
                do { try await Task.sleep(for: .seconds(30)) } catch { break }
                await model.reviewTime.report()
            }
            await model.reviewTime.stop(session: session)
        }
        .onChange(of: sheet?.id) { _, _ in model.reviewTime.input() }
        .onChange(of: model.selectedPath) { _, _ in model.reviewTime.input() }
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text("\(snapshot.item.name) · r\(snapshot.item.revision.number)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    .lineLimit(1)
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
                }.labelStyle(.iconOnly).tint(Appearance.brand).accessibilityIdentifier("reviewConversations")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .drafts(nil)
                } label: {
                    Label("Comment drafts", systemImage: "text.bubble")
                }.labelStyle(.iconOnly).tint(Appearance.brand).accessibilityIdentifier("reviewDrafts")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .guide
                } label: {
                    Label("Review guide", systemImage: "book")
                }.labelStyle(.iconOnly).tint(Appearance.brand).accessibilityIdentifier("reviewGuide")
            }
        }
        .sheet(item: $sheet, onDismiss: { Task { await model.refreshCurrent() } }) { selection in
            Group {
                switch selection {
                case .details:
                    ReviewDetailsScene(
                        model: ReviewActionsModel(
                            service: model.service, item: snapshot.item.id,
                            beforeLeaving: { await model.reviewTime.settle() }),
                        displayedRevision: snapshot.item.revision.number, close: { sheet = nil },
                        openItem: {
                            sheet = nil
                            openItem($0)
                        }, rules: rules)
                case .demo(let demo):
                    ReviewDemoScene(
                        demo: demo, service: model.sourceService, close: { sheet = nil },
                        onActivity: { model.reviewTime.input() })
                case .conversations(let location):
                    conversations(
                        scope(snapshot), location,
                        { target in Task { await revealSource(target) } },
                        { model.reviewTime.input() }
                    ) { sheet = nil }
                case .drafts(let location):
                    drafts(
                        .init(
                            hub: hub, item: snapshot.item.id, name: snapshot.item.name,
                            revision: snapshot.item.revision.number), location
                    ) { sheet = nil }
                case .feedback:
                    VStack(spacing: 0) {
                        feedback(
                            scope(snapshot), { await model.reviewTime.settle() },
                            { model.reviewTime.input() },
                            { feedbackSent = true }
                        ) { sheet = nil }
                        if feedbackSent {
                            ReviewNextItem(after: snapshot.item.id, service: model.service) {
                                sheet = nil
                                openItem($0)
                            }
                        }
                    }
                case .guide:
                    ReviewGuideScene(
                        guide: snapshot.guide, onActivity: { model.reviewTime.input() }
                    ) { sheet = nil }
                case .files:
                    ReviewFilesScene(
                        files: snapshot.files, selectedPath: model.selectedPath,
                        close: { sheet = nil }
                    ) {
                        path in
                        sourceFocus = nil
                        model.selectedPath = path
                        sheet = nil
                    }
                }
            }.onReceive(
                NotificationCenter.default.publisher(for: UITextField.textDidChangeNotification)
            ) { _ in model.reviewTime.input() }
            .onReceive(
                NotificationCenter.default.publisher(for: UITextView.textDidChangeNotification)
            ) { _ in model.reviewTime.input() }
        }
    }
    private var fileControlsLayout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 0))
            : AnyLayout(HStackLayout(spacing: 0))
    }

    private func filePicker(_ file: ReviewFile, index: Int, count: Int) -> some View {
        ReviewFilePicker(file: file, position: index + 1, count: count) {
            sheet = .files
        }
    }

    private func completionActions(_ snapshot: ReviewSnapshot) -> some View {
        HStack(spacing: 8) {
            Button {
                feedbackSent = false
                sheet = .feedback
            } label: {
                Text("Complete review").appearanceText(Appearance.label)
                    .foregroundStyle(Appearance.onFill)
                    .fixedSize(horizontal: !dynamicTypeSize.isAccessibilitySize, vertical: true)
                    .frame(minHeight: 44)
            }.buttonStyle(.glassProminent).tint(Appearance.brand)
            .disabled(
                model.isSavingViewed || model.isComparing
                    || !(model.currentDetails ?? snapshot).item.isReviewable
            ).accessibilityIdentifier("reviewFeedback")
            Button {
                sheet = .details
            } label: {
                Label("Item details and actions", systemImage: "ellipsis.circle")
                    .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
            }.buttonStyle(.glass).tint(Appearance.brand)
                .accessibilityIdentifier("reviewMore")
                .disabled(model.isSavingViewed || model.isComparing)
        }
    }

    private func revealSource(_ target: ConversationLocation) async {
        guard case .loaded(let before) = model.state,
            let file = before.files.first(where: { $0.path == target.path })
        else { return }
        if let anchor = target.anchors.first,
            !(file.interdiff?.sourceSides ?? [.old, .new]).contains(anchor.side)
        {
            await model.compare(since: nil)
        }
        guard case .loaded(let current) = model.state,
            current.item.revision.number == before.item.revision.number,
            let file = current.files.first(where: { $0.path == target.path }),
            target.anchors.allSatisfy({
                (file.interdiff?.sourceSides ?? [.old, .new]).contains($0.side)
            })
        else {
            model.refreshFailure =
                "This conversation’s source is not available on the displayed comparison."
            return
        }
        model.selectedPath = target.path
        sourceFocus = target.anchors.first
        sourceRequest = UUID()
        readerMoved = true
        restoreOffset = nil
        sheet = nil
    }

    private func scope(_ snapshot: ReviewSnapshot) -> CommentDraftScope {
        .init(
            hub: hub, item: snapshot.item.id, name: snapshot.item.name,
            revision: snapshot.item.revision.number)
    }

}
