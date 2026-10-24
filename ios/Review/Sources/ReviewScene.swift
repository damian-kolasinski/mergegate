import DiffView
import SwiftUI

struct ReviewScene: View {
    @State var model: ReviewViewModel
    private enum Sheet: String, Identifiable {
        case guide, files
        var id: String { rawValue }
    }
    @State private var sheet: Sheet?

    var body: some View {
        Group {
            switch model.state {
            case .loading: ProgressView("Opening review…")
            case .failed(let message):
                ContentUnavailableView {
                    Label("Couldn’t open review", systemImage: "wifi.exclamationmark")
                } description: {
                    Text(message)
                } actions: {
                    Button("Retry") { Task { await model.load() } }.frame(minHeight: 44)
                }
            case .loaded(let snapshot): content(snapshot)
            }
        }
        .background(Appearance.page).foregroundStyle(Appearance.primary)
        .navigationTitle("Review").navigationBarTitleDisplayMode(.inline)
        .task { await model.load() }
    }

    private func content(_ snapshot: ReviewSnapshot) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(snapshot.item.title).appearanceText(Appearance.heading)
                            .accessibilityAddTraits(.isHeader)
                        HStack(alignment: .top, spacing: 12) {
                            Text("All changes → \(snapshot.item.target)")
                            Spacer(minLength: 0)
                            Text("\(snapshot.remaining) files left")
                        }.appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                    }.padding(.horizontal, 20).padding(.vertical, 12).id("reviewTop")
                    Divider()
                    if let index = snapshot.files.firstIndex(where: { $0.path == model.selectedPath }) {
                        let file = snapshot.files[index]
                        ReviewFilePicker(file: file, position: index + 1, count: snapshot.files.count) {
                            sheet = .files
                        }
                        Divider()
                        ReviewFileReader(file: file, project: snapshot.item.project, sourceService: model.sourceService).id(file.path)
                    } else {
                        ContentUnavailableView(
                            "No changed files", systemImage: "doc",
                            description: Text("This item has no files to display."))
                    }
                }
            }
            .onChange(of: model.selectedPath) { _, _ in proxy.scrollTo("reviewTop", anchor: .top) }
        }
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text("\(snapshot.item.name) · r\(snapshot.item.revision.number)")
                    .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary).lineLimit(1)
                    .accessibilityLabel(
                        "\(snapshot.item.name) · r\(snapshot.item.revision.number) → \(snapshot.item.target)"
                    )
                    .accessibilityIdentifier("reviewIdentity")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    sheet = .guide
                } label: {
                    Label("Review guide", systemImage: "book")
                }.tint(Appearance.brand).accessibilityIdentifier("reviewGuide")
            }
        }
        .sheet(item: $sheet) { selection in
            switch selection {
            case .guide: ReviewGuideScene(guide: snapshot.guide) { sheet = nil }
            case .files:
                ReviewFilesScene(files: snapshot.files, selectedPath: model.selectedPath, close: { sheet = nil }) {
                    path in
                    model.selectedPath = path
                    sheet = nil
                }
            }
        }
    }
}
