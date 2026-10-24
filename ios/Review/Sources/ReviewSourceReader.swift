import DiffDocumentInterface
import DiffView
import SwiftUI

struct ReviewSourceReader: View {
    let file: ReviewFile
    let hunks: [DiffHunk]
    let project: String
    let service: ReviewSourceService
    @State private var model = ReviewSourceModel()

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
                        UnifiedDiffView(hunks: [DiffHunk(header: "", lines: Array(gap.lines.prefix(count)))])
                    }
                    if count < gap.lines.count {
                        Button {
                            model.revealed[gap.id] = min(count + 20, gap.lines.count)
                        } label: {
                            Label(
                                "Show \(min(20, gap.lines.count - count)) unchanged lines (\(gap.lines.count - count) hidden)",
                                systemImage: "arrow.up.and.down.text.horizontal"
                            )
                            .frame(maxWidth: .infinity, minHeight: 44)
                        }
                        .accessibilityIdentifier("expandContext-\(gap.id)")
                        .padding(.horizontal, 10)
                    }
                    if gap.id < hunks.count { UnifiedDiffView(hunks: [hunks[gap.id]]) }
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
                    .padding(.horizontal, 10)
                    if let error = model.error {
                        Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                            .padding(10).accessibilityIdentifier("sourceContextError")
                    }
                }
                if !hunks.isEmpty { UnifiedDiffView(hunks: hunks) }
            }
        }.tint(Appearance.brand)
    }
}
