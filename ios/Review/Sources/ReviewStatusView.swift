import SwiftUI

struct ReviewStatusView: View {
    let snapshot: ReviewSnapshot
    let details: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if snapshot.item.isClosed {
                Text(
                    snapshot.item.state == "merged"
                        ? "Merged into \(snapshot.item.target)" : "Abandoned"
                )
                .appearanceText(Appearance.rowHeading).accessibilityIdentifier("reviewClosed")
            } else if let blocked = snapshot.blocked {
                Button(action: details) {
                    Label(blocked.label, systemImage: "exclamationmark.triangle")
                        .frame(minHeight: 44)
                }.accessibilityIdentifier("reviewBlocked")
            }
            if snapshot.agent == "away" {
                Text("No agent is waiting. Feedback stays on the hub until this item is resumed.")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            }
            if let priority = snapshot.item.priority {
                Text("\(priority)\(snapshot.item.note.map { " · " + $0 } ?? "")")
                    .appearanceText(Appearance.caption)
            }
            ForEach(snapshot.bots ?? [], id: \.login) { bot in
                Text("\(bot.name) \(bot.state)").appearanceText(Appearance.caption)
            }
        }
    }
}
