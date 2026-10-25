import Foundation
import Networking

extension ConversationViewModel {
    func follow() async {
        let stream = await events.subscribe(scope.hub.appending(path: "api/events/ui"))
        await load()
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
            guard let event = try? JSONDecoder().decode(ConversationInvalidation.self, from: Data(data.utf8)),
                event.type == "inbox.changed" || (event.type == "item.changed" && event.item == scope.item)
            else { return }
        }
        await load()
    }
}

private struct ConversationInvalidation: Decodable {
    let type: String
    let item: String?
}
