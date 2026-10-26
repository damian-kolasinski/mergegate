import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

struct InboxScene<Review: View, Feedback: View, Rules: View>: View {
    let hub: ConnectedHub
    @State var model: InboxViewModel
    let disconnect: () -> Void
    let feedback: (CommentDraftScope, @escaping () -> Void) -> Feedback
    let rules: (String, @escaping (String) -> Void) -> Rules
    let review: (String, @escaping (String) -> Void) -> Review
    @State private var rulesProject: String?
    @State private var decisionItem: DecisionPresentation?
    @Environment(\.scenePhase) private var scenePhase
    @State private var showingHub = false
    @State private var showingClosed = false
    @State private var path: [String] = []

    var body: some View {
        NavigationStack(path: $path) {
            List {
                status
                if let snapshot = model.snapshot {
                    ForEach(snapshot.rules ?? [], id: \.project) { pending in
                        Button {
                            rulesProject = pending.project
                        } label: {
                            InboxBadge(
                                label:
                                    "\(pending.project) · \(pending.changes) rule \(pending.changes == 1 ? "change" : "changes") to approve",
                                tone: .warning)
                        }.buttonStyle(.borderless).listRowSeparator(.hidden)
                            .listRowBackground(Appearance.page)
                            .accessibilityIdentifier("pendingRules-\(pending.project)")
                    }
                    ForEach(model.groups) { group in
                        // A regular row scrolls with its cards instead of pinning over them.
                        header(group.section.rawValue, count: group.items.count)
                            .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                            .listRowInsets(EdgeInsets(top: 0, leading: 16, bottom: 0, trailing: 16))
                        if group.items.isEmpty {
                            Text(emptyNeedsYou).appearanceText(Appearance.body)
                                .foregroundStyle(Appearance.secondary)
                                .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                                .accessibilityIdentifier("inboxEmpty")
                        }
                        ForEach(group.items) { item in row(item) }
                    }
                    if !model.closed.isEmpty {
                        DisclosureGroup(isExpanded: $showingClosed) {
                            ForEach(model.closed) { item in row(item) }
                        } label: {
                            // On the label: a group's identifier would replace its rows'.
                            Text("Closed today (\(model.closed.count))")
                                .appearanceText(Appearance.label)
                                .foregroundStyle(Appearance.secondary).frame(minHeight: 44)
                                .accessibilityIdentifier("inboxClosedToday")
                        }.listRowSeparator(.hidden).listRowBackground(Appearance.page)
                    }
                    if let week = snapshot.weekly {
                        InboxWeekView(week: week).listRowSeparator(.hidden)
                            .listRowBackground(Appearance.page)
                    }
                } else if model.isLoading {
                    ProgressView("Loading inbox…").frame(maxWidth: .infinity)
                        .listRowBackground(Appearance.page)
                }
            }
            .listStyle(.plain)
            .environment(\.defaultMinListRowHeight, 0)
            .scrollContentBackground(.hidden)
            .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
            .background(Appearance.page)
            .navigationTitle("Inbox")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Hub", systemImage: "arrow.triangle.branch") { showingHub = true }
                        .accessibilityLabel("Hub settings").accessibilityIdentifier("hubSettings")
                }
                ToolbarItem(placement: .topBarTrailing) { projectMenu }
            }
            .refreshable { await model.refresh() }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                await model.follow()
            }
            .onChange(of: path) { previous, current in
                if !previous.isEmpty && current.isEmpty { Task { await model.refresh() } }
            }
            .navigationDestination(for: String.self) { item in review(item) { path.append($0) } }
            .sheet(item: $decisionItem, onDismiss: { Task { await model.refresh() } }) {
                selection in
                InboxDecisionScene(
                    item: selection.item, triage: selection.triage,
                    service: .init(transport: URLSessionTransport(), hub: hub.url)
                ) { close in
                    feedback(
                        .init(
                            hub: hub.url, item: selection.item.id, name: selection.item.name,
                            revision: selection.item.revision), close)
                }
            }
            .sheet(isPresented: $showingHub) { hubSettings }
            .sheet(
                isPresented: Binding(
                    get: { rulesProject != nil }, set: { if !$0 { rulesProject = nil } })
            ) {
                NavigationStack {
                    if let project = rulesProject {
                        rules(project) { item in
                            rulesProject = nil
                            path.append(item)
                        }
                        .toolbar {
                            Button("Done") { rulesProject = nil }.appearanceText(Appearance.label)
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder private var status: some View {
        if let date = model.cachedAt {
            VStack(alignment: .leading, spacing: 6) {
                Label("Offline · saved inbox", systemImage: "wifi.slash")
                Text(
                    "Saved \(date.formatted(date: .abbreviated, time: .shortened)). Open a previously read review to continue."
                )
                Button("Reconnect") { Task { await model.refresh() } }.buttonStyle(.borderless)
            }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                .accessibilityIdentifier("inboxOffline")
        } else if model.isReconnecting {
            Label("Reconnecting… Showing the last inbox.", systemImage: "wifi.exclamationmark")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                .accessibilityIdentifier("inboxReconnecting")
        }
        if let error = model.error, model.cachedAt == nil {
            VStack(alignment: .leading, spacing: 8) {
                Text("Couldn’t refresh inbox").appearanceText(Appearance.rowHeading)
                Text(error).foregroundStyle(Appearance.error)
                Button("Retry") { Task { await model.refresh() } }.buttonStyle(.borderless)
            }.listRowSeparator(.hidden).listRowBackground(Appearance.page)
        }
    }

    private func row(_ item: InboxItem) -> some View {
        InboxRow(
            item: item, error: model.triageErrors[item.id], offline: model.cachedAt != nil
        ) {
            Task { await model.loadTriage(item) }
        } openReview: {
            path.append(item.id)
        } openDecisions: {
            Task {
                await model.loadTriage(item)
                if let triage = model.triage[item.id] {
                    decisionItem = DecisionPresentation(item: item, triage: triage)
                }
            }
        }
        .task(id: model.generation) { await model.loadTriage(item) }
        .listRowSeparator(.hidden).listRowBackground(Appearance.page)
        .listRowInsets(EdgeInsets(top: 4, leading: 16, bottom: 4, trailing: 16))
    }

    private func header(_ title: String, count: Int) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title.uppercased()).tracking(0.6)
            Spacer(minLength: 8)
            Text("\(count)").monospacedDigit()
        }.appearanceText(Appearance.caption).fontWeight(.semibold)
            .foregroundStyle(Appearance.secondary)
            .padding(.horizontal, 4).padding(.top, 10).padding(.bottom, 2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Appearance.page)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("\(title), \(count) \(count == 1 ? "item" : "items")")
            .accessibilityAddTraits(.isHeader)
    }

    // The web's wording for an empty Needs you.
    private var emptyNeedsYou: String {
        let working = model.groups.filter { $0.section != .needsYou }.map(\.items.count)
            .reduce(0, +)
        return working == 0
            ? "Nothing to review. Items show up here when an agent runs mg submit."
            : "All caught up — agents are working on \(working) \(working == 1 ? "item" : "items")."
    }

    private var projectMenu: some View {
        Menu {
            Picker("Project", selection: $model.project) {
                Text("All projects").tag(String?.none)
                ForEach(model.snapshot?.projects ?? [], id: \.self) { project in
                    Text(project).tag(Optional(project))
                }
            }
            Section("Rules") {
                ForEach(model.snapshot?.projects ?? [], id: \.self) { project in
                    Button("\(project) rules") { rulesProject = project }
                }
            }
        } label: {
            Text(model.project ?? "All projects").appearanceText(Appearance.label).lineLimit(1)
        }.accessibilityLabel("Project").accessibilityValue(model.project ?? "All projects")
            .accessibilityIdentifier("projectFilter")
    }

    private var hubSettings: some View {
        NavigationStack {
            List {
                Section("Hub") { Text(hub.url.absoluteString).textSelection(.enabled) }
                Section("Device") { Text(hub.deviceName) }
                Button("Forget hub", role: .destructive) { disconnect() }
                    .accessibilityIdentifier("forgetHub")
            }.appearanceText(Appearance.body)
                .navigationTitle("Connected hub")
                .toolbar { Button("Done") { showingHub = false } }
        }
    }
}

// Keep the displayed revision and text together while the inbox refreshes.
private struct DecisionPresentation: Identifiable {
    let item: InboxItem
    let triage: InboxTriage
    var id: String { item.id }
}
