import SwiftUI

struct ReviewDemoScene: View {
    let demo: ReviewDemo
    let service: ReviewSourceService
    let close: () -> Void
    var onActivity: () -> Void = {}
    @State private var index = 0
    @State private var original: URL?
    @State private var zip: URL?
    @State private var error: String?
    @State private var downloading = false
    @State private var playing = false
    @State private var scrubbing = false
    @State private var controlsVisible = true
    @Environment(\.accessibilityVoiceOverEnabled) private var voiceOver

    var body: some View {
        ZStack {
            Appearance.page.ignoresSafeArea()
            if !demo.attachments.isEmpty {
                ReviewDemoAttachment(
                    attachment: demo.attachments[index], service: service,
                    onActivity: onActivity, onTap: toggleControls,
                    controlsVisible: showsControls,
                    onScrubbing: { scrubbing = $0; controlsVisible = true; onActivity() },
                    onPlayback: { value in
                        guard playing != value else { return }
                        playing = value
                        controlsVisible = true
                    },
                    onFile: { original = $0 }
                ).id(demo.attachments[index].id)
            } else {
                ContentUnavailableView("No demo attachments", systemImage: "photo.on.rectangle")
            }
        }
        .overlay(alignment: .top) {
            if showsControls { header.transition(.opacity) }
        }
        .overlay(alignment: .bottom) {
            if !isVideo, showsControls { footer.transition(.opacity) }
        }
        .animation(.easeInOut(duration: 0.2), value: showsControls)
        .foregroundStyle(Appearance.primary).tint(Appearance.brand)
        .statusBarHidden(!showsControls)
        .persistentSystemOverlays(showsControls ? .automatic : .hidden)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("reviewDemoViewer")
        .accessibilityAction(.escape, close)
        .onChange(of: index) { _, _ in
            original = nil
            playing = false
            scrubbing = false
            controlsVisible = true
            onActivity()
        }
        .task(id: "\(index)-\(playing)-\(controlsVisible)-\(voiceOver)-\(scrubbing)") {
            guard playing, controlsVisible, !voiceOver, !scrubbing else { return }
            do { try await Task.sleep(for: .seconds(3)) } catch { return }
            withAnimation { controlsVisible = false }
        }
    }

    private var showsControls: Bool { controlsVisible || voiceOver }

    private var isVideo: Bool { !demo.attachments.isEmpty && demo.attachments[index].video }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Demo · r\(demo.revision)").appearanceText(Appearance.label)
                Spacer(minLength: 8)
                if isVideo {
                    Menu { exports } label: {
                        Image(systemName: "ellipsis").font(.system(size: 18))
                            .frame(width: 44, height: 44).contentShape(Rectangle())
                    }.accessibilityLabel("Demo actions")
                }
                Button("Done", action: close).buttonStyle(AppearanceButtonStyle())
                    .accessibilityIdentifier("closeDemo")
            }
            if isVideo, let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
            }
            if !demo.attachments.isEmpty {
                Picker("Attachment", selection: $index) {
                    ForEach(demo.attachments.indices, id: \.self) { index in
                        Text("\(index + 1). \(demo.attachments[index].name)").tag(index)
                    }
                }.pickerStyle(.menu).appearanceText(Appearance.body)
                    .accessibilityIdentifier("demoAttachmentPicker")
            }
        }.padding(12).background(Appearance.page.opacity(0.96))
    }

    private var footer: some View {
        VStack(spacing: 8) {
            if !demo.attachments.isEmpty {
                HStack {
                    Button { index -= 1 } label: {
                        Image(systemName: "chevron.left").font(.system(size: 16, weight: .semibold))
                    }.disabled(index == 0).accessibilityLabel("Previous attachment")
                        .accessibilityIdentifier("previousDemo")
                    Spacer(minLength: 4)
                    Text("\(index + 1) of \(demo.attachments.count)").appearanceText(Appearance.caption)
                        .monospacedDigit()
                    Spacer(minLength: 4)
                    Button { index += 1 } label: {
                        Image(systemName: "chevron.right").font(.system(size: 16, weight: .semibold))
                    }.disabled(index + 1 == demo.attachments.count).accessibilityLabel("Next attachment")
                        .accessibilityIdentifier("nextDemo")
                }.buttonStyle(AppearanceButtonStyle())
            }
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) { exports }
                VStack(spacing: 8) { exports }
            }
            if let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
            }
        }.padding(12).background(Appearance.page.opacity(0.96))
    }

    @ViewBuilder private var exports: some View {
        if let original {
            ShareLink("Share original", item: original).buttonStyle(AppearanceButtonStyle())
        }
        if let zip {
            ShareLink("Share demo ZIP", item: zip).buttonStyle(AppearanceButtonStyle())
        } else {
            Button(downloading ? "Downloading…" : "Download demo ZIP") {
                Task {
                    downloading = true
                    defer { downloading = false }
                    do {
                        zip = try await service.export(service.media(path: demo.zip_url), name: "demo.zip")
                        error = nil
                    } catch { self.error = error.localizedDescription }
                }
            }.disabled(downloading).buttonStyle(AppearanceButtonStyle())
        }
    }

    private func toggleControls() {
        onActivity()
        guard !voiceOver else { return }
        controlsVisible.toggle()
    }
}
