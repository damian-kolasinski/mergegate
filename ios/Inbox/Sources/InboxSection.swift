import Foundation

// The web's sections (ui/inbox/inbox.tsx), so the operator reads the same inbox on both clients.
enum InboxSection: String, CaseIterable, Identifiable {
    case needsYou = "Needs you"
    case withAgents = "With agents"
    case merging = "Merging"
    var id: Self { self }
}

struct InboxGroup: Identifiable {
    let section: InboxSection
    let items: [InboxItem]
    var id: InboxSection { section }
}

extension InboxSnapshot {
    var allItems: [InboxItem] { needsYou + withAgents + merging + closed }
    var projects: [String] {
        Set(
            allItems.map(\.project) + (rules ?? []).map(\.project)
                + (weekly?.repositories.map(\.project) ?? [])
        ).sorted()
    }

    /// Needs you always shows, so its empty state can say why; the others only when they hold
    /// items. Items keep the hub's order, which already sorts by priority, stack and age.
    func groups(project: String?) -> [InboxGroup] {
        let lists: [(InboxSection, [InboxItem])] = [
            (.needsYou, needsYou), (.withAgents, withAgents), (.merging, merging),
        ]
        return lists.compactMap { section, items in
            let shown = items.matching(project)
            return section == .needsYou || !shown.isEmpty
                ? InboxGroup(section: section, items: shown) : nil
        }
    }

    func closed(project: String?) -> [InboxItem] { closed.matching(project) }
}

extension [InboxItem] {
    fileprivate func matching(_ project: String?) -> [InboxItem] {
        filter { project == nil || $0.project == project }
    }
}
