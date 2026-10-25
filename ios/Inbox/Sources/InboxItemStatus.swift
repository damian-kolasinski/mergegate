import SwiftUI

struct InboxItemStatus: View {
    let item: InboxItem
    var showPriority = true
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if showPriority, let priority = item.priority {
                Text("\(priority)\(item.note.map { " · " + $0 } ?? "")")
            } else if let note = item.note {
                Text(note)
            }
            if let changed = item.changedSince {
                Text(
                    "\(changed.files) changed since \(changed.approved ? "approved " : "")r\(changed.revision)"
                )
            }
            if let parent = item.parent {
                Text("\(item.behind == true ? "Behind" : "Stacked on") \(parent)")
            }
            if let blocked = item.blocked {
                switch blocked.kind {
                case "held":
                    Text(
                        "Merge held · \(blocked.reason?.components(separatedBy: "\n\n").first ?? "")"
                    )
                case "waiting": Text("Approved · Waiting for \(blocked.parent ?? "parent")")
                case "behind": Text("Approved · Waiting for rebase")
                case "conflict":
                    Text(
                        "Conflicts with \(item.target ?? "target"): \((blocked.paths ?? []).joined(separator: ", "))"
                    )
                default: EmptyView()
                }
            }
            if let date = updatedDate {
                Text("Updated \(date.formatted(date: .abbreviated, time: .shortened))")
            }
            if item.demo == true { Label("Demo attached", systemImage: "play.rectangle") }
        }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
    }

    private var updatedDate: Date? {
        guard let updated = item.updatedAt else { return nil }
        return try? Date(updated, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true))
    }
}
