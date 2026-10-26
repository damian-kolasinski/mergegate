import ConversationInterface
import Networking
import SwiftUI

public enum ReviewSubmissionBuilder {
    /// The whole-review comment, kept with the feedback the next review sends.
    @MainActor
    public static func summary(
        scope: CommentDraftScope, drafts: @escaping () throws -> any FeedbackDrafts,
        close: @escaping () -> Void
    ) -> some View {
        SummaryRoot(scope: scope, drafts: drafts, close: close)
    }

    /// Whether a whole-review comment waits to be sent with the next review.
    @MainActor
    public static func hasSummary(scope: CommentDraftScope) -> Bool {
        let summary = (try? SwiftDataFeedbackOutbox().load(scope: scope))?.summary ?? ""
        return !summary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    @MainActor
    public static func build(
        scope: CommentDraftScope, outcome: ReviewOutcome? = nil, unreviewed: Int = 0,
        drafts: @escaping () throws -> any FeedbackDrafts,
        beforeSend: @escaping () async -> Void = {}, onActivity: @escaping () -> Void = {},
        onSent: @escaping () -> Void = {},
        close: @escaping () -> Void
    ) -> some View {
        FeedbackRoot(
            scope: scope, outcome: outcome, unreviewed: unreviewed, drafts: drafts,
            beforeSend: beforeSend, onActivity: onActivity, onSent: onSent, close: close)
    }
}

private struct FeedbackRoot: View {
    let scope: CommentDraftScope
    let outcome: ReviewOutcome?
    let unreviewed: Int
    let drafts: () throws -> any FeedbackDrafts
    let beforeSend: () async -> Void
    let onActivity: () -> Void
    let onSent: () -> Void
    let close: () -> Void
    @State private var model: FeedbackViewModel?
    @State private var failure: String?

    var body: some View {
        Group {
            if let model {
                FeedbackScene(model: model, unreviewed: unreviewed, close: close)
                .onChange(of: model.summary) { _, _ in
                    onActivity()
                }
                .onChange(of: model.sent) { _, sent in if sent { onSent() } }
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
            let model = try FeedbackViewModel(
                scope: scope, drafts: try drafts(), outbox: SwiftDataFeedbackOutbox(),
                service: .init(transport: URLSessionTransport()), beforeSend: beforeSend)
            // A retained delivery keeps its own verdict; the setter ignores frozen feedback.
            if let outcome { model.verdict = FeedbackVerdict(outcome) }
            self.model = model
            failure = nil
        } catch { failure = error.localizedDescription }
    }
}
