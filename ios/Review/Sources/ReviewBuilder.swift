import CodeStylingInterface
import ConversationInterface
import HubConnectionInterface
import Networking
import SwiftUI

public enum ReviewBuilder {
    @MainActor
    public static func build<
        Drafts: View, Conversations: View, FileConversations: View, Feedback: View, Rules: View,
        Annotations: View, Summary: View
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
        @ViewBuilder annotations:
            @escaping (
                CommentDraftScope, String, Int, @escaping (SourceAnnotations) -> AnyView
            ) -> Annotations,
        @ViewBuilder summary: @escaping (CommentDraftScope, @escaping () -> Void) -> Summary,
        @ViewBuilder rules: @escaping (String, @escaping (String) -> Void) -> Rules,
        localDrafts: @escaping (CommentDraftScope) -> Int,
        @ViewBuilder feedback:
            @escaping (
                CommentDraftScope, ReviewOutcome, Int, @escaping () async -> Void,
                @escaping () -> Void, @escaping () -> Void, @escaping () -> Void
            ) -> Feedback
    ) -> some View {
        ReviewScene(
            model: model(
                service: ReviewWebService(
                    transport: URLSessionTransport(), url: hub.url, cache: ReviewCache()),
                item: item),
            styling: styling, hub: hub.url, drafts: drafts, conversations: conversations,
            fileConversations: fileConversations, annotations: annotations, summary: summary,
            feedback: feedback, rules: rules,
            localDrafts: localDrafts, openItem: openItem)
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
