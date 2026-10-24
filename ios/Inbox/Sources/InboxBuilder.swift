import HubConnectionInterface
import Networking
import SwiftUI

public enum InboxBuilder {
    @MainActor
    public static func build(hub: ConnectedHub, disconnect: @escaping () -> Void) -> some View {
        InboxScene(
            hub: hub,
            model: InboxViewModel(service: InboxWebService(transport: URLSessionTransport(), url: hub.url)),
            disconnect: disconnect
        )
    }
}
