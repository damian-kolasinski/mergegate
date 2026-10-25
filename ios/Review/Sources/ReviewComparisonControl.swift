import SwiftUI

struct ReviewComparisonControl: View {
    let snapshot: ReviewSnapshot
    let loading: Bool
    let disabled: Bool
    let compare: (UInt32?) -> Void
    @State private var showingHelp = false

    private var scope: String {
        snapshot.since.map { "Since r\($0) → r\(snapshot.item.revision.number)" }
            ?? "All changes → \(snapshot.item.target)"
    }

    var body: some View {
        if snapshot.interdiffs.isEmpty {
            Text(scope)
        } else {
            Menu {
                Button("All changes against base") { compare(nil) }
                    .accessibilityIdentifier("compareAll")
                ForEach(snapshot.interdiffs.reversed(), id: \.self) { revision in
                    Button("Since r\(revision)\(snapshot.last_reviewed == revision ? " · last reviewed" : "")") {
                        compare(revision)
                    }.accessibilityIdentifier("compareSince-\(revision)")
                }
                Button("About source comments") { showingHelp = true }
            } label: {
                HStack(spacing: 6) {
                    Text(scope)
                    if loading { ProgressView() } else { Image(systemName: "chevron.down") }
                }.frame(minHeight: 44, alignment: .leading)
            }.disabled(loading || disabled).tint(Appearance.brand)
                .accessibilityLabel("Compare: \(snapshot.since.map { "Since r\($0)" } ?? "All changes")")
                .accessibilityIdentifier("reviewComparison")
                .alert("Source comments", isPresented: $showingHelp) {
                    Button("Done", role: .cancel) {}
                } message: {
                    Text("Since an earlier revision, Before shows its source. Use All changes to comment on the base version. Unchanged files show their full diff. Dropped changes remain readable without source comments or viewed marks.")
                }
        }
    }
}
