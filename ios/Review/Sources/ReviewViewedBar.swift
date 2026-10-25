import SwiftUI

struct ReviewViewedBar: View {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let file: ReviewFile
    let viewed: Int
    let total: Int
    let saving: Bool
    let failure: ReviewViewedFailure?
    let mark: () -> Void
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider()
            if let failure {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Couldn’t update \(failure.change.path): \(failure.message)")
                        .foregroundStyle(Appearance.error).accessibilityIdentifier("viewedError")
                    Button("Retry viewed change", action: retry).frame(minHeight: 44)
                        .disabled(saving).accessibilityIdentifier("retryViewed")
                }.appearanceText(Appearance.caption).padding(.horizontal, 20)
            }
            layout {
                control
                if !dynamicTypeSize.isAccessibilitySize { Spacer(minLength: 0) }
                progress
            }.padding(.horizontal, 20).padding(.bottom, 8)
        }.background(Appearance.surface).tint(Appearance.brand)
    }

    private var layout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 4))
            : AnyLayout(HStackLayout(spacing: 16))
    }

    @ViewBuilder private var control: some View {
        if file.tier == .trusted {
            Label("Trusted · counts as viewed", systemImage: "checkmark.shield")
                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                .frame(minHeight: 44).accessibilityIdentifier("trustedViewed")
        } else {
            Button(action: mark) {
                HStack(spacing: 8) {
                    if saving { ProgressView().accessibilityLabel("Saving viewed state") }
                    Label(file.viewed ? "Viewed" : "Mark viewed", systemImage: file.viewed ? "checkmark.circle.fill" : "circle")
                }.appearanceText(Appearance.body).frame(minHeight: 44)
            }.disabled(saving).accessibilityIdentifier("reviewViewed")
                .accessibilityValue(file.viewed ? "Viewed" : "Not viewed")
                .accessibilityHint(file.viewed ? "Mark this file as not viewed" : "Mark this file as viewed")
        }
    }

    private var progress: some View {
        Text("\(viewed) of \(total) viewed").appearanceText(Appearance.caption)
            .foregroundStyle(Appearance.secondary).accessibilityIdentifier("reviewViewedProgress")
    }
}
