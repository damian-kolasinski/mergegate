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
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        Group {
            if dynamicTypeSize.isAccessibilitySize {
                VStack(spacing: 0) {
                    fileControl.padding(.horizontal, 12)
                    HStack(spacing: 0) {
                        previousControl
                        infoControl
                        conversations()
                        Spacer(minLength: 0)
                        nextControl
                    }
                }
            } else {
                HStack(spacing: 0) {
                    previousControl
                    fileControl
                    infoControl
                    conversations()
                    nextControl
                }
            }
        }.tint(Appearance.brand).padding(.horizontal, 4)
            .background(Appearance.page, in: RoundedRectangle(cornerRadius: Appearance.controlRadius))
            .overlay(RoundedRectangle(cornerRadius: Appearance.controlRadius).strokeBorder(Appearance.border))
    }

    private var previousControl: some View {
        step("chevron.left", "Previous file", previous).accessibilityIdentifier("reviewPreviousFile")
    }

    private var nextControl: some View {
        step("chevron.right", "Next file", next).accessibilityIdentifier("reviewNextFile")
    }

    private var infoControl: some View {
        Button(action: openInfo) {
            Image(systemName: "info.circle").font(.system(size: 18))
                .frame(width: 44, height: 44).contentShape(Rectangle())
        }.accessibilityLabel("Why this file").accessibilityIdentifier("reviewFileReasons")
    }

    private var fileControl: some View {
        Button(action: openFiles) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Image(systemName: mark).font(.system(size: 12)).foregroundStyle(Appearance.brand)
                        .accessibilityHidden(true)
                    Text(name).appearanceMono(Appearance.body).fontWeight(.semibold)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? 2 : 1).truncationMode(.middle)
                }
                ReviewChips(spacing: 4) {
                    Text("\(position) of \(count)")
                    if let additions = file.additions, additions > 0 {
                        Text("+\(additions)").foregroundStyle(Appearance.added)
                    }
                    if let deletions = file.deletions, deletions > 0 {
                        Text("−\(deletions)").foregroundStyle(Appearance.removed)
                    }
                    if !dynamicTypeSize.isAccessibilitySize, let folder {
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
            Image(systemName: symbol).font(.system(size: 16, weight: .semibold)).frame(width: 44, height: 44).contentShape(Rectangle())
        }.disabled(action == nil).accessibilityLabel(label)
    }
}
