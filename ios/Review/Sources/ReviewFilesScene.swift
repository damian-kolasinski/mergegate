import SwiftUI

struct ReviewFilesScene: View {
    let files: [ReviewFile]
    let selectedPath: String?
    let close: () -> Void
    let select: (String) -> Void

    var body: some View {
        List {
            Section {
                ForEach(Array(files.enumerated()), id: \.element.path) { index, file in
                    Button {
                        select(file.path)
                    } label: {
                        HStack(alignment: .top, spacing: 12) {
                            Text("\(index + 1)").appearanceText(Appearance.caption).monospacedDigit()
                                .foregroundStyle(Appearance.secondary)
                            VStack(alignment: .leading, spacing: 6) {
                                Text(file.path).appearanceText(Appearance.rowHeading)
                                Text(file.tier == .trusted ? "Trusted" : file.viewed ? "Viewed" : "To review")
                                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                                if let reason = file.rankingReason { Text(reason).appearanceText(Appearance.body) }
                            }.frame(maxWidth: .infinity, alignment: .leading)
                            if selectedPath == file.path {
                                Image(systemName: "checkmark").accessibilityLabel("Current file")
                            }
                        }.padding(.vertical, 8).frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                            .contentShape(Rectangle())
                    }.buttonStyle(.plain).accessibilityIdentifier("reviewFile-\(index)")
                        .listRowBackground(Appearance.surface)
                }
            }
        }.scrollContentBackground(.hidden).background(Appearance.page).foregroundStyle(Appearance.primary)
            .safeAreaInset(edge: .top, spacing: 0) {
                ReviewSheetHeader(title: "Files in review order", closeIdentifier: "closeReviewFiles", close: close)
            }.presentationDragIndicator(.visible)
    }
}
