import CodeStylingInterface
import ConversationInterface
import DiffDocumentInterface
import DiffView
import SwiftUI

struct ReviewScene<
    Drafts: View, Conversations: View, FileConversations: View, Feedback: View, Rules: View,
    Annotations: View, Summary: View
>: View {
    @State var model: ReviewViewModel
    @Environment(\.scenePhase) private var scenePhase
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
    /// Wraps a file's reader in its threads and drafts; the Int changes when drafts may have.
    let annotations:
        (CommentDraftScope, String, Int, @escaping (SourceAnnotations) -> AnyView) -> Annotations
    let summary: (CommentDraftScope, @escaping () -> Void) -> Summary
    let feedback:
        (
            CommentDraftScope, ReviewOutcome, Int, @escaping () async -> Void,
            @escaping () -> Void, @escaping () -> Void, @escaping () -> Void
        ) -> Feedback
    let rules: (String, @escaping (String) -> Void) -> Rules
    /// Comments drafted on this iPhone for a revision, which the next review sends.
    let localDrafts: (CommentDraftScope) -> Int
    @State private var feedbackSent = false
    @State private var sourceFocus: SourceAnchor?
    @State private var sourceRequest = UUID()
    @State private var fileHeaderHeight: CGFloat = 0
    @State private var mastheadHeight: CGFloat = 0
    @State private var chromeHidden = false
    @State private var scrollPhase: ScrollPhase = .idle
    @State private var draftCount = 0
    @State private var confirmingAbandon = false
    /// GitHub couldn't close the pull request the abandon tried to.
    @State private var abandonUnreachable: String?
    @State private var notice: String?
    @State private var mastheadAway = false
    @State private var fileTop: Double?
    /// Source lines picked in the gutter or by selection, which the comment pill offers.
    @State private var pickedLines: CommentDraftLocation?
    @State private var draftsVersion = 0
    let openItem: (String) -> Void
    private enum Sheet: Identifiable {
        case guide, files, details, rules, summary
        case feedback(ReviewOutcome)
        case reasons(String)
        case demo(ReviewDemo)
        case drafts(CommentDraftLocation?)
        case conversations(ConversationLocation?)
        var id: String {
            switch self {
            case .details: "details"
            case .rules: "rules"
            case .summary: "summary"
            case .demo: "demo"
            case .guide: "guide"
            case .files: "files"
            case .feedback: "feedback"
            case .reasons(let path): "reasons-\(path)"
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
        .navigationBarTitleDisplayMode(.inline)
        // Returning triggers the inbox refresh, which must follow the pending write.
        .navigationBarBackButtonHidden(model.isSavingViewed || model.isComparing)
        .toolbarVisibility(chromeHidden ? .hidden : .automatic, for: .navigationBar)
        .animation(.easeInOut(duration: 0.2), value: chromeHidden)
        .task(id: scenePhase) {
            guard scenePhase == .active else { return }
            await model.follow()
        }
    }

    private func content(_ snapshot: ReviewSnapshot) -> some View {
        let current = model.currentDetails ?? snapshot
        let order = ReviewReadingOrder(snapshot)
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: .sectionHeaders) {
                VStack(alignment: .leading, spacing: 10) {
                    ReviewMasthead(snapshot: snapshot, current: current) {
                        sheet = .details
                    } comparison: {
                        ReviewComparisonControl(
                            snapshot: snapshot, loading: model.isComparing,
                            disabled: model.isSavingViewed || model.cachedAt != nil,
                            compare: { since in Task { await model.compare(since: since) } }
                        ).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    }
                    ReviewUpdatesView(model: model, revision: snapshot.item.revision.number)
                    if let notice {
                        Text(notice).appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.secondary)
                            .accessibilityIdentifier("itemActionResult")
                    }
                    if let failure = model.comparisonFailure {
                        Text(failure).appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.error)
                            .accessibilityIdentifier("comparisonError")
                    }
                }.padding(.horizontal, 20).padding(.top, 4).padding(.bottom, 12).id("reviewTop")
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                        mastheadHeight = $0
                    }
                if let file = snapshot.files.first(where: { $0.path == model.selectedPath }) {
                    Section {
                        annotations(scope(snapshot), file.path, draftsVersion) { notes in
                        AnyView(ReviewFileReader(
                            file: file, project: snapshot.item.project,
                            sourceService: model.sourceService,
                            highlightService: model.highlightService,
                            item: snapshot.item.id, revision: snapshot.item.revision.number,
                            styling: styling,
                            contextLoaded: $model.contextLoaded, revealed: $model.revealed,
                            onReady: {
                                if !readerMoved { restoreOffset = fileTop ?? model.readingOffset }
                                fileTop = nil
                            },
                            onComment: { anchor in
                                sheet = .drafts(.init(path: file.path, anchor: anchor))
                            },
                            onReadConversations: { anchors in
                                sheet = .conversations(.init(path: file.path, anchors: anchors))
                            }, focus: sourceFocus, revealRequest: sourceRequest,
                            revealTopInset: fileHeaderHeight,
                            didReveal: { sourceFocus = nil },
                            annotations: notes,
                            onSelectLines: { anchor in
                                pickedLines = anchor.map { .init(path: file.path, anchor: $0) }
                            }
                        ))
                        }.id(
                            "\(snapshot.item.revision.number)-\(snapshot.since.map(String.init) ?? "all")-\(file.path)"
                        )
                    } header: {
                        ReviewFilePill(
                            file: file, position: (order.position(of: file.path) ?? 0) + 1,
                            count: order.files.count,
                            previous: order.previous(before: file.path).map { path in { select(path) } },
                            next: order.next(after: file.path).map { path in { select(path) } },
                            openFiles: { sheet = .files },
                            openInfo: { sheet = .reasons(file.path) }
                        ) {
                            fileConversations(scope(snapshot), file.path).id(file.path)
                        }
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
        // Room for the floating bar, so the last lines scroll clear of it.
        .contentMargins(.bottom, 96, for: .scrollContent)
        .accessibilityIdentifier("reviewScroll")
        .scrollPosition($scrollPosition)
        .onScrollGeometryChange(for: ReviewScrollGeometry.self) { geometry in
            ReviewScrollGeometry(
                offset: max(0, geometry.contentOffset.y + geometry.contentInsets.top),
                raw: geometry.contentOffset.y,
                contentHeight: geometry.contentSize.height,
                viewportHeight: geometry.containerSize.height)
        } action: { previous, value in
            followChrome(value, previous: previous)
            visibleOffset = value.offset
            if mastheadAway != (value.offset > mastheadHeight * 0.6) {
                mastheadAway = value.offset > mastheadHeight * 0.6
            }
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
            scrollPhase = phase
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
            pickedLines = nil
            // A file opened fresh starts at its own top, below the item's header.
            let offset = model.readingOffset == 0 ? Double(mastheadHeight) : model.readingOffset
            // The new reader's onReady restores too; it must land on the same spot.
            fileTop = sourceFocus == nil ? offset : nil
            restoreOffset = sourceFocus == nil ? offset : nil
            if sourceFocus == nil { scrollPosition.scrollTo(y: offset) }
            chromeHidden = false
        }
        .onChange(of: model.contextLoaded) { _, _ in model.rememberPosition() }
        .onChange(of: model.revealed) { _, _ in model.rememberPosition() }
        .onDisappear { model.rememberPosition() }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { model.rememberPosition() }
        }
        .overlay(alignment: .top) {
            // Without the navigation bar, keep source from scrolling under the status bar.
            if chromeHidden {
                Color.clear.frame(height: 0)
                    .background(Appearance.page, ignoresSafeAreaEdges: .top)
            }
        }
        .overlay(alignment: .bottom) {
            VStack(spacing: 6) {
                if current.item.isClosed {
                    ReviewNextItem(after: snapshot.item.id, service: model.service) {
                        openItem($0)
                    }
                }
                if let picked = pickedLines {
                    Button {
                        sheet = .drafts(picked)
                        pickedLines = nil
                    } label: {
                        Label("Comment on \(picked.summary)", systemImage: "plus.bubble")
                            .appearanceText(Appearance.label).foregroundStyle(Appearance.onFill)
                            .padding(.horizontal, 6).frame(minHeight: 44)
                    }.buttonStyle(.glassProminent).tint(Appearance.brand)
                        .accessibilityIdentifier("commentOnLines")
                }
                bar(snapshot, current: current, order: order)
            }.padding(.horizontal, 12).padding(.bottom, 4)
                .offset(y: chromeHidden ? 160 : 0)
                .animation(.easeInOut(duration: 0.2), value: chromeHidden)
        }
        .task(
            id:
                "\(scenePhase)-\(current.item.state)-\(snapshot.item.revision.number)"
        ) {
            guard scenePhase == .active, current.item.isReviewable, model.cachedAt == nil
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
        .task(id: "\(snapshot.item.revision.number)-\(sheet == nil)") {
            if sheet == nil { draftCount = localDrafts(scope(snapshot)) }
        }
        .onChange(of: sheet?.id) { _, _ in model.reviewTime.input() }
        .onChange(of: model.selectedPath) { _, _ in model.reviewTime.input() }
        .toolbar {
            ToolbarItem(placement: .principal) {
                // The masthead names the item; once it scrolls away the title takes over.
                if mastheadAway {
                    Text(
                        "\(snapshot.item.name) · +\(snapshot.item.revision.additions ?? 0) −\(snapshot.item.revision.deletions ?? 0)"
                    )
                    .appearanceText(Appearance.caption).monospacedDigit()
                    .foregroundStyle(Appearance.secondary).lineLimit(1)
                }
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {
                    sheet = .conversations(nil)
                } label: {
                    // What waits on the operator, as the inbox counts it.
                    // Toolbars drop a label's title, so the count is drawn beside the icon.
                    HStack(spacing: 4) {
                        Image(
                            systemName: current.needsYou > 0
                                ? "bubble.left.and.bubble.right.fill" : "bubble.left.and.bubble.right")
                        if current.needsYou > 0 {
                            Text("\(current.needsYou)").appearanceText(Appearance.label)
                                .monospacedDigit()
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(
                        current.needsYou > 0
                            ? "Conversations, \(current.needsYou) need you" : "Conversations")
                }.accessibilityIdentifier("reviewConversations")
                if let demo = current.demo {
                    Button {
                        sheet = .demo(demo)
                    } label: {
                        Label("Demo (\(demo.attachments.count))", systemImage: "play.rectangle")
                    }.accessibilityIdentifier("reviewDemo")
                }
                ReviewMoreMenu(
                    snapshot: snapshot, current: current,
                    comparing: model.isComparing || model.cachedAt != nil,
                    compare: { since in Task { await model.compare(since: since) } },
                    open: { destination in
                        switch destination {
                        case .guide: sheet = .guide
                        case .drafts: sheet = .drafts(nil)
                        case .details: sheet = .details
                        case .rules: sheet = .rules
                        }
                    },
                    prioritize: { priority in
                        act(.priority) { $0.priority = priority }
                    },
                    sync: { act(.sync) },
                    requestChanges: { sheet = .feedback(.changesRequested) },
                    abandon: { confirmingAbandon = true })
            }
        }
        .tint(Appearance.brand)
        .confirmationDialog(
            "Abandon \(snapshot.item.name) without merging?", isPresented: $confirmingAbandon,
            titleVisibility: .visible
        ) {
            Button("Abandon item", role: .destructive) { act(.abandon) }
        } message: {
            Text(
                current.item.backend == "github"
                    ? "Closes its GitHub pull request, deletes its branch and drops its diffs."
                    : "Drops the item's diffs. Add a reason under Item details to tell the agent why."
            )
        }
        .confirmationDialog(
            "Abandon \(snapshot.item.name) in mergegate only?",
            isPresented: confirmingLeaveGithub, titleVisibility: .visible
        ) {
            Button("Abandon in mergegate only", role: .destructive) {
                act(.abandonLeavingGithub)
            }
        } message: {
            Text(leaveGithubMessage)
        }
        .sheet(
            item: $sheet,
            onDismiss: {
                draftsVersion += 1
                Task { await model.refreshCurrent() }
            }
        ) { selection in
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
                case .rules:
                    NavigationStack {
                        rules(snapshot.item.project) { item in
                            sheet = nil
                            openItem(item)
                        }.toolbar { Button("Done") { sheet = nil } }
                    }
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
                    drafts(scope(snapshot), location) { sheet = nil }
                case .feedback(let outcome):
                    VStack(spacing: 0) {
                        feedback(
                            scope(snapshot), outcome, snapshot.remaining,
                            { await model.reviewTime.settle() },
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
                case .summary:
                    summary(scope(snapshot)) { sheet = nil }
                case .guide:
                    ReviewGuideScene(
                        guide: snapshot.guide, onActivity: { model.reviewTime.input() }
                    ) { sheet = nil }
                case .files:
                    ReviewFilesScene(
                        snapshot: snapshot, selectedPath: model.selectedPath,
                        close: { sheet = nil }
                    ) { path in
                        select(path)
                        sheet = nil
                    }
                case .reasons(let path):
                    if let file = snapshot.files.first(where: { $0.path == path }) {
                        ReviewFileReasons(file: file) { sheet = nil }
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

    private func bar(_ snapshot: ReviewSnapshot, current: ReviewSnapshot, order: ReviewReadingOrder)
        -> some View
    {
        let file = snapshot.files.first { $0.path == model.selectedPath }
        let busy = model.isSavingViewed || model.isComparing
        return ReviewBar(
            file: file, viewed: snapshot.reviewCount - snapshot.remaining,
            total: snapshot.reviewCount, saving: busy,
            disabled: model.cachedAt != nil || !current.item.isReviewable,
            failure: model.viewedFailure,
            primary: ReviewPrimaryAction(
                // An older revision on screen can't be approved; reopening loads the newer one.
                snapshot: snapshot,
                reviewable: current.item.isReviewable && model.cachedAt == nil
                    && (model.latestRevision ?? snapshot.item.revision.number)
                        == snapshot.item.revision.number,
                order: order, selected: model.selectedPath,
                drafts: draftCount + current.hubDraftCount),
            canComplete: current.item.isReviewable,
            mark: {
                guard let file else { return }
                Task { await model.setViewed(path: file.path, viewed: !file.viewed) }
            },
            retry: { Task { await model.retryViewed() } },
            comment: { sheet = .summary },
            perform: { action in perform(action, order: order) },
            complete: { outcome in
                feedbackSent = false
                sheet = .feedback(outcome)
            })
    }

    private func perform(_ action: ReviewPrimaryAction, order: ReviewReadingOrder) {
        switch action {
        case .viewedAndNext:
            guard let path = model.selectedPath else { return }
            Task {
                await model.setViewed(path: path, viewed: true)
                guard model.viewedFailure == nil, case .loaded(let after) = model.state,
                    let next = ReviewReadingOrder(after).nextUnviewed(after: path)
                else { return }
                select(next)
            }
        case .nextUnviewed(let path): select(path)
        case .send:
            feedbackSent = false
            sheet = .feedback(.comment)
        case .approve:
            feedbackSent = false
            sheet = .feedback(.approved)
        }
    }

    private func select(_ path: String) {
        sourceFocus = nil
        model.selectedPath = path
    }

    // Hide the bars while reading down a file, as Safari does, and bring them back on any
    // scroll up, near the top or at the end. Only the reader's own scrolling counts, so a
    // layout change can't toggle them.
    private func followChrome(_ value: ReviewScrollGeometry, previous: ReviewScrollGeometry) {
        // UIKit's offset ignores the inset the bar itself adds or removes, so toggling the bar
        // can't feed back into this decision mid-drag.
        let delta = value.raw - previous.raw
        if value.offset < mastheadHeight
            || value.offset + value.viewportHeight >= value.contentHeight - 24
        {
            chromeHidden = false
        } else if scrollPhase == .interacting || scrollPhase == .decelerating {
            if delta > 8 { chromeHidden = true } else if delta < -8 { chromeHidden = false }
        }
    }

    private var confirmingLeaveGithub: Binding<Bool> {
        Binding(
            get: { abandonUnreachable != nil },
            set: { if !$0 { abandonUnreachable = nil } })
    }

    private var leaveGithubMessage: String {
        let failure = abandonUnreachable ?? ""
        return failure
            + " If GitHub no longer has the repository, its pull request and branch stay as they are there."
    }

    private func act(
        _ action: ReviewActionsModel.Action, prepare: @escaping (ReviewActionsModel) -> Void = { _ in }
    ) {
        Task {
            let actions = ReviewActionsModel(
                service: model.service, item: model.item,
                beforeLeaving: { await model.reviewTime.settle() })
            await actions.load()
            prepare(actions)
            await actions.send(action)
            notice = actions.error ?? actions.result
            if actions.githubUnreachable { abandonUnreachable = actions.error }
            await model.refreshCurrent()
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
        chromeHidden = false
        sheet = nil
    }

    private func scope(_ snapshot: ReviewSnapshot) -> CommentDraftScope {
        .init(
            hub: hub, item: snapshot.item.id, name: snapshot.item.name,
            revision: snapshot.item.revision.number)
    }
}

extension CommentDraftLocation {
    /// The pill's short name for the picked lines, as "After · L15–16".
    fileprivate var summary: String {
        let side = anchor.side == .old ? "Before" : "After"
        let lines =
            anchor.startLine == anchor.endLine
            ? "L\(anchor.startLine)" : "L\(anchor.startLine)–\(anchor.endLine)"
        return "\(side) · \(lines)"
    }
}
