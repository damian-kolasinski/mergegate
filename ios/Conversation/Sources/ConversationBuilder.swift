import ConversationInterface
import SwiftUI

public enum ConversationBuilder {
    @MainActor
    public static func drafts(
        scope: CommentDraftScope, location: CommentDraftLocation?, close: @escaping () -> Void
    ) -> AnyView {
        AnyView(CommentDraftScene(scope: scope, location: location, close: close))
    }
}
