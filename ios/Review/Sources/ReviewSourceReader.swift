import CodeStylingInterface
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
    @Environment(\.scenePhase) private var scenePhase
    @State private var highlights = ReviewHighlightModel()
    private var paintedHunks: [DiffHunk] { highlights.painting?.paint(hunks) ?? hunks }
    @State private var model = ReviewSourceModel()

    private var maximumLine: UInt32 {
        let tail = model.context?.gaps.last?.lines.last
        return max(
            UInt32(hunks.last?.oldRange.upperBound ?? 0), UInt32(hunks.last?.newRange.upperBound ?? 0),
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
                    let count = min(model.revealed[gap.id, default: 0], gap.lines.count)
                    if count > 0 {
                        UnifiedDiffView(
                            hunks: [
                                DiffHunk(
                                    header: "",
                                    lines: highlights.painting?.paint(Array(gap.lines.prefix(count)))
                                        ?? Array(gap.lines.prefix(count)))
                            ],
                            maximumLine: maximumLine, styling: styling)
                    }
                    if count < gap.lines.count {
                        Button {
                            model.revealed[gap.id] = min(count + 20, gap.lines.count)
                        } label: {
                            HStack {
                                Label(
                                    "Show \(min(20, gap.lines.count - count)) lines",
                                    systemImage: "arrow.up.and.down.text.horizontal")
                                Spacer(minLength: 8)
                                Text("\(gap.lines.count - count) unchanged").foregroundStyle(Appearance.secondary)
                            }.appearanceText(Appearance.caption)
                                .padding(.horizontal, 14).frame(maxWidth: .infinity, minHeight: 44)
                        }
                        .accessibilityIdentifier("expandContext-\(gap.id)")
                        .background(Appearance.surface)
                    }
                    if gap.id < hunks.count {
                        UnifiedDiffView(
                            hunks: [paintedHunks[gap.id]], maximumLine: maximumLine, styling: styling)
                    }
                }
            } else {
                if file.oldBlob != nil || file.newBlob != nil {
                    Button {
                        Task { await model.load(service: service, project: project, file: file, hunks: hunks) }
                    } label: {
                        HStack {
                            if model.loading { ProgressView() }
                            Text(
                                model.loading
                                    ? "Loading source context…"
                                    : model.error == nil ? "Load unchanged context" : "Retry source context")
                        }.frame(maxWidth: .infinity, minHeight: 44)
                    }
                    .disabled(model.loading).accessibilityIdentifier("loadSourceContext")
                    .appearanceText(Appearance.caption).background(Appearance.surface)
                    if let error = model.error {
                        Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                            .padding(10).accessibilityIdentifier("sourceContextError")
                    }
                }
                if !hunks.isEmpty {
                    UnifiedDiffView(hunks: paintedHunks, maximumLine: maximumLine, styling: styling)
                }
            }
        }.tint(Appearance.brand)
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                await highlights.follow(
                    service: highlightService, item: item, revision: revision, file: file)
            }
    }
}
