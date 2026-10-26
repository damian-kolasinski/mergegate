import SwiftUI

// The web's inbox row: identity, size and age, then the title, then any flags.
struct InboxRow: View {
    let item: InboxItem
    let error: String?
    /// A saved inbox can't load decisions, so its flags don't offer to open them.
    let offline: Bool
    let retry: () -> Void
    let openReview: () -> Void
    let openDecisions: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Decision flags are buttons of their own, so only these two lines open the review.
            Button(action: openReview) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Text(item.name).font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(Appearance.secondary)
                        if let priority = item.priority { InboxPriorityBadge(priority: priority) }
                        Spacer(minLength: 8)
                        size
                        TimelineView(.everyMinute) { context in
                            if let age = item.age(now: context.date) {
                                Text(age).appearanceText(Appearance.caption).monospacedDigit()
                                    .foregroundStyle(Appearance.secondary)
                            }
                        }
                        Image(systemName: "chevron.right").font(.caption2.weight(.semibold))
                            .foregroundStyle(Appearance.secondary).accessibilityHidden(true)
                    }
                    Text(title).appearanceText(Appearance.rowHeading).lineLimit(2)
                        .multilineTextAlignment(.leading)
                }.frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("openReview-\(item.id)")
                .accessibilityHint("Open review")
            if !item.flags.isEmpty {
                InboxFlowLayout {
                    // One target for every pending decision: they all open the same sheet.
                    if !decisions.isEmpty {
                        if offline {
                            badges(decisions)
                        } else {
                            Button(action: openDecisions) {
                                HStack(spacing: 4) {
                                    badges(decisions)
                                    Image(systemName: "chevron.right")
                                        .font(.caption2.weight(.semibold))
                                        .foregroundStyle(Appearance.brand)
                                }.frame(minHeight: 44).contentShape(Rectangle())
                            }.buttonStyle(.borderless)
                                .accessibilityIdentifier("decisionDetails-\(item.id)")
                                .accessibilityHint("Read and answer")
                        }
                    }
                    ForEach(item.flags.filter { !$0.opensDecisions }) { flag in
                        InboxBadge(label: flag.label, tone: flag.tone, solid: flag.solid)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            if let error {
                HStack {
                    Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                    Button("Retry", action: retry).buttonStyle(.borderless)
                }
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Appearance.surface, in: RoundedRectangle(cornerRadius: 14))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }

    private var decisions: [InboxFlag] { item.flags.filter(\.opensDecisions) }

    private func badges(_ flags: [InboxFlag]) -> some View {
        HStack(spacing: 4) {
            ForEach(flags) { flag in
                InboxBadge(label: flag.label, tone: flag.tone, solid: flag.solid)
            }
        }
    }

    private var title: AttributedString {
        var title = AttributedString(item.title)
        if let note = item.note {
            var suffix = AttributedString(" · \(note)")
            suffix.foregroundColor = Appearance.secondary
            title += suffix
        }
        return title
    }

    private var size: some View {
        HStack(spacing: 6) {
            if item.additions > 0 {
                Text("+\(item.additions)").foregroundStyle(Appearance.added)
                    .accessibilityLabel("\(item.additions) added lines")
            }
            if item.deletions > 0 {
                Text("−\(item.deletions)").foregroundStyle(Appearance.removed)
                    .accessibilityLabel("\(item.deletions) removed lines")
            }
            if let files = item.filesLabel {
                Text(files).foregroundStyle(Appearance.secondary)
            }
        }.appearanceText(Appearance.caption).monospacedDigit().fixedSize()
            .accessibilityElement(children: .combine).accessibilityIdentifier("inboxSize")
    }
}
