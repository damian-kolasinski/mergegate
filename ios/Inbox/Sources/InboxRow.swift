import SwiftUI

struct InboxRow: View {
    let item: InboxItem
    let triage: InboxTriage?
    let error: String?
    let retry: () -> Void

    var body: some View {
        Group {
            if item.needsDecision {
                InboxDecisionCard(item: item, triage: triage, error: error, retry: retry)
            } else {
                VStack(alignment: .leading, spacing: 7) {
                    Text(item.title).appearanceText(Appearance.rowHeading)
                    Text("\(item.name) · r\(item.revision) · \(item.status)")
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    InboxProgress(item: item, triage: triage)
                    if let error {
                        Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                        Button("Retry item details", action: retry).frame(minHeight: 44)
                    }
                    Divider().padding(.top, 8)
                }.padding(.horizontal, 6).padding(.vertical, 6)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }
}
