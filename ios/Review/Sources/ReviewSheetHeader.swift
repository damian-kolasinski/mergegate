import SwiftUI

struct ReviewSheetHeader: View {
    let title: String
    let closeIdentifier: String
    let close: () -> Void

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).appearanceText(Appearance.heading).accessibilityAddTraits(.isHeader)
            Spacer(minLength: 12)
            Button(action: close) {
                Text("Done").appearanceText(Appearance.label).frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }.buttonStyle(.glass).tint(Appearance.brand).accessibilityIdentifier(closeIdentifier)
        }.padding(.horizontal, 20).padding(.top, 12).background(Appearance.page)
    }
}
