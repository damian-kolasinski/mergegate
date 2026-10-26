import SwiftUI

// The web's file list (ui/item/files.tsx): files to review as a tree, then the ones unchanged
// since the compared revision and the trusted ones, each folded into a group of its own.
struct ReviewFilesScene: View {
    let snapshot: ReviewSnapshot
    let selectedPath: String?
    let close: () -> Void
    let select: (String) -> Void
    @State private var folded: Set<String> = []
    @State private var showingUnchanged = false
    @State private var showingTrusted = false

    var body: some View {
        let order = ReviewReadingOrder(snapshot)
        List {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text("\(snapshot.reviewCount - snapshot.remaining) of \(snapshot.reviewCount) reviewed")
                        .appearanceText(Appearance.label)
                    ProgressView(
                        value: Double(snapshot.reviewCount - snapshot.remaining),
                        total: Double(max(1, snapshot.reviewCount))
                    ).tint(Appearance.brand)
                }.listRowBackground(Appearance.page).accessibilityElement(children: .combine)
            }
            Section {
                rows(ReviewFileTree.build(order.review, prefix: "r:"), depth: 0)
            }
            if !order.unchanged.isEmpty {
                Section {
                    DisclosureGroup(isExpanded: $showingUnchanged) {
                        ForEach(order.unchanged, id: \.path) { file in fileRow(file, depth: 0, test: false) }
                    } label: {
                        Text("\(order.unchanged.count) unchanged since r\(snapshot.since ?? 0)")
                            .appearanceText(Appearance.label).frame(minHeight: 44)
                            .accessibilityIdentifier("reviewUnchangedFiles")
                    }.listRowBackground(Appearance.page)
                }
            }
            if !order.trusted.isEmpty {
                Section {
                    DisclosureGroup(isExpanded: $showingTrusted) {
                        rows(ReviewFileTree.build(order.trusted, prefix: "t:"), depth: 0)
                    } label: {
                        // On the label: a group's identifier would replace its rows'.
                        Text("Trusted (\(order.trusted.count))").appearanceText(Appearance.label)
                            .frame(minHeight: 44).accessibilityIdentifier("reviewTrustedFiles")
                    }.listRowBackground(Appearance.page)
                }
            }
        }
        .listStyle(.plain).environment(\.defaultMinListRowHeight, 0)
        .scrollContentBackground(.hidden).background(Appearance.page)
        .foregroundStyle(Appearance.primary)
        .safeAreaInset(edge: .top, spacing: 0) {
            ReviewSheetHeader(title: "Files", closeIdentifier: "closeReviewFiles", close: close)
        }
        .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
        .onAppear {
            // Opening on the trusted or unchanged file being read shows it.
            showingTrusted = order.trusted.contains { $0.path == selectedPath }
            showingUnchanged = order.unchanged.contains { $0.path == selectedPath }
        }
    }

    private func rows(_ nodes: [ReviewTreeNode], depth: Int) -> AnyView {
        AnyView(
            ForEach(nodes) { node in
                switch node {
                case .group(let group):
                    groupRow(group, depth: depth)
                    if !folded.contains(group.key) { rows(group.nodes, depth: depth + 1) }
                case .file(let leaf):
                    fileRow(leaf.file, depth: depth, test: false, tag: leaf.tag)
                    ForEach(leaf.tests, id: \.path) { test in fileRow(test, depth: depth + 1, test: true) }
                }
            })
    }

    private func groupRow(_ group: ReviewTreeGroup, depth: Int) -> some View {
        Button {
            if folded.contains(group.key) { folded.remove(group.key) } else { folded.insert(group.key) }
        } label: {
            HStack(spacing: 6) {
                Image(systemName: folded.contains(group.key) ? "chevron.right" : "chevron.down")
                    .font(.caption.weight(.semibold)).foregroundStyle(Appearance.secondary)
                    .frame(width: 12)
                VStack(alignment: .leading, spacing: 1) {
                    Text(group.kind == .folder ? "\(group.name)/" : group.name)
                        .font(.system(size: 13, design: .monospaced).weight(.semibold))
                        .lineLimit(1).truncationMode(.middle)
                    if !group.sub.isEmpty {
                        Text(group.sub).appearanceText(Appearance.caption)
                            .foregroundStyle(Appearance.secondary).lineLimit(1).truncationMode(.head)
                    }
                }
                Spacer(minLength: 6)
                size(group.additions, group.deletions)
                score(group.peak)
            }.padding(.leading, CGFloat(depth) * 14).frame(minHeight: 36).contentShape(Rectangle())
        }.buttonStyle(.plain).listRowBackground(Appearance.page)
            .accessibilityLabel("\(group.name), \(folded.contains(group.key) ? "folded" : "open")")
    }

    private func fileRow(_ file: ReviewFile, depth: Int, test: Bool, tag: String? = nil) -> some View {
        let name = file.path.split(separator: "/").last.map(String.init) ?? file.path
        let dot = name.lastIndex(of: ".").flatMap { $0 > name.startIndex ? $0 : nil }
        return Button {
            select(file.path)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: file.tier == .trusted
                    ? "checkmark.shield" : file.viewed ? "checkmark.circle.fill" : "circle")
                    .font(.caption).foregroundStyle(file.viewed || file.tier == .trusted
                        ? Appearance.brand : Appearance.secondary)
                    .frame(width: 12)
                    .accessibilityLabel(file.tier == .trusted ? "Trusted" : file.viewed ? "Viewed" : "To review")
                HStack(spacing: 0) {
                    if test { Text("↳ ").foregroundStyle(Appearance.secondary) }
                    Text(dot.map { String(name[..<$0]) } ?? name)
                    if let dot { Text(name[dot...]).foregroundStyle(Appearance.secondary) }
                }.font(.system(size: 13, design: .monospaced)).lineLimit(1).truncationMode(.middle)
                if let tag { ReviewBadge(label: tag, tone: .neutral) }
                if let status = status(file) {
                    Text(status).appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 6)
                size(file.additions ?? 0, file.deletions ?? 0)
                score(file.importance)
            }.padding(.leading, CGFloat(depth) * 14).frame(minHeight: 40).contentShape(Rectangle())
        }.buttonStyle(.plain)
            .listRowBackground(file.path == selectedPath ? Appearance.tintBrand : Appearance.page)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(
                [file.path, file.tier == .trusted ? "Trusted" : file.viewed ? "Viewed" : "To review",
                 tag, status(file), file.importance.map { "Judge \($0)" }]
                    .compactMap { $0 }.joined(separator: ", "))
            .accessibilityAddTraits(file.path == selectedPath ? [.isButton, .isSelected] : .isButton)
            .accessibilityIdentifier(
                "reviewFile-\(snapshot.files.firstIndex { $0.path == file.path } ?? 0)")
    }

    private func status(_ file: ReviewFile) -> String? {
        switch file.interdiff {
        case .dropped: return "dropped"
        case .unseparated: return "whole diff"
        case .unchanged: return snapshot.since.map { "unchanged since r\($0)" }
        case .added: return "added"
        case .deleted: return "deleted"
        case .changed, nil:
            return file.change == "Added" ? "added" : file.change == "Deleted" ? "deleted" : nil
        }
    }

    private func size(_ additions: UInt64, _ deletions: UInt64) -> some View {
        HStack(spacing: 4) {
            if additions > 0 { Text("+\(additions)").foregroundStyle(Appearance.added) }
            if deletions > 0 { Text("−\(deletions)").foregroundStyle(Appearance.removed) }
        }.appearanceText(Appearance.caption).monospacedDigit()
    }

    // The web's judge chip: 70 and up stands out, 40 and up asks for a look.
    @ViewBuilder private func score(_ importance: UInt8?) -> some View {
        if let importance {
            ReviewBadge(
                label: "\(importance)",
                tone: importance >= 70 ? .warning : importance >= 40 ? .brand : .neutral
            ).accessibilityLabel("Judge \(importance) of 100")
        }
    }
}
