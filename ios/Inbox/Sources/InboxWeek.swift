import SwiftUI

struct InboxWeek: Codable, Sendable {
    struct Repository: Codable, Sendable {
        let project: String
        let days: [UInt32]
    }
    let monday: String
    let today: String
    let repositories: [Repository]
    var total: UInt64 { repositories.flatMap(\.days).reduce(0) { $0 + UInt64($1) } }
}

struct InboxWeekView: View {
    let week: InboxWeek
    var body: some View {
        DisclosureGroup("Closed this week · \(week.total)") {
            Text("Week of \(week.monday) · Merged + abandoned via mergegate")
                .appearanceText(Appearance.caption)
            if week.repositories.isEmpty { Text("No items closed this week yet.") }
            ForEach(week.repositories, id: \.project) { repository in
                VStack(alignment: .leading, spacing: 6) {
                    Text(repository.project).appearanceText(Appearance.rowHeading)
                    ForEach(Array(repository.days.enumerated()), id: \.offset) { index, count in
                        let day = [
                            "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
                            "Sunday",
                        ]
                        if index < day.count {
                            HStack {
                                Text(day[index])
                                Spacer()
                                Text("\(count)")
                            }
                            .accessibilityElement(children: .combine)
                        }
                    }
                }.padding(.vertical, 8)
            }
        }.appearanceText(Appearance.body).accessibilityIdentifier("inboxWeekly")
    }
}
