import CodeStylingInterface
import DiffDocumentInterface
import SwiftUI

struct ReviewFileReader: View {
    let file: ReviewFile
    let project: String
    let sourceService: ReviewSourceService
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
    @State private var showingPreview = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Size, tier and ranking live in the file pill and its ⓘ; only exceptions stay here.
            if hasNotes {
            VStack(alignment: .leading, spacing: 6) {
                if let oldPath = file.oldPath {
                    Text("From \(oldPath)").appearanceText(Appearance.caption)
                }
                if let comparison = file.interdiff, comparison != .changed {
                    Text(comparison.label).appearanceText(Appearance.label)
                        .foregroundStyle(Appearance.secondary)
                }
                if file.isLFSPointer {
                    Text("Git LFS pointer · The actual file is stored outside this repository.")
                        .appearanceText(Appearance.caption)
                }
                if file.oldMode == "160000" || file.newMode == "160000" {
                    Text(
                        "Submodule · \(file.oldBlob?.prefix(12) ?? "absent") → \(file.newBlob?.prefix(12) ?? "absent")"
                    )
                    .appearanceText(Appearance.caption)
                }
                if file.previewKind != nil {
                    Button(
                        showingPreview
                            ? "Show diff"
                            : file.previewKind == "image" ? "Compare images" : "Preview Markdown"
                    ) {
                        showingPreview.toggle()
                    }.frame(minHeight: 44).accessibilityIdentifier("toggleFilePreview")
                }
            }.padding(.horizontal, 20).padding(.vertical, 10).frame(
                maxWidth: .infinity, alignment: .leading)
            Divider()
            }
            if showingPreview {
                ReviewFilePreview(file: file, project: project, service: sourceService)
            } else {
                switch file.content {
                case .text(let hunks):
                    if hunks.isEmpty {
                        message(
                            "No changed text",
                            "This change contains only file metadata, such as a rename or mode change."
                        )
                    }
                    ReviewSourceReader(
                        file: file, hunks: hunks, project: project, service: sourceService,
                        highlightService: highlightService, item: item, revision: revision,
                        styling: styling,
                        contextLoaded: $contextLoaded, revealed: $revealed, onReady: onReady,
                        onComment: onComment, onReadConversations: onReadConversations,
                        focus: focus, revealRequest: revealRequest,
                        revealTopInset: revealTopInset, didReveal: didReveal
                    )
                case .binary:
                    message("Binary file", "A unified text diff is unavailable for this file.")
                case .tooLarge:
                    message(
                        "Diff too large",
                        "The hub counted this change but could not include its text.")
                case .gone:
                    message(
                        "Diff no longer available",
                        "The hub discarded this item's source diff after it was abandoned.")
                }
            }
        }.onChange(of: revealRequest) { _, request in if request != nil { showingPreview = false } }
    }

    private var hasNotes: Bool {
        file.oldPath != nil || (file.interdiff.map { $0 != .changed } ?? false) || file.isLFSPointer
            || file.oldMode == "160000" || file.newMode == "160000" || file.previewKind != nil
    }

    private func message(_ title: String, _ detail: String) -> some View {
        ContentUnavailableView(title, systemImage: "doc.text", description: Text(detail))
    }
}
