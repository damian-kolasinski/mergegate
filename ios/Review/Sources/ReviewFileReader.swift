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
    let onComment: (SourceAnchor) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                if let oldPath = file.oldPath {
                    Text("From \(oldPath)").appearanceText(Appearance.caption)
                }
                Text(metadata).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                if let reason = file.rankingReason {
                    Text(reason).appearanceText(Appearance.body)
                }
            }.padding(.horizontal, 20).padding(.vertical, 10).frame(
                maxWidth: .infinity, alignment: .leading)
            Divider()
            switch file.content {
            case .text(let hunks):
                if hunks.isEmpty {
                    message(
                        "No changed text",
                        "This change contains only file metadata, such as a rename or mode change.")
                }
                ReviewSourceReader(
                    file: file, hunks: hunks, project: project, service: sourceService,
                    highlightService: highlightService, item: item, revision: revision, styling: styling,
                    onComment: onComment)
            case .binary:
                message("Binary file", "A unified text diff is unavailable for this file.")
            case .tooLarge:
                message("Diff too large", "The hub counted this change but could not include its text.")
            case .gone:
                message(
                    "Diff no longer available",
                    "The hub discarded this item's source diff after it was abandoned.")
            }
        }
    }

    private var metadata: String {
        var parts = [file.change, file.tier == .trusted ? "Trusted" : "Review"]
        if let additions = file.additions, let deletions = file.deletions {
            parts.append("+\(additions) −\(deletions)")
        }
        if let old = file.oldMode, let new = file.newMode, old != new {
            parts.append("\(old) → \(new)")
        }
        return parts.joined(separator: " · ")
    }

    private func message(_ title: String, _ detail: String) -> some View {
        ContentUnavailableView(title, systemImage: "doc.text", description: Text(detail))
    }
}
