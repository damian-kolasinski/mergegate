import SwiftUI

struct ReviewFileReasons: View {
    let file: ReviewFile
    var body: some View {
        if let reason = file.rankingReason { Text(reason).appearanceText(Appearance.body) }
        DisclosureGroup("Why this file") {
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
        }.appearanceText(Appearance.caption).frame(minHeight: 44)
    }
    private func part(_ label: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).bold()
            Text(text).textSelection(.enabled)
        }
    }
}
