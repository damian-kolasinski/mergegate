import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build(
        hub: ConnectedHub, item: String, styling: any CodeStyling,
        drafts: @escaping (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> AnyView
    ) -> some View {
        ReviewScene(
            model: ReviewViewModel(
                service: .init(transport: URLSessionTransport(), url: hub.url), item: item),
            styling: styling, hub: hub.url, drafts: drafts)
    }
}
