import SwiftUI

struct RulesScene: View {
    @State var model: RulesViewModel
    let openItem: (String) -> Void
    @Environment(\.scenePhase) private var phase
    @State private var confirmingOverwrite = false

    var body: some View {
        List {
            if let error = model.error {
                Section {
                    Text(error).foregroundStyle(Appearance.error)
                    Button("Retry") { Task { await model.load() } }.frame(minHeight: 44)
                }
            }
            if let snapshot = model.snapshot {
                if let error = snapshot.error {
                    Section("Rules file error") {
                        Text(
                            "\(snapshot.location) can’t be used. The previously applied rules remain active."
                        )
                        Text(error).foregroundStyle(Appearance.error)
                    }
                }
                ForEach(snapshot.pending, id: \.rule.id) { proposal in
                    Section("Proposed \(proposal.rule.id)") {
                        rule(proposal.rule)
                        if let preview = proposal.preview { previewRows(preview) }
                        decisions(
                            approve: .proposal(proposal.rule.id, approve: true),
                            reject: .proposal(proposal.rule.id, approve: false))
                    }
                }
                if let edit = snapshot.edit {
                    Section("Edit awaiting approval") {
                        Text("\(snapshot.location) changed to rules that would hide more files.")
                        ForEach(edit.rules) { rule($0) }
                        previewRows(edit.preview)
                        Button("Approve") { Task { await model.decide(.edit(approve: true)) } }
                        Button("Reject edit…", role: .destructive) { confirmingOverwrite = true }
                    }
                }
                Section("Applied rules · first match wins") {
                    Text(snapshot.location).textSelection(.enabled).accessibilityLabel(
                        "Rules file: \(snapshot.location)")
                    if snapshot.rules.isEmpty { Text("No rules. Every file is reviewed.") }
                    ForEach(snapshot.rules) { rule($0) }
                }
            } else if model.error == nil {
                ProgressView("Loading rules…")
            }
        }
        .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
        .scrollContentBackground(.hidden).background(Appearance.page)
        .navigationTitle("\(model.service.project) rules").navigationBarTitleDisplayMode(.inline)
        .disabled(model.busy).refreshable { await model.load() }
        .task(id: phase) { if phase == .active { await model.follow() } }
        .confirmationDialog(
            "Overwrite the rules file with the applied rules?",
            isPresented: $confirmingOverwrite, titleVisibility: .visible
        ) {
            Button("Overwrite", role: .destructive) {
                Task { await model.decide(.edit(approve: false)) }
            }
        } message: {
            Text(
                "Rejecting this edit writes the applied rules back to \(model.snapshot?.location ?? "the rules file")."
            )
        }
        .accessibilityIdentifier("projectRules")
    }

    private func rule(_ value: RulesSnapshot.Rule) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(value.id) · \(value.tier)").appearanceText(Appearance.rowHeading)
            Text(value.summary).font(.system(.body, design: .monospaced)).accessibilityLabel(
                "Rule matches: \(value.summary)")
            Text(value.reason)
        }.padding(.vertical, 6)
    }
    @ViewBuilder private func previewRows(_ preview: RulesSnapshot.Preview) -> some View {
        Text(preview.summary).appearanceText(Appearance.caption)
        ForEach(Array(preview.hidden.enumerated()), id: \.offset) { _, file in
            Button {
                openItem(file.item)
            } label: {
                VStack(alignment: .leading, spacing: 4) {
                    Text("\(file.name) · \(file.path)")
                    Text("\(file.title) · by \(file.rule)").appearanceText(Appearance.caption)
                }.frame(minHeight: 44)
            }
        }
    }
    @ViewBuilder private func decisions(approve: RulesDecision, reject: RulesDecision) -> some View
    {
        Button("Approve") { Task { await model.decide(approve) } }.frame(minHeight: 44)
        Button("Reject", role: .destructive) { Task { await model.decide(reject) } }.frame(
            minHeight: 44)
    }
}
