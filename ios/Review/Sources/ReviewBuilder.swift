import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build(hub: ConnectedHub, item: String) -> some View {
        ReviewScene(model: ReviewViewModel(service: .init(transport: URLSessionTransport(), url: hub.url), item: item))
    }
}
