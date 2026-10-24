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
                if let error = model.error {
                    Section {
                        Text(error).foregroundStyle(Appearance.error)
                        Button("Retry") { Task { await model.refresh() } }.frame(minHeight: 44)
                    } header: {
                        Text("Couldn’t refresh inbox").foregroundStyle(Appearance.secondary)
                    }
                }
                if let snapshot = model.snapshot {
                    Section {
                        Picker("Project", selection: $model.project) {
                            Text("All projects").tag(String?.none)
                            ForEach(snapshot.projects, id: \.self) { project in
                                Text(project).tag(Optional(project))
                            }
                        }.frame(minHeight: 44).accessibilityIdentifier("projectFilter")
                        Picker("State", selection: $model.filter) {
                            ForEach(InboxFilter.allCases) { filter in Text(filter.rawValue).tag(filter) }
                        }.frame(minHeight: 44).accessibilityIdentifier("stateFilter")
                    }.listRowBackground(Appearance.page)
                    if model.groups.isEmpty {
                        ContentUnavailableView(
                            snapshot.isEmpty ? "Inbox is clear" : "No matching items", systemImage: "tray",
                            description: Text("Choose another project or state, or pull to refresh."))
                    }
                    ForEach(model.groups) { group in
                        Section {
                            Text("\(group.title) · \(group.items.count)")
                                .appearanceText(Appearance.label)
                                .listRowBackground(Appearance.page)
                                .accessibilityAddTraits(.isHeader)
                            ForEach(group.items) { item in
                                InboxRow(item: item, triage: model.triage[item.id], error: model.triageErrors[item.id]) {
                                    Task { await model.loadTriage(item) }
                                }.task(id: model.generation) { await model.loadTriage(item) }
                            }
                        }
                    }
                } else if model.isLoading {
                    ProgressView("Loading inbox…").frame(maxWidth: .infinity)
                }
            }
            .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
            .scrollContentBackground(.hidden).background(Appearance.page)
            .navigationTitle("Decision desk")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Hub", systemImage: "network") { showingHub = true }
                        .accessibilityIdentifier("hubSettings")
                }
            }
            .refreshable { await model.refresh() }
            .task { await model.refresh() }
            .onChange(of: scenePhase) { _, phase in
                if phase == .active { Task { await model.refresh() } }
            }
            .sheet(isPresented: $showingHub) {
                NavigationStack {
                    List {
                        Section("Hub") {
                            Text(hub.url.absoluteString).textSelection(.enabled)
                        }
                        Section("Device") { Text(hub.deviceName) }
                        Button("Forget hub", role: .destructive) { disconnect() }
                            .frame(minHeight: 44).accessibilityIdentifier("forgetHub")
                    }.appearanceText(Appearance.body)
                        .navigationTitle("Connected hub")
                        .toolbar { Button("Done") { showingHub = false } }
                }
            }
        }
    }
}
