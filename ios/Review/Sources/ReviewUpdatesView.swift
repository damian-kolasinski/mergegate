import SwiftUI

struct ReviewUpdatesView: View {
    let model: ReviewViewModel
    let revision: UInt32

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let latest = model.latestRevision, latest > revision {
                VStack(alignment: .leading, spacing: 4) {
                    Label(
                        "New revision r\(latest) available",
                        systemImage: "arrow.triangle.2.circlepath"
                    )
                    .appearanceText(Appearance.label)
                    Text(
                        "You’re reading r\(revision). Drafts stay with this revision. Return to the inbox to open r\(latest)."
                    )
                    .appearanceText(Appearance.caption)
                }.accessibilityIdentifier("reviewRevisionUpdate")
            }
            if let state = model.latestState, ["merged", "abandoned", "merging"].contains(state) {
                Text(
                    state == "merged"
                        ? "This item has merged."
                        : state == "abandoned"
                            ? "This item was abandoned." : "This item is merging."
                )
                .appearanceText(Appearance.caption).accessibilityIdentifier("reviewStateUpdate")
            }
            if model.isReconnecting {
                Label(
                    "Reconnecting… Your review and drafts are kept.",
                    systemImage: "wifi.exclamationmark"
                )
                .appearanceText(Appearance.caption).accessibilityIdentifier("reviewReconnecting")
            }
            if let failure = model.refreshFailure {
                Text(failure).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                Button("Retry live updates") { Task { await model.refreshCurrent() } }
                    .appearanceText(Appearance.label).frame(minHeight: 44)
                    .accessibilityIdentifier("retryReviewUpdates")
            }
        }.foregroundStyle(Appearance.secondary).frame(maxWidth: .infinity, alignment: .leading)
    }
}
