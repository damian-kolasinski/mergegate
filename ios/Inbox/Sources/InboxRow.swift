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
            Button(action: openReview) {
                VStack(alignment: .leading, spacing: 6) {
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 8) {
                            identity
                            Spacer(minLength: 8)
                            size
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            identity
                            size
                        }
                    }
                    Text(item.title).appearanceText(Appearance.rowHeading)
                        .fixedSize(horizontal: false, vertical: true)
                        .multilineTextAlignment(.leading)
                    if let note = item.note, !note.isEmpty {
                        Text(note).appearanceText(Appearance.body).foregroundStyle(Appearance.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                            .multilineTextAlignment(.leading)
                    }
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
                                InboxFlowLayout {
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
        .background(Appearance.surface, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
        // Child buttons keep their actions; padding and passive badges open the review.
        .contentShape(Rectangle())
        .onTapGesture(perform: openReview)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }

    private var decisions: [InboxFlag] { item.flags.filter(\.opensDecisions) }

    private func badges(_ flags: [InboxFlag]) -> some View {
        InboxFlowLayout {
            ForEach(flags) { flag in
                InboxBadge(label: flag.label, tone: flag.tone, solid: flag.solid)
            }
        }
    }

    private var identity: some View {
        HStack(spacing: 6) {
            Text(item.name).appearanceMono(Appearance.caption)
                .foregroundStyle(Appearance.secondary)
            if let priority = item.priority { InboxPriorityBadge(priority: priority) }
            TimelineView(.everyMinute) { context in
                if let age = item.age(now: context.date) {
                    Text(age).appearanceText(Appearance.caption).monospacedDigit()
                        .foregroundStyle(Appearance.secondary)
                }
            }
            Image(systemName: "chevron.right").font(.caption2.weight(.semibold))
                .foregroundStyle(Appearance.secondary).accessibilityHidden(true)
        }
    }

    private var size: some View {
        InboxFlowLayout {
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
        }.appearanceText(Appearance.caption).monospacedDigit()
            .accessibilityElement(children: .combine).accessibilityIdentifier("inboxSize")
    }
}
