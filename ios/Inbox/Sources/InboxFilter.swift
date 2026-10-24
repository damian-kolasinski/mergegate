import Foundation

enum InboxFilter: String, CaseIterable, Identifiable {
    case all = "All states"
    case decisions = "Decide"
    case review = "Review"
    case needsYou = "Needs you"
    case working = "In progress"
    case merging = "Merging"
    case closed = "Closed today"
    var id: Self { self }
}

struct InboxGroup: Identifiable {
    let title: String
    let items: [InboxItem]
    var id: String { title }
}

extension InboxSnapshot {
    var allItems: [InboxItem] { needsYou + withAgents + merging + closed }
    var projects: [String] { Set(allItems.map(\.project)).sorted() }

    func groups(project: String?, filter: InboxFilter) -> [InboxGroup] {
        let matching: ([InboxItem]) -> [InboxItem] = { items in
            items.filter { project == nil || $0.project == project }
        }
        let needs = matching(needsYou)
        let groups: [(InboxFilter, String, [InboxItem])] = [
            (.decisions, "Decide next", needs.filter(\.needsDecision)),
            (.review, "Review next", needs.filter { !$0.needsDecision }),
            (.working, "In progress", matching(withAgents)),
            (.merging, "Merging", matching(merging)),
            (.closed, "Closed today", matching(closed)),
        ]
        return groups.compactMap { state, title, items in
            guard (filter == .all || filter == state || (filter == .needsYou && (state == .decisions || state == .review))) && !items.isEmpty else { return nil }
            return InboxGroup(title: title, items: items)
        }
    }
}
