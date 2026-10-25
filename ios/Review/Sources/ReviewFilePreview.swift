import SwiftUI

extension ReviewFile {
    var previewKind: String? {
        guard interdiff != .dropped, oldMode != "160000", newMode != "160000" else { return nil }
        let ext = URL(fileURLWithPath: path).pathExtension.lowercased()
        switch content {
        case .binary where ["png", "jpg", "jpeg", "gif", "webp"].contains(ext): return "image"
        case .text where newBlob != nil && ["md", "markdown"].contains(ext): return "markdown"
        default: return nil
        }
    }
    var isLFSPointer: Bool {
        guard case .text(let hunks) = content, let first = hunks.first,
            first.newRange.lowerBound <= 1
        else { return false }
        return first.lines.contains { $0.text == "version https://git-lfs.github.com/spec/v1" }
    }
}

struct ReviewFilePreview: View {
    let file: ReviewFile
    let project: String
    let service: ReviewSourceService
    @State private var before: UIImage?
    @State private var after: UIImage?
    @State private var difference: ReviewPixelDifference?
    @State private var markdown: String?
    @State private var exports: [URL] = []
    @State private var error: String?
    @State private var retry = 0
    @State private var mode = "Before / After"
    @State private var opacity = 0.5

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let markdown { ReviewMarkdown(source: markdown) }
            if before != nil || after != nil {
                Picker("Image comparison", selection: $mode) {
                    Text("Before / After").tag("Before / After")
                    if before != nil && after != nil {
                        Text("Overlay").tag("Overlay")
                        if difference != nil { Text("Difference").tag("Difference") }
                    }
                }
                if mode == "Overlay", let before, let after {
                    ZStack {
                        image(before, label: "Before")
                        image(after, label: "After").opacity(opacity)
                    }
                    Slider(value: $opacity, in: 0...1) { Text("After image opacity") }
                } else if mode == "Difference", let difference {
                    image(difference.image, label: "Pixel difference")
                    Text("\(difference.changed) of \(difference.total) pixels changed")
                        .appearanceText(
                            Appearance.caption)
                } else {
                    if let before { image(before, label: "Before") }
                    if let after { image(after, label: "After") }
                }
            }
            if before != nil && after != nil && difference == nil {
                Text("Too large to compare pixel by pixel.").appearanceText(Appearance.caption)
            }
            ForEach(exports, id: \.self) { url in
                ShareLink("Share \(url.lastPathComponent)", item: url).frame(minHeight: 44)
            }
            if let error {
                Text(error).foregroundStyle(Appearance.error)
                Button("Retry preview") { retry += 1 }.frame(minHeight: 44)
            } else if markdown == nil && before == nil && after == nil {
                ProgressView("Loading preview…")
            }
        }.padding(20).accessibilityIdentifier("filePreview")
            .task(id: retry) { await load() }
    }

    private func image(_ value: UIImage, label: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(
                "\(label) · \(Int(value.size.width * value.scale)) × \(Int(value.size.height * value.scale))"
            )
            .appearanceText(Appearance.caption)
            Image(uiImage: value).resizable().scaledToFit().accessibilityLabel(
                "\(label): \(file.path)")
        }
    }

    private func load() async {
        do {
            exports = []
            if file.previewKind == "markdown", let blob = file.newBlob {
                let data = try await service.blob(project: project, id: blob)
                guard let text = String(data: data, encoding: .utf8) else {
                    throw URLError(.cannotDecodeContentData)
                }
                markdown = text
            } else {
                if let blob = file.oldBlob {
                    let data = try await service.blob(project: project, id: blob)
                    let image = try await ReviewImages.shared.decode(data)
                    before = image
                    exports.append(
                        try service.export(
                            data,
                            name: "before-" + URL(fileURLWithPath: file.path).lastPathComponent))
                }
                if let blob = file.newBlob {
                    let data = try await service.blob(project: project, id: blob)
                    let image = try await ReviewImages.shared.decode(data)
                    after = image
                    exports.append(
                        try service.export(
                            data, name: "after-" + URL(fileURLWithPath: file.path).lastPathComponent
                        ))
                }
                if let before, let after {
                    difference = await ReviewImages.shared.compare(before, after)
                }
            }
            error = nil
        } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
    }
}
