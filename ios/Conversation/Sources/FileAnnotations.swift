import ConversationInterface
import DiffDocumentInterface
import Networking
import SwiftUI

// Loads one file's sent threads, hub drafts and local drafts, and hands the reader where their
// cards go and the cards themselves, so they sit under the lines they are about, as on the web.
struct FileAnnotations<Content: View>: View {
    let scope: CommentDraftScope
    let path: String
    /// Changes when local drafts may have changed, as after the drafts sheet closes.
    let refresh: Int
    @ViewBuilder let content: (SourceAnnotations) -> Content
    // Made once per file rather than on every pass of the reader that builds this view.
    @State private var model: ConversationViewModel?
    @State private var drafts: [CommentDraft] = []
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        content(annotations)
            .task(id: refresh) { loadDrafts() }
            .task(id: scenePhase == .active) {
                guard scenePhase == .active else { return }
                let model = model ?? ConversationViewModel(
                    scope: scope, path: path, service: .init(transport: URLSessionTransport()))
                self.model = model
                await model.follow()
            }
    }

    private var annotations: SourceAnnotations {
        guard let model else { return SourceAnnotations(marks: [], ready: false) { _ in AnyView(EmptyView()) } }
        let threads = model.threads
        // Hub drafts may come from an earlier revision; place them as their threads would be.
        let hubDrafts = (model.snapshot?.drafts ?? []).filter { $0.filePath == path }.map {
            ConversationThread(comment: $0, replies: [], revision: scope.revision)
        }
        var marks = Set(threads.compactMap(\.mark))
        marks.formUnion(hubDrafts.compactMap(\.mark))
        marks.formUnion(drafts.map(\.mark))
        return SourceAnnotations(
            marks: marks, ready: model.snapshot != nil || model.error != nil
        ) { mark in
            AnyView(
                InlineCards(
                    model: model, threads: threads.filter { $0.mark == mark },
                    hubDrafts: hubDrafts.filter { $0.mark == mark }.map(\.comment),
                    drafts: drafts.filter { $0.mark == mark }
                ) { loadDrafts() })
        }
    }

    private func loadDrafts() {
        // A store failure leaves the cards without local drafts; the drafts sheet reports it.
        drafts = ((try? CommentDraftStore().load(scope: scope)) ?? []).filter {
            $0.location.path == path
        }
    }
}

extension ConversationThread {
    /// Where the thread sits on the displayed revision; none when it isn't on this diff.
    var mark: SourceMark? {
        guard let range = sourceRange, let side = comment.side.flatMap(SourceAnchor.Side.init)
        else { return nil }
        return SourceMark(side: side, line: range.upperBound)
    }
}

extension CommentDraft {
    var mark: SourceMark { SourceMark(side: location.anchor.side, line: location.anchor.endLine) }
}
