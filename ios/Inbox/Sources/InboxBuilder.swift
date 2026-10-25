import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum InboxBuilder {
    @MainActor
    public static func build<Review: View, Feedback: View, Rules: View>(
        hub: ConnectedHub, disconnect: @escaping () -> Void,
        @ViewBuilder feedback: @escaping (CommentDraftScope, @escaping () -> Void) -> Feedback,
        @ViewBuilder rules: @escaping (String, @escaping (String) -> Void) -> Rules,
        @ViewBuilder review: @escaping (String, @escaping (String) -> Void) -> Review
    ) -> some View {
        InboxScene(
            hub: hub,
            model: InboxViewModel(
                service: InboxWebService(transport: URLSessionTransport(), url: hub.url),
                cache: InboxCache()),
            disconnect: disconnect, feedback: feedback, rules: rules, review: review
        )
    }
}
