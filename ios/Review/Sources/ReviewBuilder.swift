import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build<
        Drafts: View, Conversations: View, FileConversations: View, Feedback: View, Rules: View
    >(
        hub: ConnectedHub, item: String, styling: any CodeStyling,
        openItem: @escaping (String) -> Void,
        @ViewBuilder drafts:
            @escaping (CommentDraftScope, CommentDraftLocation?, @escaping () -> Void) -> Drafts,
        @ViewBuilder conversations:
            @escaping (
                CommentDraftScope, ConversationLocation?, @escaping (ConversationLocation) -> Void,
                @escaping () -> Void,
                @escaping () -> Void
            ) -> Conversations,
        @ViewBuilder fileConversations: @escaping (CommentDraftScope, String) -> FileConversations,
        @ViewBuilder rules: @escaping (String, @escaping (String) -> Void) -> Rules,
        @ViewBuilder feedback:
            @escaping (
                CommentDraftScope, @escaping () async -> Void, @escaping () -> Void,
                @escaping () -> Void,
                @escaping () -> Void
            ) -> Feedback
    ) -> some View {
        ReviewScene(
            model: model(
                service: ReviewWebService(
                    transport: URLSessionTransport(), url: hub.url, cache: ReviewCache()),
                item: item),
            styling: styling, hub: hub.url, drafts: drafts, conversations: conversations,
            fileConversations: fileConversations, feedback: feedback, rules: rules,
            openItem: openItem)
    }

    @MainActor static func model(service: ReviewWebService, item: String) -> ReviewViewModel {
        do {
            return ReviewViewModel(
                service: service, item: item, readingStore: try ReviewReadingStore())
        } catch {
            let model = ReviewViewModel(service: service, item: item)
            model.positionFailure =
                "Couldn’t open reading-position storage. \(error.localizedDescription)"
            return model
        }
    }

}
