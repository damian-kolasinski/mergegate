import HubConnection
import Inbox
import SwiftUI

@main
struct MergegateApp: App {
    var body: some Scene {
        WindowGroup {
            HubConnectionBuilder.build { hub, disconnect in
                InboxBuilder.build(hub: hub, disconnect: disconnect)
            }
        }
    }
}
