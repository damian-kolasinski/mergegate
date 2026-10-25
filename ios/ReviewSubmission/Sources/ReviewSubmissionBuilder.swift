import ConversationInterface
import Networking
import SwiftUI

public enum ReviewSubmissionBuilder {
    @MainActor
    public static func build(
        scope: CommentDraftScope, drafts: any FeedbackDrafts, close: @escaping () -> Void
    ) -> AnyView {
        AnyView(FeedbackRoot(scope: scope, drafts: drafts, close: close))
    }
}

private struct FeedbackRoot: View {
    let scope: CommentDraftScope
    let drafts: any FeedbackDrafts
    let close: () -> Void
    @State private var model: FeedbackViewModel?
    @State private var failure: String?

    var body: some View {
        Group {
            if let model {
                FeedbackScene(model: model, close: close)
            } else {
                VStack(spacing: 16) {
                    Text(failure ?? "Opening feedback…")
                    Button("Retry", action: open).frame(minHeight: 44)
                    Button("Done", action: close).frame(minHeight: 44)
                }.padding(20)
            }
        }.task { open() }
    }

    private func open() {
        do {
            model = try FeedbackViewModel(
                scope: scope, drafts: drafts, outbox: SwiftDataFeedbackOutbox(),
                service: .init(transport: URLSessionTransport()))
            failure = nil
        } catch { failure = error.localizedDescription }
    }
}
