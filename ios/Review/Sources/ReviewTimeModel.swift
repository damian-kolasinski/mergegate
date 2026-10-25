import Foundation

@MainActor
final class ReviewTimeModel {
    private var session: UUID?
    private var revision: UInt32?
    private var last: Date?
    private var pending: TimeInterval = 0
    private let service: ReviewWebService
    private let item: String
    private var sending: Task<Void, Never>?

    init(service: ReviewWebService, item: String) {
        self.service = service
        self.item = item
    }

    @discardableResult func start(revision: UInt32, now: Date = .now) -> UUID {
        if self.revision != nil {
            input(now: now)
            _ = enqueue()
        }
        let session = UUID()
        self.session = session
        self.revision = revision
        last = now
        return session
    }
    func input(now: Date = .now) {
        guard revision != nil else { return }
        if let last {
            let gap = now.timeIntervalSince(last)
            if gap >= 0 && gap <= 120 { pending += gap }
        }
        last = now
    }
    func settle() async {
        input()
        await report()
        await sending?.value
    }

    func report() async { await enqueue()?.value }

    private func enqueue() -> Task<Void, Never>? {
        let milliseconds = UInt64(max(0, (pending * 1000).rounded()))
        pending = 0
        guard let revision, milliseconds > 0 else { return nil }
        let previous = sending
        let service = service
        let item = item
        let task = Task {
            await previous?.value
            struct Body: Encodable {
                let revision: UInt32
                let ms: UInt64
            }
            let endpoint = service.url.appending(path: "api/items").appending(component: item)
                .appending(
                    path: "review-time")
            // Telemetry must never block reviewing or make a failed report count twice.
            _ = try? await service.transport.post(
                endpoint, body: JSONEncoder().encode(Body(revision: revision, ms: milliseconds)))
        }
        sending = task
        return task
    }
    func stop(session expected: UUID? = nil, now: Date = .now) async {
        guard expected == nil || session == expected else { return }
        input(now: now)
        _ = enqueue()
        last = nil
        revision = nil
        session = nil
        await sending?.value
    }
}
