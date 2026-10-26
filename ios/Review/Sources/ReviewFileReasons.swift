import SwiftUI

struct ReviewFileReasons: View {
    let file: ReviewFile
    var body: some View {
        DisclosureGroup {
            VStack(alignment: .leading, spacing: 8) {
                if let reason = file.reason { part("Guide", reason) }
                if let reason = file.judgeReason {
                    part(file.importance.map { "Judge · \($0)/100" } ?? "Judge", reason)
                }
                if let why = file.why {
                    part(
                        file.rule.map {
                            "Rule \($0)\(file.promoted == true ? " · Promoted by judge" : "")"
                        }
                            ?? "Kept in review", why)
                }
                if file.importance == nil, let hint = file.hint { part("Heuristics", hint) }
                if let missing = file.missingPack {
                    part("Highlighting unavailable", "Install language pack \(missing) on the hub.")
                }
            }
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                Text("Why this file").appearanceText(Appearance.label)
                if let reason = file.rankingReason {
                    Text(reason).appearanceText(Appearance.caption).lineLimit(2)
                        .foregroundStyle(Appearance.secondary)
                }
            }.padding(.vertical, 6)
        }.appearanceText(Appearance.caption).frame(minHeight: 44)
            .accessibilityIdentifier("reviewFileReasons")
    }
    private func part(_ label: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).bold()
            Text(text).textSelection(.enabled)
        }
    }
}
