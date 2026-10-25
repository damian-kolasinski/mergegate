import CodeStylingInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build(hub: ConnectedHub, item: String, styling: any CodeStyling) -> some View {
        ReviewScene(
            model: ReviewViewModel(
                service: .init(transport: URLSessionTransport(), url: hub.url), item: item),
            styling: styling)
    }
}
