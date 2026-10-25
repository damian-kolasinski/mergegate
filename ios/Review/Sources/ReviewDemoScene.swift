import AVKit
import Combine
import SwiftUI

struct ReviewDemoScene: View {
    let demo: ReviewDemo
    let service: ReviewSourceService
    let close: () -> Void
    var onActivity: () -> Void = {}
    @State private var index = 0
    @State private var zip: URL?
    @State private var error: String?
    @State private var downloading = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 12) {
                if !demo.attachments.isEmpty {
                    Picker("Attachment", selection: $index) {
                        ForEach(demo.attachments.indices, id: \.self) { index in
                            Text("\(index + 1). \(demo.attachments[index].name)").tag(index)
                        }
                    }.padding(.horizontal, 20)
                    ReviewAttachmentView(
                        attachment: demo.attachments[index], service: service,
                        onActivity: onActivity
                    )
                    .id(demo.attachments[index].id)
                    HStack {
                        Button("Previous", systemImage: "chevron.left") { index -= 1 }.disabled(
                            index == 0)
                        Spacer()
                        Text("\(index + 1) of \(demo.attachments.count)").appearanceText(
                            Appearance.caption)
                        Spacer()
                        Button("Next", systemImage: "chevron.right") { index += 1 }.disabled(
                            index + 1 == demo.attachments.count)
                    }.frame(minHeight: 44).padding(.horizontal, 20)
                }
                if let error { Text(error).foregroundStyle(Appearance.error).padding() }
                if let zip {
                    ShareLink("Share demo ZIP", item: zip).frame(minHeight: 44)
                } else {
                    Button(downloading ? "Downloading…" : "Download current demo ZIP") {
                        Task {
                            downloading = true
                            defer { downloading = false }
                            do {
                                zip = try await service.export(
                                    service.media(path: demo.zip_url), name: "demo.zip")
                                error = nil
                            } catch { self.error = error.localizedDescription }
                        }
                    }.disabled(downloading).frame(minHeight: 44)
                }
            }.background(Appearance.page).foregroundStyle(Appearance.primary)
                .navigationTitle("Demo · r\(demo.revision)").navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    Button("Done", action: close).appearanceText(Appearance.label).frame(
                        minWidth: 44, minHeight: 44)
                }
        }.accessibilityIdentifier("reviewDemoViewer")
    }
}

private struct ReviewAttachmentView: View {
    let attachment: ReviewDemo.Attachment
    let service: ReviewSourceService
    let onActivity: () -> Void
    @Environment(\.scenePhase) private var phase
    @State private var playbackCredit: Double = 0
    @State private var image: UIImage?
    @State private var player: AVPlayer?
    @State private var file: URL?
    @State private var error: String?
    @State private var retry = 0
    var body: some View {
        VStack(spacing: 8) {
            Text(attachment.name).appearanceText(Appearance.rowHeading)
            if let image {
                ScrollView([.horizontal, .vertical]) {
                    Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: 900)
                        .accessibilityLabel(attachment.name)
                }
                Text(
                    "\(Int(image.size.width * image.scale)) × \(Int(image.size.height * image.scale)) pixels"
                )
                .appearanceText(Appearance.caption)
            } else if let player {
                VideoPlayer(player: player)
            } else if let error {
                Text(error).foregroundStyle(Appearance.error)
                Button("Retry attachment") { retry += 1 }.frame(minHeight: 44)
            } else {
                ProgressView("Loading attachment…").frame(maxHeight: .infinity)
            }
            if let file { ShareLink("Share original", item: file).frame(minHeight: 44) }
        }.frame(maxHeight: .infinity)
            .task(id: retry) {
                do {
                    let data = try await service.media(path: attachment.url)
                    try Task.checkCancellation()
                    let saved = try service.export(data, name: attachment.name)
                    file = saved
                    if attachment.video {
                        player = AVPlayer(url: saved)
                    } else {
                        image = try await ReviewImages.shared.decode(data)
                    }
                    error = nil
                } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
            }.onDisappear { player?.pause() }
            .onChange(of: phase) { _, value in if value != .active { player?.pause() } }
            .onReceive(Timer.publish(every: 1, on: .main, in: .common).autoconnect()) { _ in
                guard phase == .active, let player, player.rate > 0,
                    let duration = player.currentItem?.duration.seconds, duration.isFinite,
                    playbackCredit < duration
                else { return }
                playbackCredit += 1
                onActivity()
            }
    }
}
