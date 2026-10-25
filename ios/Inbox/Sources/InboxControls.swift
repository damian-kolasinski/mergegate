import SwiftUI

struct InboxControls: View {
    let snapshot: InboxSnapshot
    @Bindable var model: InboxViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("\(model.project ?? "All projects") · \(needs.count) \(needs.count == 1 ? "item needs" : "items need") you · \(workingCount) in progress")
                .appearanceText(Appearance.body).foregroundStyle(Appearance.secondary)
                .accessibilityIdentifier("inboxSummary")
            ScrollView(.horizontal) { chips }.scrollIndicators(.hidden).scrollClipDisabled()
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private var needs: [InboxItem] {
        snapshot.needsYou.filter { model.project == nil || $0.project == model.project }
    }
    private var workingCount: Int {
        snapshot.withAgents.filter { model.project == nil || $0.project == model.project }.count
    }
    private var chips: some View {
        GlassEffectContainer(spacing: 8) {
            HStack(spacing: 12) {
                chip(.all, label: "All")
                chip(.decisions, label: "Decide · \(needs.filter(\.needsDecision).count)")
                chip(.review, label: "Review · \(needs.filter { !$0.needsDecision }.count)")
            }
        }
    }

    @ViewBuilder private func chip(_ filter: InboxFilter, label: String) -> some View {
        if model.filter == filter {
            chipButton(filter, label: label).buttonStyle(.glassProminent).foregroundStyle(Appearance.onFill)
        } else {
            chipButton(filter, label: label).buttonStyle(.glass)
        }
    }

    private func chipButton(_ filter: InboxFilter, label: String) -> some View {
        Button { model.filter = filter } label: {
            Text(label).appearanceText(Appearance.body)
                .foregroundStyle(model.filter == filter ? Appearance.onFill : Appearance.primary)
                .accessibilityIdentifier("inboxActionLabel")
                .padding(.horizontal, 4).frame(minHeight: 44)
        }
        .buttonBorderShape(.capsule).tint(Appearance.brand)
        .accessibilityAddTraits(model.filter == filter ? .isSelected : [])
        .accessibilityIdentifier("actionFilter-\(filter.rawValue)")
    }
}
