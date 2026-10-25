import Foundation
import Networking

extension InboxViewModel {
    func follow() async {
        let stream = await events.subscribe(service.url.appending(path: "api/events/ui"))
        await refresh()
        for await event in stream {
            guard !Task.isCancelled else { return }
            await receive(event)
        }
    }

    func receive(_ event: ServerEvent) async {
        switch event {
        case .connected: isReconnecting = false
        case .disconnected:
            isReconnecting = true
            return
        case .data(let data):
            guard let event = try? JSONDecoder().decode(InboxInvalidation.self, from: Data(data.utf8)),
                ["inbox.changed", "item.changed", "rules.changed"].contains(event.type)
            else { return }
        }
        await refresh()
    }
}

private struct InboxInvalidation: Decodable { let type: String }
