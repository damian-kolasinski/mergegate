import ConversationInterface
import Networking
import SwiftUI

public enum ConversationBuilder {
    @MainActor
    public static func threads(
        scope: CommentDraftScope, location: ConversationLocation?,
        showSource: @escaping (ConversationLocation) -> Void = { _ in },
        onActivity: @escaping () -> Void = {}, close: @escaping () -> Void
    ) -> some View {
        ConversationScene(
            model: ConversationViewModel(
                scope: scope, path: location?.path, anchors: location?.anchors ?? [],
                service: .init(transport: URLSessionTransport())), close: close,
            showSource: showSource)
    }

    @MainActor
    public static func fileThreads(scope: CommentDraftScope, path: String) -> some View {
        SourceConversationsView(
            model: ConversationViewModel(
                scope: scope, path: path, service: .init(transport: URLSessionTransport())))
    }

    /// Wraps a file's reader in what its conversations add to it: where cards go, and the cards.
    /// `refresh` changes when local drafts may have changed.
    @MainActor
    public static func annotations<Content: View>(
        scope: CommentDraftScope, path: String, refresh: Int,
        @ViewBuilder content: @escaping (SourceAnnotations) -> Content
    ) -> some View {
        FileAnnotations(scope: scope, path: path, refresh: refresh, content: content)
    }

    @MainActor
    public static func drafts(
        scope: CommentDraftScope, location: CommentDraftLocation?, close: @escaping () -> Void
    ) -> some View {
        CommentDraftScene(scope: scope, location: location, close: close)
    }
}
