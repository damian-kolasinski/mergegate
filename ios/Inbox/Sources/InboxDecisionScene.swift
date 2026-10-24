import SwiftUI

struct InboxDecisionScene: View {
    let item: InboxItem
    let triage: InboxTriage
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("\(item.name) · Revision \(item.revision)")
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    Text(item.title).appearanceText(Appearance.heading)
                    if let target = item.target {
                        Text("Target: \(target)").appearanceText(Appearance.caption)
                    }
                    if let note = item.note { Text(note).appearanceText(Appearance.body) }
                }
                ForEach(triage.decisions) { decision in
                    VStack(alignment: .leading, spacing: 12) {
                        Text(decision.kind).appearanceText(Appearance.rowHeading)
                            .foregroundStyle(Appearance.brand).accessibilityAddTraits(.isHeader)
                        if let path = decision.path {
                            Text(path).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                                .accessibilityLabel("File \(path)")
                        }
                        Text("Original comment").appearanceText(Appearance.label)
                        Text(decision.comment).appearanceText(Appearance.body)
                        if let response = decision.response {
                            Text("Latest response").appearanceText(Appearance.label)
                            Text(response).appearanceText(Appearance.body)
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                    Divider()
                }
            }.padding(24).frame(maxWidth: 680)
                .frame(maxWidth: .infinity)
        }
        .foregroundStyle(Appearance.primary).background(Appearance.page)
        .safeAreaInset(edge: .top, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("Decision details").appearanceText(Appearance.heading)
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 12)
                Button { dismiss() } label: {
                    Text("Done").appearanceText(Appearance.label).frame(minWidth: 44, minHeight: 44)
                        .contentShape(Rectangle())
                }.buttonStyle(.plain).foregroundStyle(Appearance.brand)
                    .accessibilityIdentifier("closeDecisionDetails")
            }.padding(.horizontal, 24).padding(.top, 12).background(Appearance.page)
        }
        .presentationDragIndicator(.visible)
    }
}
