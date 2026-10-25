import ConversationInterface
import Networking
import SwiftUI

public enum ConversationBuilder {
    @MainActor
    public static func threads(
        scope: CommentDraftScope, location: ConversationLocation?, close: @escaping () -> Void
    ) -> AnyView {
        AnyView(
            ConversationScene(
                model: ConversationViewModel(
                    scope: scope, path: location?.path, anchors: location?.anchors ?? [],
                    service: .init(transport: URLSessionTransport())), close: close))
    }

    @MainActor
    public static func fileThreads(scope: CommentDraftScope, path: String) -> AnyView {
        AnyView(
            SourceConversationsView(
                model: ConversationViewModel(
                    scope: scope, path: path, service: .init(transport: URLSessionTransport()))))
    }

    @MainActor
    public static func drafts(
        scope: CommentDraftScope, location: CommentDraftLocation?, close: @escaping () -> Void
    ) -> AnyView {
        AnyView(CommentDraftScene(scope: scope, location: location, close: close))
    }
}
