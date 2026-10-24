import HubConnectionInterface
import SwiftUI

struct InboxScene: View {
    let hub: ConnectedHub
    @State var model: InboxViewModel
    let disconnect: () -> Void
    @Environment(\.scenePhase) private var scenePhase
    @State private var showingHub = false

    var body: some View {
        NavigationStack {
            List {
                if let snapshot = model.snapshot {
                    InboxControls(snapshot: snapshot, model: model).padding(.top, 12).padding(.bottom, 8)
                        .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                }
                if let error = model.error {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Couldn’t refresh inbox").appearanceText(Appearance.rowHeading)
                        Text(error).foregroundStyle(Appearance.error)
                        Button("Retry") { Task { await model.refresh() } }.frame(minHeight: 44)
                    }
                }
                if let snapshot = model.snapshot {
                    if model.groups.isEmpty {
                        ContentUnavailableView(
                            snapshot.isEmpty ? "Inbox is clear" : "No matching items", systemImage: "tray",
                            description: Text("Choose another project or state, or pull to refresh."))
                    }
                    ForEach(model.groups) { group in
                        Section {
                            ForEach(group.items) { item in
                                InboxRow(item: item, triage: model.triage[item.id], error: model.triageErrors[item.id]) {
                                    Task { await model.loadTriage(item) }
                                }.task(id: model.generation) { await model.loadTriage(item) }
                                    .listRowSeparator(.hidden).listRowBackground(Appearance.page)
                            }
                        } header: {
                            HStack(alignment: .firstTextBaseline) {
                                Text(group.title.uppercased())
                                Spacer(minLength: 8)
                                Text("\(group.items.count) \(group.items.count == 1 ? "item" : "items")")
                            }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                                .padding(.horizontal, 6).padding(.top, 8).accessibilityAddTraits(.isHeader)
                        }
                    }
                } else if model.isLoading {
                    ProgressView("Loading inbox…").frame(maxWidth: .infinity)
                }
            }
            .listStyle(.plain).listSectionSpacing(12)
            .environment(\.defaultMinListRowHeight, 0)
            .listRowSpacing(6)
            .scrollContentBackground(.hidden)
            .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
            .background(Appearance.page)
            .toolbar(.hidden, for: .navigationBar)
            .safeAreaInset(edge: .top, spacing: 0) {
                HStack(spacing: 12) {
                    Button { showingHub = true } label: {
                        Label("mergegate", systemImage: "arrow.triangle.branch")
                            .appearanceText(Appearance.label).frame(minHeight: 44)
                            .contentShape(Rectangle())
                    }.buttonStyle(.plain).foregroundStyle(Appearance.brand)
                        .accessibilityLabel("Hub settings").accessibilityIdentifier("hubSettings")
                    Spacer(minLength: 0)
                    Menu {
                        Picker("Project", selection: $model.project) {
                            Text("All projects").tag(String?.none)
                            ForEach(model.snapshot?.projects ?? [], id: \.self) { project in
                                Text(project).tag(Optional(project))
                            }
                        }
                    } label: {
                        HStack(spacing: 6) {
                            Text(model.project ?? "All projects")
                            Image(systemName: "chevron.down")
                        }.appearanceText(Appearance.caption).lineLimit(1).frame(minHeight: 44)
                    }.accessibilityLabel("Project").accessibilityValue(model.project ?? "All projects")
                        .accessibilityIdentifier("projectFilter")
                }.padding(.horizontal, 24).background(Appearance.page)
            }
            .safeAreaInset(edge: .bottom, spacing: 0) { footer }
            .refreshable { await model.refresh() }
            .task { await model.refresh() }
            .onChange(of: scenePhase) { _, phase in
                if phase == .active { Task { await model.refresh() } }
            }
            .sheet(isPresented: $showingHub) { hubSettings }
        }
    }

    private var footer: some View {
        HStack(spacing: 12) {
            Label(hub.url.host ?? "Hub", systemImage: "network")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary).lineLimit(1)
            Spacer(minLength: 0)
            Menu {
                Picker("State", selection: $model.filter) {
                    ForEach(InboxFilter.allCases) { filter in Text(filter.rawValue).tag(filter) }
                }
            } label: {
                Label(stateLabel, systemImage: "chevron.down").appearanceText(Appearance.caption)
                    .frame(minHeight: 44)
            }.accessibilityLabel("State").accessibilityValue(model.filter.rawValue)
                .accessibilityIdentifier("stateFilter")
        }.padding(.horizontal, 24).padding(.top, 4).background(Appearance.page)
            .overlay(alignment: .top) { Divider() }
    }

    private var stateLabel: String {
        switch model.filter {
        case .all, .decisions, .review: "Other states"
        default: model.filter.rawValue
        }
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
