import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build(
        hub: ConnectedHub, item: String, styling: any CodeStyling,
        drafts: @escaping (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> AnyView,
        conversations:
            @escaping (CommentDraftScope, ConversationLocation?, @escaping () -> Void) -> AnyView,
        fileConversations: @escaping (CommentDraftScope, String) -> AnyView,
        feedback: @escaping (CommentDraftScope, @escaping () -> Void) -> AnyView
    ) -> some View {
        ReviewScene(
            model: ReviewViewModel(
                service: .init(transport: URLSessionTransport(), url: hub.url), item: item),
            styling: styling, hub: hub.url, drafts: drafts, conversations: conversations,
            fileConversations: fileConversations, feedback: feedback)
    }
}
