import SwiftUI

struct InboxRow: View {
    let item: InboxItem
    let triage: InboxTriage?
    let error: String?
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(item.name)
                if let priority = item.priority { Text(priority.uppercased()).bold() }
            }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            Text(item.title).appearanceText(Appearance.rowHeading)
            Text("\(item.status) · Revision \(item.revision)\(item.target.map { " → \($0)" } ?? "")")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            if let note = item.note { Text(note).appearanceText(Appearance.body) }
            if item.needsDecision {
                Text(decisionCounts).appearanceText(Appearance.label).foregroundStyle(Appearance.brand)
            }
            if let triage {
                ForEach(triage.decisions) { decision in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(decision.kind).appearanceText(Appearance.label).foregroundStyle(Appearance.brand)
                        if let path = decision.path {
                            Text(path).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                                .accessibilityLabel("File \(path)")
                        }
                        Text(decision.comment).appearanceText(Appearance.body)
                        if let response = decision.response {
                            Text(response).appearanceText(Appearance.body).bold()
                        }
                    }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
                        .background(Appearance.surface, in: RoundedRectangle(cornerRadius: 10))
                }
            }
            if !item.isClosed {
                if let triage {
                    Text("\(triage.remainingFiles) of \(triage.reviewFiles) review files remaining")
                        .appearanceText(Appearance.caption)
                } else if error == nil {
                    Text("\(item.reviewFiles) review files · Loading remaining work…")
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                }
            }
            HStack {
                Text("+\(item.additions)").foregroundStyle(Appearance.added)
                Text("−\(item.deletions)").foregroundStyle(Appearance.removed)
            }.appearanceText(Appearance.caption)
            if let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                Button("Retry item details", action: retry).frame(minHeight: 44)
            }
        }
        .padding(.vertical, 10)
        .listRowBackground(Appearance.page)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("inboxItem-\(item.id)")
    }

    private var decisionCounts: String {
        let counts: [(UInt32, String)] = [(item.questions ?? 0, "question"), (item.rejections ?? 0, "disagreement"), (item.votes ?? 0, "vote")]
        return counts
            .filter { $0.0 > 0 }
            .map { "\($0.0) \($0.1)\($0.0 == 1 ? "" : "s")" }.joined(separator: " · ")
    }
}
