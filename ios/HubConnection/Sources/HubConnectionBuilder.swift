import HubConnectionInterface
import Networking
import SwiftUI

public enum HubConnectionBuilder {
    @MainActor
    public static func build<Content: View>(
        @ViewBuilder inbox: @escaping (ConnectedHub, @escaping () -> Void) -> Content
    ) -> some View {
        HubConnectionScene(
            model: HubConnectionViewModel(
                interactor: HubConnectionInteractor(
                    service: HubConnectionWebService(transport: URLSessionTransport()),
                    selection: HubSelection(defaults: .standard)
                )), inbox: inbox
        )
    }
}
