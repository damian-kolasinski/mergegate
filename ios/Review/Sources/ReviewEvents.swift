import Foundation
import Networking

extension ReviewViewModel {
    func follow() async {
        let stream = await events.subscribe(service.url.appending(path: "api/events/ui"))
        await refreshCurrent()
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
            guard let event = try? JSONDecoder().decode(ReviewInvalidation.self, from: Data(data.utf8)),
                event.type == "inbox.changed" || (event.type == "item.changed" && event.item == item)
            else { return }
        }
        await refreshCurrent()
    }
}

private struct ReviewInvalidation: Decodable {
    let type: String
    let item: String?
}
