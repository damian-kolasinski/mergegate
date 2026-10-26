import SwiftUI

// The pinned file control: step to the previous or next file, open the tree by its name, and
// read why the hub ranked it, without scrolling back to the top of a long diff.
struct ReviewFilePill<Conversations: View>: View {
    let file: ReviewFile
    let position: Int
    let count: Int
    let previous: (() -> Void)?
    let next: (() -> Void)?
    let openFiles: () -> Void
    let openInfo: () -> Void
    @ViewBuilder let conversations: () -> Conversations

    var body: some View {
        HStack(spacing: 0) {
            step("chevron.left", "Previous file", previous).accessibilityIdentifier("reviewPreviousFile")
            Button(action: openFiles) {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Image(systemName: mark).font(.caption).foregroundStyle(Appearance.brand)
                            .accessibilityHidden(true)
                        Text(name).font(.system(size: 14, design: .monospaced).weight(.semibold))
                            .lineLimit(1).truncationMode(.middle)
                    }
                    HStack(spacing: 4) {
                        Text("\(position) of \(count)")
                        if let additions = file.additions, additions > 0 {
                            Text("+\(additions)").foregroundStyle(Appearance.added)
                        }
                        if let deletions = file.deletions, deletions > 0 {
                            Text("−\(deletions)").foregroundStyle(Appearance.removed)
                        }
                        if let folder {
                            Text(folder).lineLimit(1).truncationMode(.head)
                        }
                    }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        .monospacedDigit()
                }.frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                    .contentShape(Rectangle())
            }.buttonStyle(.plain)
                .accessibilityLabel("File \(position) of \(count), \(file.path)")
                .accessibilityHint("Choose a file in review order")
                .accessibilityIdentifier("reviewFiles")
            Button(action: openInfo) {
                Image(systemName: "info.circle").frame(width: 36, height: 44)
            }.accessibilityLabel("Why this file").accessibilityIdentifier("reviewFileReasons")
            conversations()
            step("chevron.right", "Next file", next).accessibilityIdentifier("reviewNextFile")
        }.tint(Appearance.brand).padding(.horizontal, 4)
            .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 24))
    }

    private var name: String { file.path.split(separator: "/").last.map(String.init) ?? file.path }
    private var folder: String? {
        let parts = file.path.split(separator: "/")
        return parts.count > 1 ? parts.dropLast().joined(separator: "/") : nil
    }
    private var mark: String {
        if file.tier == .trusted { return "checkmark.shield" }
        return file.viewed ? "checkmark.circle.fill" : "circle"
    }

    private func step(_ symbol: String, _ label: String, _ action: (() -> Void)?) -> some View {
        Button {
            action?()
        } label: {
            Image(systemName: symbol).font(.body.weight(.semibold)).frame(width: 44, height: 44)
        }.disabled(action == nil).accessibilityLabel(label)
    }
}
