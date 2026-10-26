import SwiftUI

// Why the hub ranked a file where it is, and what kind of change it is, behind the file pill's
// ⓘ instead of above every diff.
struct ReviewFileReasons: View {
    let file: ReviewFile
    let close: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(file.path).font(.system(size: 14, design: .monospaced).weight(.semibold))
                    .textSelection(.enabled)
                Text(metadata).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                if let reason = file.reason { part("Guide", reason) }
                if let reason = file.judgeReason {
                    part(file.importance.map { "Judge · \($0)/100" } ?? "Judge", reason)
                }
                if let why = file.why {
                    part(
                        file.rule.map {
                            "Rule \($0)\(file.promoted == true ? " · Promoted by judge" : "")"
                        } ?? "Kept in review", why)
                }
                if file.importance == nil, let hint = file.hint { part("Heuristics", hint) }
                if let missing = file.missingPack {
                    part("Highlighting unavailable", "Install language pack \(missing) on the hub.")
                }
                if file.rankingReason == nil && file.missingPack == nil {
                    Text("The hub gave no reason for this file’s place.")
                        .appearanceText(Appearance.body).foregroundStyle(Appearance.secondary)
                }
            }.padding(20).frame(maxWidth: .infinity, alignment: .leading)
        }
        .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
        .background(Appearance.page)
        .safeAreaInset(edge: .top, spacing: 0) {
            ReviewSheetHeader(title: "Why this file", closeIdentifier: "closeFileReasons", close: close)
        }
        .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
    }

    private var metadata: String {
        var parts = [file.change, file.tier == .trusted ? "Trusted" : "Review"]
        if let additions = file.additions, let deletions = file.deletions {
            parts.append("+\(additions) −\(deletions)")
        }
        if let old = file.oldMode, let new = file.newMode, old != new {
            parts.append("\(old) → \(new)")
        }
        if let comparison = file.interdiff { parts.append(comparison.label) }
        return parts.joined(separator: " · ")
    }

    private func part(_ label: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).appearanceText(Appearance.label)
            Text(text).textSelection(.enabled)
        }
    }
}
