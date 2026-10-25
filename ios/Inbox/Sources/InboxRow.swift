import SwiftUI

struct InboxRow: View {
    let item: InboxItem
    let triage: InboxTriage?
    let error: String?
    let retry: () -> Void
    let openReview: () -> Void
    let openDecisions: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if item.needsDecision {
                InboxDecisionCard(
                    item: item, triage: triage, error: error, retry: retry, openReview: openReview,
                    openDecisions: openDecisions)
            } else {
                Button(action: openReview) {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 7) {
                            Text(item.title).appearanceText(Appearance.rowHeading)
                            Text("\(item.name) · r\(item.revision) · \(item.status)")
                                .appearanceText(Appearance.caption).foregroundStyle(
                                    Appearance.secondary)
                            InboxItemStatus(item: item)
                            InboxProgress(item: item, triage: triage)
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.padding(.horizontal, 6).padding(.vertical, 6)
                        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                        .contentShape(Rectangle())
                }.buttonStyle(.plain).accessibilityHint("Open review")
                    .accessibilityIdentifier("openReview-\(item.id)")
                if let error {
                    Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                    Button("Retry item details", action: retry).frame(minHeight: 44)
                }
                Divider().padding(.top, 8)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }
}
