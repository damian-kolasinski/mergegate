import AVFoundation
import Combine
import SwiftUI
import UIKit

struct ReviewDemoVideo: View {
    let player: AVPlayer
    let controlsVisible: Bool
    let onTap: () -> Void
    let onScrubbing: (Bool) -> Void
    @State private var playing = false
    @State private var position: Double = 0
    @State private var duration: Double = 0
    @State private var scrubbing = false

    var body: some View {
        ZStack {
            ReviewDemoPlayerSurface(player: player).ignoresSafeArea()
                .contentShape(Rectangle()).onTapGesture(perform: onTap)
        }
            .overlay(alignment: .bottom) {
                if controlsVisible {
                    HStack(spacing: 12) {
                        Button {
                            if playing {
                                player.pause()
                            } else {
                                if let item = player.currentItem, item.duration.isNumeric,
                                    CMTimeCompare(player.currentTime(), item.duration) >= 0 {
                                    player.seek(to: .zero)
                                }
                                player.play()
                            }
                        } label: {
                            Image(systemName: playing ? "pause.fill" : "play.fill")
                                .font(.system(size: 22)).frame(width: 44, height: 44)
                                .contentShape(Rectangle())
                        }.buttonStyle(.plain).foregroundStyle(Appearance.brand)
                            .accessibilityLabel(playing ? "Pause" : "Play")
                            .accessibilityIdentifier("demoPlayback")
                        VStack(spacing: 4) {
                            Slider(value: $position, in: 0...max(1, duration)) { editing in
                                scrubbing = editing
                                onScrubbing(editing)
                                if !editing {
                                    player.seek(to: CMTime(seconds: position, preferredTimescale: 600))
                                }
                            }.disabled(duration <= 0).accessibilityLabel("Video position")
                            HStack {
                                Text(time(position))
                                Spacer()
                                Text(time(duration))
                            }.appearanceText(Appearance.caption).monospacedDigit()
                        }
                    }.padding(12).background(Appearance.page.opacity(0.96))
                        .transition(.opacity)
                }
            }
            .animation(.easeInOut(duration: 0.2), value: controlsVisible)
            .onReceive(player.publisher(for: \.timeControlStatus).receive(on: RunLoop.main)) {
                let value = $0 == .playing
                if playing != value { playing = value }
            }
            .onReceive(Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()) { _ in
                if let value = player.currentItem?.duration.seconds, value.isFinite, value > 0 {
                    duration = value
                }
                let value = player.currentTime().seconds
                if !scrubbing, value.isFinite { position = min(duration, max(0, value)) }
            }
    }

    private func time(_ seconds: Double) -> String {
        let value = Int(seconds)
        return String(format: "%d:%02d", value / 60, value % 60)
    }
}

private struct ReviewDemoPlayerSurface: UIViewRepresentable {
    let player: AVPlayer

    func makeUIView(context: Context) -> Surface {
        let view = Surface()
        view.backgroundColor = .black
        view.playerLayer.videoGravity = .resizeAspect
        return view
    }

    func updateUIView(_ view: Surface, context: Context) { view.playerLayer.player = player }

    final class Surface: UIView {
        override class var layerClass: AnyClass { AVPlayerLayer.self }
        var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }
    }
}
