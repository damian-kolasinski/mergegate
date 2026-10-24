import SwiftUI

struct InboxControls: View {
    let snapshot: InboxSnapshot
    @Bindable var model: InboxViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Your next move").appearanceText(Appearance.displayHeading)
                    .accessibilityAddTraits(.isHeader)
                Text("\(needs.count) items need you · \(workingCount) in progress")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            }
            ScrollView(.horizontal) { chips }.scrollIndicators(.hidden)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private var needs: [InboxItem] {
        snapshot.needsYou.filter { model.project == nil || $0.project == model.project }
    }
    private var workingCount: Int {
        snapshot.withAgents.filter { model.project == nil || $0.project == model.project }.count
    }
    private var chips: some View {
        HStack(spacing: 6) {
            chip(.all, label: "All")
            chip(.decisions, label: "Decide · \(needs.filter(\.needsDecision).count)")
            chip(.review, label: "Review · \(needs.filter { !$0.needsDecision }.count)")
        }
    }
    private func chip(_ filter: InboxFilter, label: String) -> some View {
        Button { model.filter = filter } label: {
            Text(label).appearanceText(Appearance.body)
                .padding(.horizontal, 16).frame(minHeight: 44)
                .foregroundStyle(model.filter == filter ? Appearance.onFill : Appearance.primary)
                .background(model.filter == filter ? Appearance.brand : Color.clear, in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(model.filter == filter ? .isSelected : [])
        .accessibilityIdentifier("actionFilter-\(filter.rawValue)")
    }
}
