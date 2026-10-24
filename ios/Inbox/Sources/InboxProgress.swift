import SwiftUI

struct InboxProgress: View {
    let item: InboxItem
    let triage: InboxTriage?
    var onSurface = false

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                remaining
                Spacer(minLength: 4)
                delta
            }
            VStack(alignment: .leading, spacing: 6) {
                remaining
                delta
            }
        }.appearanceText(Appearance.caption)
    }

    @ViewBuilder private var remaining: some View {
        if !item.isClosed {
            if let triage {
                HStack(spacing: 8) {
                    ProgressView(value: Double(triage.reviewFiles - triage.remainingFiles), total: Double(max(1, triage.reviewFiles)))
                        .tint(Appearance.brand).frame(width: 32).accessibilityHidden(true)
                    Text("\(triage.remainingFiles) of \(triage.reviewFiles) files left")
                        .foregroundStyle(Appearance.secondary)
                        .accessibilityLabel("\(triage.remainingFiles) of \(triage.reviewFiles) review files remaining")
                }
            } else {
                Text("\(item.reviewFiles) review files · Loading progress…")
                    .foregroundStyle(Appearance.secondary)
            }
        }
    }
    private var delta: some View {
        HStack(spacing: 5) {
            if item.additions > 0 {
                Text("+\(item.additions)").foregroundStyle(onSurface ? Appearance.onTintSuccess : Appearance.added)
                    .accessibilityLabel("\(item.additions) added lines")
            }
            if item.deletions > 0 {
                Text("−\(item.deletions)").foregroundStyle(onSurface ? Appearance.onTintError : Appearance.removed)
                    .accessibilityLabel("\(item.deletions) removed lines")
            }
        }
    }
}
