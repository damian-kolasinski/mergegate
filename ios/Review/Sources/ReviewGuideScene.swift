import SwiftUI

struct ReviewGuideScene: View {
    let guide: String?
    var onActivity: () -> Void = {}
    let close: () -> Void

    var body: some View {
        ScrollView {
            if let guide, !guide.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                ReviewMarkdown(source: guide)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(20)
            } else {
                ContentUnavailableView(
                    "No review guide", systemImage: "text.alignleft",
                    description: Text("This item was submitted without a guide."))
            }
        }.onScrollPhaseChange { _, phase in
            if phase == .interacting || phase == .idle { onActivity() }
        }
        .background(Appearance.page).foregroundStyle(Appearance.primary)
        .safeAreaInset(edge: .top, spacing: 0) {
            ReviewSheetHeader(
                title: "Review guide", closeIdentifier: "closeReviewGuide", close: close)
        }.presentationDragIndicator(.visible)
    }
}
