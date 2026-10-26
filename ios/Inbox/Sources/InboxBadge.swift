import SwiftUI

struct InboxBadge: View {
    let label: String
    let tone: InboxFlag.Tone
    var solid = false

    var body: some View {
        Text(label).appearanceText(Appearance.caption).fontWeight(solid ? .semibold : .medium)
            .lineLimit(1)
            .padding(.horizontal, tone == .plain ? 0 : 7).padding(.vertical, 2)
            .foregroundStyle(foreground)
            .background(background, in: Capsule())
    }

    private var foreground: Color {
        switch (tone, solid) {
        case (.plain, _): Appearance.secondary
        case (.brand, true), (.warning, true): Appearance.onFill
        case (.brand, false): Appearance.onTintBrand
        case (.warning, false): Appearance.onTintWarning
        case (.neutral, _): Appearance.onTintNeutral
        }
    }

    private var background: Color {
        switch (tone, solid) {
        case (.plain, _): .clear
        case (.brand, true): Appearance.brand
        case (.brand, false): Appearance.tintBrand
        case (.warning, true): Appearance.warning
        case (.warning, false): Appearance.tintWarning
        case (.neutral, _): Appearance.tintNeutral
        }
    }
}

// The web's priority badge: p0 urgent, p1 warning, p2 brand, p3 neutral.
struct InboxPriorityBadge: View {
    let priority: String

    var body: some View {
        switch priority {
        case "p0":
            Text(priority).appearanceText(Appearance.caption).fontWeight(.semibold)
                .padding(.horizontal, 6).padding(.vertical, 1)
                .foregroundStyle(Appearance.onFill)
                .background(Appearance.error, in: Capsule())
        case "p1": InboxBadge(label: priority, tone: .warning, solid: true)
        case "p2": InboxBadge(label: priority, tone: .brand, solid: true)
        default: InboxBadge(label: priority, tone: .neutral)
        }
    }
}

// Badges wrap onto further lines instead of truncating, like the web's flags column.
struct InboxFlowLayout: Layout {
    var spacing: CGFloat = 4

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
                let size = subviews[index].sizeThatFits(.unspecified)
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
            let size = subviews[index].sizeThatFits(.unspecified)
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
