import SwiftUI

struct ReviewScopeBar: View {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let snapshot: ReviewSnapshot
    let loading: Bool
    let disabled: Bool
    let compare: (UInt32?) -> Void

    private var layout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(alignment: .top, spacing: 12))
    }

    var body: some View {
        layout {
            ReviewComparisonControl(
                snapshot: snapshot, loading: loading, disabled: disabled, compare: compare)
            if !dynamicTypeSize.isAccessibilitySize { Spacer(minLength: 0) }
            Text("\(snapshot.remaining) \(snapshot.remaining == 1 ? "file" : "files") left")
        }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
    }
}
