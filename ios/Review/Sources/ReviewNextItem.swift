import Foundation
import SwiftUI

private struct ReviewQueueSnapshot: Decodable {
    struct Item: Decodable {
        let id: String
        let name: String
        let title: String
        let revision: UInt32
        let questions: UInt32
        let rejections: UInt32
        var signature: String { "\(revision)/\(questions)/\(rejections)" }
    }
    let needs_you: [Item]
}

@MainActor private enum ReviewQueue {
    static var decided: [URL: [String: String]] = [:]
    static func next(in items: [ReviewQueueSnapshot.Item], after: String, hub: URL)
        -> ReviewQueueSnapshot
        .Item?
    {
        if let own = items.first(where: { $0.id == after }) {
            decided[hub, default: [:]][after] = own.signature
        }
        if let next = items.first(where: { $0.id != after && decided[hub]?[$0.id] != $0.signature })
        {
            return next
        }
        let index = items.firstIndex(where: { $0.id == after }).map { $0 + 1 } ?? 0
        return (Array(items.dropFirst(index)) + Array(items.prefix(index))).first { $0.id != after }
    }
}

struct ReviewNextItem: View {
    let after: String
    let service: ReviewWebService
    let open: (String) -> Void
    @State private var next: ReviewQueueSnapshot.Item?
    @State private var loaded = false
    @State private var error: String?
    @State private var retry = 0
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let next {
                Button("Next: \(next.name) · \(next.title)") { open(next.id) }
                    .frame(minHeight: 44).accessibilityIdentifier("nextReview")
            } else if loaded {
                Text("Nothing else needs you").appearanceText(Appearance.caption)
            }
            if let error {
                Text(error).appearanceText(Appearance.caption).foregroundStyle(Appearance.error)
                Button("Retry next item") { retry += 1 }.frame(minHeight: 44)
            }
        }.padding(.horizontal, 20).background(Appearance.page)
            .task(id: retry) {
                do {
                    let inbox = try await JSONDecoder().decode(
                        ReviewQueueSnapshot.self,
                        from: service.transport.get(service.url.appending(path: "api/inbox")))
                    try Task.checkCancellation()
                    next = ReviewQueue.next(in: inbox.needs_you, after: after, hub: service.url)
                    loaded = true
                    error = nil
                } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
            }
    }
}
