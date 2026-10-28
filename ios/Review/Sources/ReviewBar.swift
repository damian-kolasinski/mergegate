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
    /// Opens the comment on the whole review.
    let comment: () -> Void
    let perform: (ReviewPrimaryAction) -> Void
    let complete: (ReviewOutcome) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let failure {
                HStack(spacing: 8) {
                    Text("Couldn’t update \(failure.change.path): \(failure.message)")
                        .foregroundStyle(Appearance.error).fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("viewedError")
                    Button("Retry", action: retry).disabled(saving || disabled)
                        .accessibilityIdentifier("retryViewed")
                }.appearanceText(Appearance.caption).padding(.horizontal, 12)
            }
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 6) {
                    viewedControl
                    commentControl
                    Spacer(minLength: 4)
                    primaryControl.fixedSize(horizontal: true, vertical: false)
                    if canComplete { outcomes }
                }
                VStack(spacing: 6) {
                    HStack(spacing: 6) {
                        viewedControl
                        Spacer(minLength: 4)
                        commentControl
                        if canComplete { outcomes }
                    }
                    primaryControl.frame(maxWidth: .infinity)
                }
            }
        }.padding(6)
            .background(Appearance.page, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
            .overlay(RoundedRectangle(cornerRadius: Appearance.controlRadius).strokeBorder(Appearance.border))
            .tint(Appearance.brand)
    }

    private var commentControl: some View {
        Button(action: comment) {
            Image(systemName: "text.bubble").font(.system(size: 20)).frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }.buttonStyle(.plain).foregroundStyle(Appearance.brand)
            .accessibilityLabel("Review comment").accessibilityIdentifier("reviewComment")
    }

    @ViewBuilder private var primaryControl: some View {
        if let primary {
            Button { perform(primary) } label: {
                Text(primary.title).frame(maxWidth: .infinity)
            }.buttonStyle(AppearanceButtonStyle(prominent: true))
                .disabled(saving).accessibilityIdentifier("reviewPrimary")
        }
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
                                .font(.system(size: 22))
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
                    .font(.system(size: 22)).foregroundStyle(Appearance.secondary)
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
            Image(systemName: "chevron.down").font(.system(size: 16, weight: .semibold))
                .frame(width: 44, height: 44)
        }.accessibilityLabel("Review outcomes").accessibilityIdentifier("reviewOutcomes")
            .disabled(saving)
    }
}
