import HubConnectionInterface
import SwiftUI

struct HubConnectionScene<Content: View>: View {
    @State var model: HubConnectionViewModel
    let inbox: (ConnectedHub, @escaping () -> Void) -> Content

    var body: some View {
        Group {
            switch model.state {
            case .connected(let hub): inbox(hub, model.forget)
            case .disconnected, .connecting: connection
            }
        }
        .tint(Appearance.brand)
        .task { await model.restore() }
    }

    private var connection: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    Image(systemName: "network")
                        .font(.system(size: 36)).foregroundStyle(Appearance.brand)
                        .accessibilityHidden(true)
                    Text("Your review desk, wherever you are.")
                        .appearanceText(Appearance.heading)
                    Text("Connect Tailscale on this device, then enter your hub’s address.")
                        .appearanceText(Appearance.body).foregroundStyle(Appearance.secondary)
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Hub URL").appearanceText(Appearance.label)
                        TextField("http://my-mac:7800", text: $model.address)
                            .textContentType(.URL).keyboardType(.URL)
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                            .textFieldStyle(.roundedBorder).appearanceText(Appearance.body)
                            .frame(minHeight: 44)
                            .accessibilityIdentifier("hubURL")
                            .accessibilityLabel("Hub URL")
                            .submitLabel(.go)
                            .onSubmit { Task { await model.connect() } }
                            .disabled(isConnecting)
                        Text("On your hub’s machine, run mg hub set listen tailnet to find the address.")
                            .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    }
                    if let error = model.error {
                        Text(error).appearanceText(Appearance.body).foregroundStyle(Appearance.error)
                            .accessibilityIdentifier("connectionError")
                    }
                    Button {
                        Task { await model.connect() }
                    } label: {
                        HStack {
                            if isConnecting { ProgressView() }
                            Text(isConnecting ? "Connecting…" : "Connect to hub")
                        }.frame(maxWidth: .infinity, minHeight: 44)
                            .foregroundStyle(Appearance.onFill)
                    }
                    .buttonStyle(.borderedProminent).appearanceText(Appearance.label)
                    .disabled(isConnecting || model.address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("connectHub")
                }.padding(24)
                    .foregroundStyle(Appearance.primary)
            }
            .scrollDismissesKeyboard(.immediately)
            .background(Appearance.page)
            .navigationTitle("mergegate")
        }
    }

    private var isConnecting: Bool {
        if case .connecting = model.state { true } else { false }
    }
}
