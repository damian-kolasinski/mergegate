import AVKit
import Combine
import SwiftUI

struct ReviewDemoAttachment: View {
    let attachment: ReviewDemo.Attachment
    let service: ReviewSourceService
    let onActivity: () -> Void
    let onTap: () -> Void
    let controlsVisible: Bool
    let onScrubbing: (Bool) -> Void
    let onPlayback: (Bool) -> Void
    let onFile: (URL) -> Void
    @Environment(\.scenePhase) private var phase
    @State private var playbackCredit: Double = 0
    @State private var image: UIImage?
    @State private var player: AVPlayer?
    @State private var error: String?
    @State private var retry = 0

    var body: some View {
        Group {
            if let image {
                ReviewDemoImage(image: image, name: attachment.name, onTap: onTap, onActivity: onActivity)
            } else if let player {
                ReviewDemoVideo(player: player, controlsVisible: controlsVisible, onTap: onTap, onScrubbing: onScrubbing)
                    .onReceive(player.publisher(for: \.timeControlStatus).receive(on: RunLoop.main)) {
                        onPlayback($0 == .playing)
                    }
            } else if let error {
                VStack(spacing: 12) {
                    Text(error).appearanceText(Appearance.body).foregroundStyle(Appearance.error)
                    Button("Retry attachment") { retry += 1 }.buttonStyle(AppearanceButtonStyle())
                }.padding(20)
            } else {
                ProgressView("Loading attachment…")
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
            .task(id: retry) {
                do {
                    let data = try await service.media(path: attachment.url)
                    try Task.checkCancellation()
                    let saved = try service.export(data, name: attachment.name)
                    if attachment.video {
                        player = AVPlayer(url: saved)
                    } else {
                        image = try await ReviewImages.shared.decode(data)
                        try Task.checkCancellation()
                    }
                    onFile(saved)
                    error = nil
                } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
            }
            .onDisappear { player?.pause(); onPlayback(false) }
            .onChange(of: phase) { _, value in if value != .active { player?.pause() } }
            .onReceive(Timer.publish(every: 1, on: .main, in: .common).autoconnect()) { _ in
                if let player, player.currentItem?.status == .failed {
                    error = player.currentItem?.error?.localizedDescription ?? "This video could not be played."
                    player.pause()
                    self.player = nil
                    onPlayback(false)
                    return
                }
                guard phase == .active, let player, player.rate > 0,
                    let duration = player.currentItem?.duration.seconds, duration.isFinite,
                    playbackCredit < duration
                else { return }
                playbackCredit += 1
                onActivity()
            }
    }
}
