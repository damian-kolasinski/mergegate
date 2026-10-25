import Foundation
import Networking
import Observation

@MainActor @Observable
final class ReviewHighlightModel {
    private(set) var painting: ReviewPainting?

    func refresh(service: ReviewHighlightService, item: String, revision: UInt32, file: ReviewFile)
        async
    {
        do {
            let next = try await service.load(item: item, revision: revision, file: file)
            try Task.checkCancellation()
            painting = next
        } catch { /* Highlight failures leave the source and its existing spans readable. */  }
    }

    func follow(service: ReviewHighlightService, item: String, revision: UInt32, file: ReviewFile)
        async
    {
        let stream = await service.events.subscribe(service.web.url.appending(path: "api/events/ui"))
        // Subscribe first so updates during the initial read remain queued, including on a reused stream.
        await refresh(service: service, item: item, revision: revision, file: file)
        for await event in stream {
            guard !Task.isCancelled else { return }
            switch event {
            case .connected: break
            case .disconnected: continue
            case .data(let data):
                guard let value = try? JSONDecoder().decode(Invalidation.self, from: Data(data.utf8)),
                    value.type == "highlights.changed"
                else { continue }
            }
            await refresh(service: service, item: item, revision: revision, file: file)
        }
    }

    private struct Invalidation: Decodable { let type: String }
}
