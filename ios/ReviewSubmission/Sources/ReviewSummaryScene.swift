import ConversationInterface
import Networking
import SwiftUI

struct SummaryRoot: View {
    let scope: CommentDraftScope
    let drafts: () throws -> any FeedbackDrafts
    let close: () -> Void
    @State private var model: FeedbackViewModel?
    @State private var failure: String?

    var body: some View {
        Group {
            if let model {
                ReviewSummaryScene(model: model, close: close)
            } else {
                VStack(spacing: 16) {
                    Text(failure ?? "Opening review comment…")
                    Button("Done", action: close).frame(minHeight: 44)
                }.padding(20)
            }
        }.task {
            do {
                model = try FeedbackViewModel(
                    scope: scope, drafts: try drafts(), outbox: SwiftDataFeedbackOutbox(),
                    service: .init(transport: URLSessionTransport()))
            } catch { failure = error.localizedDescription }
        }
    }
}

// A comment on the whole review, written whenever it comes to mind instead of only when finishing.
// It is the summary the completion sheet sends with whichever outcome the operator picks.
struct ReviewSummaryScene: View {
    @State var model: FeedbackViewModel
    let close: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Review comment").appearanceText(Appearance.heading)
                        .accessibilityAddTraits(.isHeader)
                    Text("\(model.scope.name) · r\(model.scope.revision)")
                        .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                }
                Spacer(minLength: 12)
                Button(action: close) {
                    Text("Done").appearanceText(Appearance.label).frame(minWidth: 28)
                }.buttonStyle(AppearanceButtonStyle()).disabled(!model.summarySaved)
                    .accessibilityIdentifier("closeReviewComment")
            }
            TextEditor(text: $model.summary).appearanceText(Appearance.body)
                .scrollContentBackground(.hidden).padding(8)
                .background(Appearance.surface, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
                .focused($focused).disabled(model.frozen)
                .accessibilityLabel("Review comment").accessibilityIdentifier("reviewComment")
            Text(
                model.frozen
                    ? "Retained with a delivery in progress. Finish it from the review bar."
                    : "Saved on this iPhone and sent with your next review, whatever its outcome."
            ).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
            if let error = model.error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
            }
        }
        .padding(20).background(Appearance.page).foregroundStyle(Appearance.primary)
        .tint(Appearance.brand)
        .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
        .interactiveDismissDisabled(!model.summarySaved)
        .onChange(of: model.summary) { _, _ in model.saveSummary() }
        .onAppear { focused = true }
    }
}
