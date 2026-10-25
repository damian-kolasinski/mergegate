import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build<
        Drafts: View, Conversations: View, FileConversations: View, Feedback: View
    >(
        hub: ConnectedHub, item: String, styling: any CodeStyling,
        @ViewBuilder drafts:
            @escaping (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> Drafts,
        @ViewBuilder conversations:
            @escaping (CommentDraftScope, ConversationLocation?, @escaping () -> Void) -> Conversations,
        @ViewBuilder fileConversations: @escaping (CommentDraftScope, String) -> FileConversations,
        @ViewBuilder feedback: @escaping (CommentDraftScope, @escaping () -> Void) -> Feedback
    ) -> some View {
        ReviewScene(
            model: model(hub: hub, item: item),
            styling: styling, hub: hub.url, drafts: drafts, conversations: conversations,
            fileConversations: fileConversations, feedback: feedback)
    }

    @MainActor private static func model(hub: ConnectedHub, item: String) -> ReviewViewModel {
        let service = ReviewWebService(
            transport: URLSessionTransport(), url: hub.url, cache: ReviewCache())
        do {
            return ReviewViewModel(service: service, item: item, readingStore: try ReviewReadingStore())
        } catch {
            let model = ReviewViewModel(service: service, item: item)
            model.positionFailure =
                "Couldn’t open reading-position storage. \(error.localizedDescription)"
            return model
        }
    }

}
