import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build<Drafts: View, Conversations: View, FileConversations: View, Feedback: View>(
        hub: ConnectedHub, item: String, styling: any CodeStyling,
        @ViewBuilder drafts: @escaping (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> Drafts,
        @ViewBuilder conversations:
            @escaping (CommentDraftScope, ConversationLocation?, @escaping () -> Void) -> Conversations,
        @ViewBuilder fileConversations: @escaping (CommentDraftScope, String) -> FileConversations,
        @ViewBuilder feedback: @escaping (CommentDraftScope, @escaping () -> Void) -> Feedback
    ) -> some View {
        ReviewScene(
            model: ReviewViewModel(
                service: .init(transport: URLSessionTransport(), url: hub.url), item: item),
            styling: styling, hub: hub.url, drafts: drafts, conversations: conversations,
            fileConversations: fileConversations, feedback: feedback)
    }
}
