import SwiftUI

struct ReviewUpdatesView: View {
    let model: ReviewViewModel
    let revision: UInt32

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let date = model.cachedAt {
                VStack(alignment: .leading, spacing: 4) {
                    Label("Offline · saved review", systemImage: "wifi.slash")
                        .appearanceText(Appearance.label).accessibilityIdentifier("reviewOffline")
                    Text(
                        "Saved \(date.formatted(date: .abbreviated, time: .shortened)). Reconnect to check the revision."
                    )
                    .appearanceText(Appearance.caption).accessibilityIdentifier("offlineReviewDetail")
                    Button("Reconnect") { Task { await model.refreshCurrent() } }.frame(minHeight: 44)
                        .accessibilityIdentifier("reconnectReview")
                }.foregroundStyle(Appearance.primary)
            }
            if let message = model.positionFailure ?? model.cacheFailure {
                Text(message).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
            }
            if let latest = model.latestRevision, latest > revision {
                VStack(alignment: .leading, spacing: 4) {
                    Label(
                        "New revision r\(latest) available",
                        systemImage: "arrow.triangle.2.circlepath"
                    )
                    .appearanceText(Appearance.label).accessibilityIdentifier("reviewRevisionUpdate")
                    Text(
                        "You’re reading r\(revision). Drafts stay with this revision. Open r\(latest) when you’re ready."
                    )
                    .appearanceText(Appearance.caption)
                    Button("Open r\(latest)") { Task { await model.load(allowCached: false) } }
                        .appearanceText(Appearance.label).frame(minHeight: 44)
                        .disabled(model.isComparing || model.isSavingViewed)
                        .accessibilityIdentifier("openLatestRevision")
                }
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
            if model.isReconnecting && model.cachedAt == nil {
                Label(
                    "Reconnecting… Your review and drafts are kept.",
                    systemImage: "wifi.exclamationmark"
                )
                .appearanceText(Appearance.caption).accessibilityIdentifier("reviewReconnecting")
            }
            if let failure = model.refreshFailure, model.cachedAt == nil {
                Text(failure).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                Button("Retry live updates") { Task { await model.refreshCurrent() } }
                    .appearanceText(Appearance.label).frame(minHeight: 44)
                    .accessibilityIdentifier("retryReviewUpdates")
            }
        }.foregroundStyle(Appearance.secondary).frame(maxWidth: .infinity, alignment: .leading)
    }
}
