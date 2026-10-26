import SwiftUI

// Everything the reader needs less often, in one labeled menu like the web's ⋯: what to read,
// the item itself, then actions that change it.
struct ReviewMoreMenu: View {
    enum Destination { case guide, drafts, details, rules }

    let snapshot: ReviewSnapshot
    let current: ReviewSnapshot
    let comparing: Bool
    let compare: (UInt32?) -> Void
    let open: (Destination) -> Void
    let prioritize: (String?) -> Void
    let sync: () -> Void
    let requestChanges: () -> Void
    let abandon: () -> Void

    var body: some View {
        Menu {
            Section {
                if !snapshot.interdiffs.isEmpty {
                    Menu("Compare", systemImage: "arrow.left.arrow.right") {
                        Button("All changes against base") { compare(nil) }
                            .accessibilityIdentifier("compareAll")
                        ForEach(snapshot.interdiffs.reversed(), id: \.self) { revision in
                            Button(
                                "Since r\(revision)\(snapshot.last_reviewed == revision ? " · last reviewed" : "")"
                            ) { compare(revision) }.accessibilityIdentifier("compareSince-\(revision)")
                        }
                    }.disabled(comparing)
                }
                Button("Review guide", systemImage: "book") { open(.guide) }
                    .accessibilityIdentifier("reviewGuide")
                Button("Comment drafts", systemImage: "text.bubble") { open(.drafts) }
                    .accessibilityIdentifier("reviewDrafts")
            }
            Section {
                Menu("Priority", systemImage: "flag") {
                    ForEach(["p0", "p1", "p2", "p3"], id: \.self) { priority in
                        Button {
                            prioritize(priority)
                        } label: {
                            if current.item.priority == priority {
                                Label(priority, systemImage: "checkmark")
                            } else {
                                Text(priority)
                            }
                        }
                    }
                    Button("None") { prioritize(nil) }
                }
                Button("Item details", systemImage: "info.circle") { open(.details) }
                    .accessibilityIdentifier("reviewDetailsMenu")
                if let link = current.item.link, let url = link.webURL {
                    Link(destination: url) {
                        Label("View \(link.label) on GitHub", systemImage: "arrow.up.right.square")
                    }
                }
                Button("\(current.item.project) rules", systemImage: "list.bullet.rectangle") {
                    open(.rules)
                }
            }
            if !current.item.isClosed {
                Section {
                    if current.item.backend == "github" {
                        Button("Fetch GitHub comments", systemImage: "arrow.clockwise", action: sync)
                    }
                    if current.item.isReviewable {
                        Button("Request changes…", systemImage: "arrow.uturn.backward", action: requestChanges)
                    }
                    Button("Abandon…", systemImage: "trash", role: .destructive, action: abandon)
                        .accessibilityIdentifier("reviewAbandon")
                }
            }
        } label: {
            Label("More", systemImage: "ellipsis")
        }.accessibilityIdentifier("reviewMore")
    }
}
