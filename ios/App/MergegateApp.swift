import CodeStyling
import Conversation
import HubConnection
import Inbox
import Review
import ReviewSubmission
import SwiftUI

@main
struct MergegateApp: App {
    var body: some Scene {
        WindowGroup {
            HubConnectionBuilder.build { hub, disconnect in
                InboxBuilder.build(hub: hub, disconnect: disconnect) { item in
                    ReviewBuilder.build(
                        hub: hub, item: item, styling: NativeCodeStyling(),
                        drafts: { scope, location, close in
                            ConversationBuilder.drafts(scope: scope, location: location, close: close)
                        },
                        conversations: { scope, location, close in
                            ConversationBuilder.threads(scope: scope, location: location, close: close)
                        },
                        fileConversations: { scope, path in
                            ConversationBuilder.fileThreads(scope: scope, path: path)
                        },
                        feedback: { scope, close in
                            do {
                                return ReviewSubmissionBuilder.build(
                                    scope: scope, drafts: try FeedbackDraftStore(), close: close)
                            } catch {
                                return AnyView(
                                    VStack {
                                        Text("Couldn’t open local drafts: \(error.localizedDescription)")
                                        Button("Done", action: close)
                                    }.padding())
                            }
                        })
                }
            }
        }
    }
}
