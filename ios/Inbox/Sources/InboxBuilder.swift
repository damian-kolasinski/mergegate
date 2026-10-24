import HubConnectionInterface
import Networking
import SwiftUI

public enum InboxBuilder {
    @MainActor
    public static func build<Review: View>(
        hub: ConnectedHub, disconnect: @escaping () -> Void,
        @ViewBuilder review: @escaping (String) -> Review
    ) -> some View {
        InboxScene(
            hub: hub,
            model: InboxViewModel(service: InboxWebService(transport: URLSessionTransport(), url: hub.url)),
            disconnect: disconnect, review: review
        )
    }
}
