import SwiftUI

struct ReviewDetailsScene<Rules: View>: View {
    @State var model: ReviewActionsModel
    let displayedRevision: UInt32
    let close: () -> Void
    let openItem: (String) -> Void
    let rules: (String, @escaping (String) -> Void) -> Rules
    @State private var confirmingAbandon = false
    @State private var confirmingCancel = false
    @Environment(\.scenePhase) private var phase

    var body: some View {
        NavigationStack {
            List {
                if let value = model.snapshot {
                    Section(value.item.name) {
                        Text(value.item.title).appearanceText(Appearance.rowHeading)
                        Text(
                            "\(value.item.state.replacingOccurrences(of: "_", with: " ")) · r\(value.item.revision.number) → \(value.item.target)"
                        )
                        if let branch = value.item.branch {
                            Text("\(branch) → \(value.item.target)").appearanceText(
                                Appearance.caption
                            ).textSelection(.enabled)
                        }
                        if value.item.revision.number != displayedRevision {
                            Text(
                                "The reader is pinned to r\(displayedRevision). These actions apply to the current item."
                            )
                        }
                        if let link = value.item.link, let url = link.webURL {
                            Link("View \(link.label) on GitHub", destination: url).frame(
                                minHeight: 44)
                        }
                    }
                    if let blocked = value.blocked {
                        Section(blocked.label) {
                            Text(blocked.detail).textSelection(.enabled)
                            if blocked.kind == "held" {
                                Button("Retry merge") { Task { await model.send(.retryMerge) } }
                                    .frame(
                                        minHeight: 44)
                            }
                            if ["held", "waiting", "behind"].contains(blocked.kind) {
                                Button("Cancel merge…", role: .destructive) {
                                    confirmingCancel = true
                                }.frame(
                                    minHeight: 44)
                            }
                        }
                    }
                    if value.agent == "away" {
                        Section("No agent") {
                            Text(
                                "What you send waits until a session picks this item up with mg resume \(value.item.name)."
                            )
                        }
                    }
                    if let bots = value.bots, !bots.isEmpty {
                        Section("GitHub reviews") {
                            ForEach(bots, id: \.login) { bot in
                                Label(
                                    "\(bot.name) \(bot.state)",
                                    systemImage: bot.state == "approved" ? "hand.thumbsup" : "eye")
                            }
                        }
                    }
                    if let stack = value.stack, !stack.isEmpty {
                        Section("Stack · bottom first") {
                            ForEach(stack) { entry in
                                if entry.id == value.item.id {
                                    Text(
                                        "\(entry.name) · \(entry.title) · Current item\(value.behind == true ? " · Behind parent" : "")"
                                    )
                                } else {
                                    Button {
                                        openItem(entry.id)
                                    } label: {
                                        VStack(alignment: .leading, spacing: 4) {
                                            Text("\(entry.name) · \(entry.title)")
                                            Text(entry.state).appearanceText(Appearance.caption)
                                        }.frame(minHeight: 44)
                                    }
                                }
                            }
                        }
                    }
                    Section("Priority") {
                        Picker("Priority", selection: $model.priority) {
                            Text("None").tag(String?.none)
                            ForEach(["p0", "p1", "p2", "p3"], id: \.self) {
                                Text($0).tag(Optional($0))
                            }
                        }
                        .pickerStyle(.menu).accessibilityIdentifier("itemPriorityPicker")
                        TextField("Note (optional)", text: $model.note, axis: .vertical).lineLimit(3...6)
                        Button {
                            Task { await model.send(.priority) }
                        } label: {
                            Text("Save priority").appearanceText(Appearance.label).frame(
                                minHeight: 44)
                        }.buttonStyle(.plain).foregroundStyle(Appearance.brand)
                            .accessibilityIdentifier("savePriority")
                    }
                    Section("Project") {
                        NavigationLink("Project rules") { rules(value.item.project, openItem) }
                    }
                    if !value.item.isClosed {
                        Section("More actions") {
                            if value.item.backend == "github" {
                                Button("Fetch GitHub comments") { Task { await model.send(.sync) } }
                                    .frame(
                                        minHeight: 44)
                            }
                            TextField(
                                "Reason for abandoning (optional)", text: $model.reason,
                                axis: .vertical).lineLimit(3...6)
                            Button("Abandon item…", role: .destructive) { confirmingAbandon = true }
                                .frame(
                                    minHeight: 44)
                        }
                    }
                } else if model.error == nil {
                    ProgressView("Loading item…")
                }

            }
            .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
            .scrollContentBackground(.hidden).background(Appearance.page)
            .safeAreaInset(edge: .bottom) {
                if model.result != nil || model.error != nil {
                    VStack(alignment: .leading, spacing: 8) {
                        if let result = model.result {
                            Text(result).accessibilityIdentifier("itemActionResult")
                        }
                        if let error = model.error {
                            Text(error).foregroundStyle(Appearance.error)
                            if model.githubUnreachable {
                                Text(
                                    "If GitHub no longer has the repository, abandon the item in mergegate only: its pull request and branch stay as they are there."
                                )
                                Button("Abandon in mergegate only", role: .destructive) {
                                    Task { await model.send(.abandonLeavingGithub) }
                                }.frame(minHeight: 44)
                            }
                            Button("Refresh item") { Task { await model.load() } }.frame(
                                minHeight: 44)
                        }
                    }.appearanceText(Appearance.body).padding(16).frame(
                        maxWidth: .infinity, alignment: .leading
                    )
                    .background(Appearance.surface)
                }
            }
            .disabled(model.busy).refreshable { await model.load() }
            .navigationTitle("Item details").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                Button(action: close) {
                    Image(systemName: "xmark").foregroundStyle(Appearance.brand)
                        .frame(minWidth: 44, minHeight: 44)
                }.accessibilityLabel("Done").disabled(model.busy)
            }
            .confirmationDialog(
                "Abandon \(model.snapshot?.item.name ?? "item") without merging?",
                isPresented: $confirmingAbandon, titleVisibility: .visible
            ) {
                Button("Abandon item", role: .destructive) { Task { await model.send(.abandon) } }
            } message: {
                Text(
                    model.snapshot?.item.backend == "github"
                        ? "Closes its GitHub pull request, deletes its branch and drops its diffs. The agent receives your reason."
                        : "Drops the item's diffs. The agent receives your reason.")
            }
            .confirmationDialog(
                "Cancel the approved merge?", isPresented: $confirmingCancel,
                titleVisibility: .visible
            ) {
                Button("Cancel merge", role: .destructive) {
                    Task { await model.send(.cancelMerge) }
                }
            }
        }
        .interactiveDismissDisabled(model.busy)
        .task(id: phase) { if phase == .active { await model.load() } }
        .accessibilityIdentifier("reviewDetails")
    }
}
