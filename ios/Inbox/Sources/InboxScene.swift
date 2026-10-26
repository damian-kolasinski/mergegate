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
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var showingHub = false
    @State private var path: [String] = []

    var body: some View {
        NavigationStack(path: $path) {
            VStack(spacing: 0) {
                List {
                    if let date = model.cachedAt {
                        VStack(alignment: .leading, spacing: 6) {
                            Label("Offline · saved inbox", systemImage: "wifi.slash")
                            Text(
                                "Saved \(date.formatted(date: .abbreviated, time: .shortened)). Open a previously read review to continue."
                            )
                            Button("Reconnect") { Task { await model.refresh() } }.frame(
                                minHeight: 44)
                        }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                            .accessibilityIdentifier("inboxOffline")
                    } else if model.isReconnecting {
                        Label(
                            "Reconnecting… Showing the last inbox.",
                            systemImage: "wifi.exclamationmark"
                        )
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        .accessibilityIdentifier("inboxReconnecting")
                    }
                    if let snapshot = model.snapshot {
                        InboxControls(snapshot: snapshot, model: model).padding(.top, 12).padding(
                            .bottom, 8
                        )
                        .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                    }
                    if let error = model.error, model.cachedAt == nil {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Couldn’t refresh inbox").appearanceText(Appearance.rowHeading)
                            Text(error).foregroundStyle(Appearance.error)
                            Button("Retry") { Task { await model.refresh() } }.frame(minHeight: 44)
                        }
                    }
                    if let snapshot = model.snapshot {
                        ForEach(snapshot.rules ?? [], id: \.project) { pending in
                            Button {
                                rulesProject = pending.project
                            } label: {
                                Label(
                                    "\(pending.project): \(pending.changes) rule changes waiting for you",
                                    systemImage: "line.3.horizontal.decrease"
                                )
                                .frame(minHeight: 44)
                            }.accessibilityIdentifier("pendingRules-\(pending.project)")
                        }
                        if model.groups.isEmpty {
                            ContentUnavailableView(
                                snapshot.isEmpty ? "Inbox is clear" : "No matching items",
                                systemImage: "tray",
                                description: Text(
                                    "Choose another project or state, or pull to refresh."))
                        }
                        ForEach(model.groups) { group in
                            Section {
                                ForEach(group.items) { item in
                                    InboxRow(
                                        item: item, triage: model.triage[item.id],
                                        error: model.triageErrors[item.id]
                                    ) {
                                        Task { await model.loadTriage(item) }
                                    } openReview: {
                                        path.append(item.id)
                                    } openDecisions: {
                                        if let triage = model.triage[item.id] {
                                            decisionItem = DecisionPresentation(
                                                item: item, triage: triage)
                                        }
                                    }.task(id: model.generation) { await model.loadTriage(item) }
                                        .listRowSeparator(.hidden).listRowBackground(
                                            Appearance.page)
                                }
                            } header: {
                                HStack(alignment: .firstTextBaseline) {
                                    Text(group.title.uppercased())
                                    if !dynamicTypeSize.isAccessibilitySize {
                                        Spacer(minLength: 8)
                                        Text(
                                            "\(group.items.count) \(group.items.count == 1 ? "item" : "items")"
                                        )
                                    }
                                }.appearanceText(Appearance.caption).foregroundStyle(
                                    Appearance.secondary
                                )
                                .padding(.horizontal, 6).padding(.vertical, 8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Appearance.page)
                                .accessibilityElement(children: .ignore)
                                .accessibilityLabel("\(group.title), \(group.items.count) \(group.items.count == 1 ? "item" : "items")")
                                .accessibilityAddTraits(.isHeader)
                            }
                        }
                        if let week = snapshot.weekly {
                            InboxWeekView(week: week)
                        }
                    } else if model.isLoading {
                        ProgressView("Loading inbox…").frame(maxWidth: .infinity)
                    }
                }
                .listStyle(.plain).listSectionSpacing(12)
                .environment(\.defaultMinListRowHeight, 0)
                .listRowSpacing(6)
                .scrollContentBackground(.hidden)
                .scrollEdgeEffectHidden(for: .bottom)
                .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
                .background(Appearance.page)
                footer
            }
            .background(Appearance.page)
            .navigationTitle("Your next move")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Hub", systemImage: "arrow.triangle.branch") { showingHub = true }
                        .accessibilityLabel("Hub settings").accessibilityIdentifier("hubSettings")
                }
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
                                .frame(minWidth: 44, minHeight: 44)
                        }
                    }
                }
            }
        }
    }

    private var filterLayout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 0))
            : AnyLayout(HStackLayout(spacing: 12))
    }

    private var footer: some View {
        filterLayout {
            Menu {
                Picker("Project", selection: $model.project) {
                    Text("All projects").tag(String?.none)
                    ForEach(model.snapshot?.projects ?? [], id: \.self) { project in
                        Text(project).tag(Optional(project))
                    }
                }
                ForEach(model.snapshot?.projects ?? [], id: \.self) { project in
                    Button("\(project) rules") { rulesProject = project }
                }
            } label: {
                Label(model.project ?? "All projects", systemImage: "folder")
                    .appearanceText(Appearance.caption)
                    .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                    .frame(minHeight: 44)
            }.accessibilityLabel("Project").accessibilityValue(model.project ?? "All projects")
                .accessibilityIdentifier("projectFilter")
            if !dynamicTypeSize.isAccessibilitySize { Spacer(minLength: 0) }
            Menu {
                Picker("State", selection: $model.filter) {
                    ForEach(InboxFilter.allCases) { filter in Text(filter.rawValue).tag(filter) }
                }
            } label: {
                Label(model.filter.rawValue, systemImage: "chevron.down").appearanceText(Appearance.caption)
                    .accessibilityIdentifier("inboxStateLabel")
                    .frame(minHeight: 44)
            }.accessibilityLabel("State").accessibilityValue(model.filter.rawValue)
                .accessibilityIdentifier("stateFilter")
        }.padding(.horizontal, 16).padding(.vertical, 6)
            .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 28))
            .padding(.horizontal, 16).padding(.bottom, 8).tint(Appearance.brand)
    }

    private var hubSettings: some View {
        NavigationStack {
            List {
                Section("Hub") { Text(hub.url.absoluteString).textSelection(.enabled) }
                Section("Device") { Text(hub.deviceName) }
                Button("Forget hub", role: .destructive) { disconnect() }
                    .frame(minHeight: 44).accessibilityIdentifier("forgetHub")
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
