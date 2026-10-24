import SwiftUI

struct InboxDecisionCard: View {
    let item: InboxItem
    let triage: InboxTriage?
    let error: String?
    let retry: () -> Void
    @State private var details: DecisionPresentation?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(item.name).monospaced()
                if let priority = item.priority {
                    Text(priority.uppercased()).padding(.horizontal, 5).padding(.vertical, 2)
                        .foregroundStyle(Appearance.onTintBrand)
                        .background(Appearance.tintBrand, in: RoundedRectangle(cornerRadius: 4))
                }
                Spacer(minLength: 0)
                Text("r\(item.revision)").accessibilityLabel("Revision \(item.revision)")
            }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            VStack(alignment: .leading, spacing: 6) {
                Text(triage?.decisions.first?.headline ?? "A decision needs your attention")
                    .appearanceText(Appearance.rowHeading)
                Text(item.title).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            }
            if let decision = triage?.decisions.first {
                Text(decision.preview).appearanceText(Appearance.body)
                    .lineLimit(3).fixedSize(horizontal: false, vertical: true)
                ViewThatFits(in: .horizontal) {
                    HStack {
                        Text(decisionLabel).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        Spacer(minLength: 8)
                        detailsButton
                    }
                    VStack(alignment: .leading, spacing: 6) {
                        Text(decisionLabel).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        detailsButton
                    }
                }
            } else if error == nil {
                Text(triage == nil ? "Loading decision details…" : "Decision changed. Refresh the inbox.")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            }
            InboxProgress(item: item, triage: triage, onSurface: true)
            if let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                Button("Retry item details", action: retry).frame(minHeight: 44)
            }
        }
        .padding(16).frame(maxWidth: .infinity, alignment: .leading)
        .background(Appearance.surface, in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Appearance.border, lineWidth: 0.5))
        .sheet(item: $details) { selection in
            InboxDecisionScene(item: selection.item, triage: selection.triage)
        }
    }

    private var decisionLabel: String {
        let count = triage?.decisions.count ?? 0
        return count > 1 ? "\(count) pending decisions" : triage?.decisions.first?.kind ?? "Decision"
    }
    private var detailsButton: some View {
        Button {
            if let triage { details = DecisionPresentation(item: item, triage: triage) }
        } label: {
            Label("Read details", systemImage: "arrow.up.right")
                .appearanceText(Appearance.label)
                .padding(.horizontal, 12).frame(minHeight: 44)
        }.buttonStyle(.glass).tint(Appearance.brand).accessibilityIdentifier("decisionDetails-\(item.id)")
    }
}

// Keep the displayed revision and text together while the inbox refreshes.
private struct DecisionPresentation: Identifiable {
    let item: InboxItem
    let triage: InboxTriage
    var id: String { item.id }
}
