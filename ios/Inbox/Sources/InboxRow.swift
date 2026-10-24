import SwiftUI

struct InboxRow: View {
    let item: InboxItem

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(item.name).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            Text(item.title).appearanceText(Appearance.rowHeading)
            Text("\(item.status) · Revision \(item.revision)")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            HStack {
                Text("\(item.reviewFiles) \(item.reviewFiles == 1 ? "file" : "files") to review")
                Text("+\(item.additions)").foregroundStyle(Appearance.added)
                Text("−\(item.deletions)").foregroundStyle(Appearance.removed)
            }.appearanceText(Appearance.caption)
        }
        .padding(.vertical, 8)
        .listRowBackground(Appearance.page)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }
}
