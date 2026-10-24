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
                    if snapshot.isEmpty {
                        ContentUnavailableView(
                            "Inbox is clear", systemImage: "tray",
                            description: Text("New work appears here after you refresh."))
                    }
                    items(snapshot.needsYou, heading: "Needs you")
                    items(snapshot.withAgents, heading: "In progress")
                    items(snapshot.merging, heading: "Merging")
                    items(snapshot.closed, heading: "Closed today")
                } else if model.isLoading {
                    ProgressView("Loading inbox…").frame(maxWidth: .infinity)
                }
            }
            .appearanceText(Appearance.body).foregroundStyle(Appearance.primary)
            .scrollContentBackground(.hidden).background(Appearance.page)
            .navigationTitle("Inbox")
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

    @ViewBuilder
    private func items(_ items: [InboxItem], heading: String) -> some View {
        if !items.isEmpty {
            Section {
                Text(heading)
                    .appearanceText(Appearance.label)
                    .foregroundStyle(Appearance.primary)
                    .listRowBackground(Appearance.page)
                    .listRowSeparator(.hidden)
                    .accessibilityAddTraits(.isHeader)
                ForEach(items) { item in InboxRow(item: item) }
            }
        }
    }
}
