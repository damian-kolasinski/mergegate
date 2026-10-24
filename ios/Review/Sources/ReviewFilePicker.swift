import SwiftUI

struct ReviewFilePicker: View {
    let file: ReviewFile
    let position: Int
    let count: Int
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("File \(position) of \(count)").appearanceText(Appearance.caption)
                        .foregroundStyle(Appearance.secondary)
                    Text(file.path).font(.system(.subheadline, design: .monospaced).weight(.semibold))
                }.frame(maxWidth: .infinity, alignment: .leading)
                Image(systemName: "list.bullet").foregroundStyle(Appearance.brand)
            }.padding(.horizontal, 20).padding(.vertical, 12)
                .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading).contentShape(Rectangle())
        }.buttonStyle(.plain).background(Appearance.surface)
            .accessibilityLabel("File \(position) of \(count), \(file.path)")
            .accessibilityHint("Choose a file in review order")
            .accessibilityIdentifier("reviewFiles")
    }
}
