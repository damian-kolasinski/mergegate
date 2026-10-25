import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum InboxBuilder {
    @MainActor
    public static func build<Review: View, Feedback: View>(
        hub: ConnectedHub, disconnect: @escaping () -> Void,
        @ViewBuilder feedback: @escaping (CommentDraftScope, @escaping () -> Void) -> Feedback,
        @ViewBuilder review: @escaping (String) -> Review
    ) -> some View {
        InboxScene(
            hub: hub,
            model: InboxViewModel(
                service: InboxWebService(transport: URLSessionTransport(), url: hub.url),
                cache: InboxCache()),
            disconnect: disconnect, feedback: feedback, review: review
        )
    }
}
