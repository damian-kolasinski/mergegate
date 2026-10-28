import SwiftUI

// The web item header (ui/item/header.tsx) for a phone: identity and badges, the title, branch
// and target, then size, review progress and whether it merges, so none of it needs a tap.
struct ReviewMasthead<Comparison: View>: View {
    /// The displayed comparison, which sets the review progress.
    let snapshot: ReviewSnapshot
    /// The newest state of the item, which sets its badges and merge status.
    let current: ReviewSnapshot
    let openDetails: () -> Void
    @ViewBuilder let comparison: () -> Comparison

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ReviewChips {
                Text(item.name).appearanceMono(Appearance.caption)
                    .foregroundStyle(Appearance.secondary)
                    .accessibilityLabel("\(item.name) · r\(item.revision.number) → \(item.target)")
                    .accessibilityIdentifier("reviewIdentity")
                ReviewBadge(label: "r\(item.revision.number)", tone: .brand)
                ReviewBadge(label: state, tone: .neutral).accessibilityIdentifier("reviewState")
                if let priority = current.item.priority {
                    ReviewBadge(label: priority, tone: priority == "p0" || priority == "p1" ? .warning : .neutral)
                }
            }
            Text(item.title).appearanceText(Appearance.heading).accessibilityAddTraits(.isHeader)
            if let branch = item.branch {
                Text("\(branch) → \(item.target)").appearanceMono(Appearance.caption)
                    .foregroundStyle(Appearance.secondary).fixedSize(horizontal: false, vertical: true)
            }
            stats
            ReviewChips {
                merge
                ForEach(current.bots ?? [], id: \.login) { bot in
                    ReviewBadge(
                        label: "\(bot.state == "approved" ? "👍" : "👀") \(bot.name)", tone: .neutral)
                }
                if current.agent == "away" {
                    ReviewBadge(label: "No agent", tone: .neutral)
                        .accessibilityHint("What you send waits until a session resumes this item")
                }
            }
            comparison()
        }
    }

    private var item: ReviewSnapshot.Item { snapshot.item }

    private var state: String {
        switch current.item.state {
        case "needs_review": "Needs review"
        case "changes_requested": "Changes requested"
        case "merging": "Merging"
        case "merged": "Merged into \(item.target)"
        case "abandoned": "Abandoned"
        default: current.item.state.replacingOccurrences(of: "_", with: " ")
        }
    }

    private var stats: some View {
        let revision = item.revision
        let additions = revision.additions ?? snapshot.files.reduce(0) { $0 + ($1.additions ?? 0) }
        let deletions = revision.deletions ?? snapshot.files.reduce(0) { $0 + ($1.deletions ?? 0) }
        let files = revision.files.map(Int.init) ?? snapshot.files.count
        return ReviewChips {
            Text("+\(additions)").foregroundStyle(Appearance.added)
            Text("−\(deletions)").foregroundStyle(Appearance.removed)
            Text("· \(files) \(files == 1 ? "file" : "files") ·").foregroundStyle(Appearance.secondary)
            Text("\(snapshot.reviewCount - snapshot.remaining) of \(snapshot.reviewCount) reviewed")
                .fontWeight(.semibold)
            if snapshot.trustedCount > 0 {
                Text("· \(snapshot.trustedCount) trusted").foregroundStyle(Appearance.secondary)
            }
        }.appearanceText(Appearance.caption).monospacedDigit()
            .accessibilityElement(children: .combine).accessibilityIdentifier("reviewStats")
    }

    // A merge that didn't land outranks the pre-approval check; closed items show neither.
    @ViewBuilder private var merge: some View {
        if current.item.isClosed {
            EmptyView()
        } else if let blocked = current.blocked {
            Button(action: openDetails) {
                ReviewBadge(label: "⚠︎ \(blocked.label)", tone: .warning, solid: true)
            }.buttonStyle(.plain).accessibilityIdentifier("reviewBlocked")
        } else {
            switch current.mergeable {
            case .clean:
                ReviewBadge(label: "● Merges cleanly", tone: .success)
                    .accessibilityIdentifier("reviewMergeable")
            case .conflict(let paths):
                ReviewBadge(label: "● Conflicts with \(item.target)", tone: .warning, solid: true)
                    .accessibilityHint(paths.joined(separator: ", "))
                    .accessibilityIdentifier("reviewMergeable")
            case .unknown, nil: EmptyView()
            }
        }
    }
}

struct ReviewBadge: View {
    enum Tone { case brand, warning, neutral, success }
    let label: String
    let tone: Tone
    var solid = false

    var body: some View {
        Text(label).appearanceText(Appearance.caption).fontWeight(.semibold).fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .foregroundStyle(foreground).background(background, in: Capsule())
    }

    private var foreground: Color {
        if solid { return Appearance.onFill }
        return switch tone {
        case .brand: Appearance.onTintBrand
        case .warning: Appearance.onTintWarning
        case .neutral: Appearance.onTintNeutral
        case .success: Appearance.onTintSuccess
        }
    }

    private var background: Color {
        switch (tone, solid) {
        case (.warning, true): Appearance.warning
        case (_, true): Appearance.brand
        case (.brand, false): Appearance.tintBrand
        case (.warning, false): Appearance.tintWarning
        case (.neutral, false): Appearance.tintNeutral
        case (.success, false): Appearance.tintSuccess
        }
    }
}

// Badges wrap onto further lines instead of truncating.
struct ReviewChips: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(subviews, width: proposal.width ?? .infinity)
        return CGSize(
            width: rows.map(\.width).max() ?? 0,
            height: rows.map(\.height).reduce(0, +) + spacing * CGFloat(max(0, rows.count - 1)))
    }

    func placeSubviews(
        in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()
    ) {
        var y = bounds.minY
        for row in arrange(subviews, width: bounds.width) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(ProposedViewSize(width: bounds.width, height: nil))
                subviews[index].place(
                    at: CGPoint(x: x, y: y + (row.height - size.height) / 2),
                    proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + spacing
        }
    }

    private struct Row {
        var indices: [Int] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> [Row] {
        var rows: [Row] = []
        var row = Row()
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(ProposedViewSize(width: width, height: nil))
            if size == .zero { continue }
            if !row.indices.isEmpty && row.width + spacing + size.width > width {
                rows.append(row)
                row = Row()
            }
            row.width += (row.indices.isEmpty ? 0 : spacing) + size.width
            row.height = max(row.height, size.height)
            row.indices.append(index)
        }
        if !row.indices.isEmpty { rows.append(row) }
        return rows
    }
}
