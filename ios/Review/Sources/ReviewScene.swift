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
                        Text("\(snapshot.item.name) · r\(snapshot.item.revision.number) → \(snapshot.item.target)")
                            .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                            .accessibilityIdentifier("reviewIdentity")
                        HStack(alignment: .top) {
                            Text("All changes · \(snapshot.remaining) review files remaining")
                                .appearanceText(Appearance.caption).foregroundStyle(Appearance.secondary)
                            Spacer(minLength: 0)
                        }
                        HStack(spacing: 12) {
                            Button {
                                sheet = .guide
                            } label: {
                                Label("Guide", systemImage: "text.alignleft").frame(minHeight: 44)
                            }
                            .accessibilityIdentifier("reviewGuide")
                            Spacer()
                            Button {
                                sheet = .files
                            } label: {
                                Label("Files (\(snapshot.files.count))", systemImage: "list.bullet").frame(
                                    minHeight: 44)
                            }
                            .accessibilityIdentifier("reviewFiles")
                        }.buttonStyle(.glass).tint(Appearance.brand).frame(minHeight: 44)
                    }.padding(16).id("reviewTop")
                    Divider()
                    if let file = snapshot.files.first(where: { $0.path == model.selectedPath }) {
                        ReviewFileReader(file: file).id(file.path)
                    } else {
                        ContentUnavailableView(
                            "No changed files", systemImage: "doc",
                            description: Text("This item has no files to display."))
                    }
                }
            }
            .onChange(of: model.selectedPath) { _, _ in proxy.scrollTo("reviewTop", anchor: .top) }
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
