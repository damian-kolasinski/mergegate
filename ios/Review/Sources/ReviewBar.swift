import ConversationInterface
import SwiftUI

// The reader's bottom bar: the viewed mark with progress, then the next step of the review and,
// beside it, every outcome, so each frequent action stays one or two taps away.
struct ReviewBar: View {
    let file: ReviewFile?
    let viewed: Int
    let total: Int
    let saving: Bool
    let disabled: Bool
    let failure: ReviewViewedFailure?
    let primary: ReviewPrimaryAction?
    /// Feedback can be written while offline, though only the primary step needs the hub.
    let canComplete: Bool
    let mark: () -> Void
    let retry: () -> Void
    let perform: (ReviewPrimaryAction) -> Void
    let complete: (ReviewOutcome) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let failure {
                HStack(spacing: 8) {
                    Text("Couldn’t update \(failure.change.path): \(failure.message)")
                        .foregroundStyle(Appearance.error).lineLimit(2)
                        .accessibilityIdentifier("viewedError")
                    Button("Retry", action: retry).disabled(saving || disabled)
                        .accessibilityIdentifier("retryViewed")
                }.appearanceText(Appearance.caption).padding(.horizontal, 12)
            }
            HStack(spacing: 6) {
                viewedControl
                Spacer(minLength: 4)
                if let primary {
                    Button {
                        perform(primary)
                    } label: {
                        Text(primary.title).appearanceText(Appearance.label)
                            .foregroundStyle(Appearance.onFill).padding(.horizontal, 4)
                            .frame(minHeight: 44)
                    }.buttonStyle(.glassProminent).tint(Appearance.brand)
                        .disabled(saving).accessibilityIdentifier("reviewPrimary")
                }
                if canComplete { outcomes }
            }
        }.padding(6).glassEffect(.regular, in: RoundedRectangle(cornerRadius: 28))
            .tint(Appearance.brand)
    }

    @ViewBuilder private var viewedControl: some View {
        let progress = Text("\(viewed)/\(total)").appearanceText(Appearance.caption)
            .monospacedDigit().foregroundStyle(Appearance.secondary)
            .accessibilityLabel("\(viewed) of \(total) viewed")
            .accessibilityIdentifier("reviewViewedProgress")
        HStack(spacing: 2) {
            if let file, file.tier == .review, file.interdiff != .dropped {
                Button(action: mark) {
                    Group {
                        if saving {
                            ProgressView().accessibilityLabel("Saving viewed state")
                        } else {
                            Image(systemName: file.viewed ? "checkmark.circle.fill" : "circle")
                                .font(.title2)
                        }
                    }.frame(width: 44, height: 44).contentShape(Rectangle())
                }.disabled(saving || disabled)
                    .accessibilityLabel(file.viewed ? "Viewed" : "Mark viewed")
                    .accessibilityValue(file.viewed ? "Viewed" : "Not viewed")
                    .accessibilityHint(
                        file.viewed ? "Mark this file as not viewed" : "Mark this file as viewed")
                    .accessibilityIdentifier("reviewViewed")
            } else {
                Image(systemName: file?.tier == .trusted ? "checkmark.shield" : "minus.circle")
                    .font(.title2).foregroundStyle(Appearance.secondary)
                    .frame(width: 44, height: 44)
                    .accessibilityLabel(
                        file?.tier == .trusted
                            ? "Trusted · counts as viewed" : "Dropped change · no viewed mark")
                    .accessibilityIdentifier("trustedViewed")
            }
            progress
        }
    }

    private var outcomes: some View {
        Menu {
            Button("Send comments", systemImage: "paperplane") { complete(.comment) }
            Button("Request changes", systemImage: "arrow.uturn.backward") {
                complete(.changesRequested)
            }
            Button("Approve & merge", systemImage: "checkmark.seal") { complete(.approved) }
        } label: {
            Image(systemName: "chevron.down").font(.body.weight(.semibold))
                .frame(width: 44, height: 44)
        }.accessibilityLabel("Review outcomes").accessibilityIdentifier("reviewOutcomes")
            .disabled(saving)
    }
}
