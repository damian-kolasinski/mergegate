import SwiftUI

struct ReviewGuideScene: View {
    let guide: String?
    let close: () -> Void

    var body: some View {
        ScrollView {
            if let guide, !guide.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                Text(guide).appearanceText(Appearance.body).textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(20)
            } else {
                ContentUnavailableView(
                    "No review guide", systemImage: "text.alignleft",
                    description: Text("This item was submitted without a guide."))
            }
        }.background(Appearance.page).foregroundStyle(Appearance.primary)
            .safeAreaInset(edge: .top, spacing: 0) {
                ReviewSheetHeader(title: "Review guide", closeIdentifier: "closeReviewGuide", close: close)
            }.presentationDragIndicator(.visible)
    }
}
