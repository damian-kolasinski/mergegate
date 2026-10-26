import CodeStylingInterface
import ConversationInterface
import DiffDocumentInterface
import DiffView
import SwiftUI

struct ReviewSourceReader: View {
    let file: ReviewFile
    let hunks: [DiffHunk]
    let project: String
    let service: ReviewSourceService
    let highlightService: ReviewHighlightService
    let item: String
    let revision: UInt32
    let styling: any CodeStyling
    @Binding var contextLoaded: Bool
    @Binding var revealed: [Int: Int]
    let onReady: () -> Void
    let onComment: (SourceAnchor) -> Void
    let onReadConversations: ([SourceAnchor]) -> Void
    var focus: SourceAnchor? = nil
    var revealRequest: UUID? = nil
    var revealTopInset: CGFloat = 0
    var didReveal: (() -> Void)? = nil
    var annotations: SourceAnnotations = .none
    var onSelectLines: ((SourceAnchor?) -> Void)? = nil
    @Environment(\.scenePhase) private var scenePhase

    /// A submitted hunk, cut after each line a thread or draft ends on, with its cards between.
    @ViewBuilder private func annotated(_ hunk: DiffHunk, id: String) -> some View {
        // An interdiff's Before side replays the earlier revision; cards there would sit
        // under the wrong lines, as the web also leaves them off.
        let sides = file.interdiff?.sourceSides ?? [.old, .new]
        let segments = ReviewAnnotatedLines.split(
            hunk, marks: annotations.marks.filter { sides.contains($0.side) })
        // Reveal once the cards are in, in the one block holding the target's first line;
        // cards arriving later would push it away.
        let target = segments.firstIndex { segment in
            focus.map { ReviewAnnotatedLines.shows(segment.hunk, start: $0) } ?? false
        }
        ForEach(Array(segments.enumerated()), id: \.offset) { index, segment in
            UnifiedDiffView(
                hunks: [segment.hunk], maximumLine: maximumLine, styling: styling,
                sourceSides: sides,
                onComment: onComment, onReadConversations: onReadConversations,
                onSelectLines: onSelectLines,
                focus: annotations.ready && index == target ? focus : nil,
                revealRequest: revealRequest,
                revealTopInset: revealTopInset, didReveal: didReveal
            )
            // A short segment would otherwise center, moving its gutter off the others'.
            .frame(maxWidth: .infinity, alignment: .leading)
            .id(index == 0 ? id : "\(id)-\(index)")
            ForEach(segment.marks, id: \.self) { mark in annotations.card(mark) }
        }
    }
    @State private var highlights = ReviewHighlightModel()
    private var paintedHunks: [DiffHunk] { highlights.painting?.paint(hunks) ?? hunks }
    @State private var model = ReviewSourceModel()

    private var maximumLine: UInt32 {
        let tail = model.context?.gaps.last?.lines.last
        return max(
            UInt32(hunks.last?.oldRange.upperBound ?? 0),
            UInt32(hunks.last?.newRange.upperBound ?? 0),
            tail?.oldLine ?? 0, tail?.newLine ?? 0)
    }

    var body: some View {
        VStack(spacing: 0) {
            if let context = model.context {
                if context.gaps.allSatisfy({ $0.lines.isEmpty }) {
                    Text("All source context is already visible").appearanceText(Appearance.caption)
                        .foregroundStyle(Appearance.secondary).padding(10)
                }
                ForEach(context.gaps) { gap in
                    let count = min(revealed[gap.id, default: 0], gap.lines.count)
                    if count > 0 {
                        UnifiedDiffView(
                            hunks: [
                                DiffHunk(
                                    header: "",
                                    lines: highlights.painting?.paint(
                                        Array(gap.lines.prefix(count)))
                                        ?? Array(gap.lines.prefix(count)))
                            ],
                            // The hub accepts anchors only within its submitted diff hunks.
                            maximumLine: maximumLine, styling: styling,
                            sourceSides: file.interdiff?.sourceSides ?? [.old, .new],
                            onReadConversations: onReadConversations)
                    }
                    if count < gap.lines.count {
                        Button {
                            revealed[gap.id] = min(count + 20, gap.lines.count)
                        } label: {
                            HStack {
                                Label(
                                    "Show \(min(20, gap.lines.count - count)) lines",
                                    systemImage: "arrow.up.and.down.text.horizontal")
                                Spacer(minLength: 8)
                                Text("\(gap.lines.count - count) unchanged").foregroundStyle(
                                    Appearance.secondary)
                            }.appearanceText(Appearance.caption)
                                .padding(.horizontal, 14).frame(maxWidth: .infinity, minHeight: 44)
                        }
                        .accessibilityIdentifier("expandContext-\(gap.id)")
                        .background(Appearance.surface)
                    }
                    if gap.id < hunks.count {
                        annotated(paintedHunks[gap.id], id: "reviewHunk-\(gap.id)")
                    }
                }
            } else {
                if file.oldBlob != nil || file.newBlob != nil {
                    Button {
                        Task {
                            await model.load(
                                service: service, project: project, file: file, hunks: hunks)
                            contextLoaded = model.context != nil
                        }
                    } label: {
                        HStack {
                            if model.loading { ProgressView() }
                            Text(
                                model.loading
                                    ? "Loading source context…"
                                    : model.error == nil
                                        ? "Load unchanged context" : "Retry source context")
                        }.frame(maxWidth: .infinity, minHeight: 44)
                    }
                    .disabled(model.loading).accessibilityIdentifier("loadSourceContext")
                    .appearanceText(Appearance.caption).background(Appearance.surface)
                    if let error = model.error {
                        Text(error).appearanceText(Appearance.caption).foregroundStyle(
                            Appearance.error
                        )
                        .padding(10).accessibilityIdentifier("sourceContextError")
                    }
                }
                if !hunks.isEmpty {
                    ForEach(Array(paintedHunks.enumerated()), id: \.offset) { index, hunk in
                        annotated(hunk, id: "reviewHunk-\(index)")
                    }
                }
            }
        }.tint(Appearance.brand)
            .task {
                if contextLoaded {
                    await model.load(service: service, project: project, file: file, hunks: hunks)
                }
                onReady()
            }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                await highlights.follow(
                    service: highlightService, item: item, revision: revision, file: file)
            }
    }
}
