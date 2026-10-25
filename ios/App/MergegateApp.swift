import CodeStyling
import Conversation
import HubConnection
import Inbox
import ProjectRules
import Review
import ReviewSubmission
import SwiftUI

@main
struct MergegateApp: App {
    var body: some Scene {
        WindowGroup {
            HubConnectionBuilder.build { hub, disconnect in
                InboxBuilder.build(
                    hub: hub, disconnect: disconnect,
                    feedback: { scope, close in
                        ReviewSubmissionBuilder.build(
                            scope: scope, drafts: { try FeedbackDraftStore() }, close: close)
                    },
                    rules: { project, openItem in
                        ProjectRulesBuilder.build(
                            hub: hub.url, project: project, openItem: openItem)
                    }
                ) { item, openItem in
                    ReviewBuilder.build(
                        hub: hub, item: item, styling: NativeCodeStyling(), openItem: openItem,
                        drafts: { scope, location, close in
                            ConversationBuilder.drafts(
                                scope: scope, location: location, close: close)
                        },
                        conversations: { scope, location, showSource, activity, close in
                            ConversationBuilder.threads(
                                scope: scope, location: location, showSource: showSource,
                                close: close)
                        },
                        fileConversations: { scope, path in
                            ConversationBuilder.fileThreads(scope: scope, path: path)
                        },
                        rules: { project, openItem in
                            ProjectRulesBuilder.build(
                                hub: hub.url, project: project, openItem: openItem)
                        },
                        feedback: { scope, settle, activity, sent, close in
                            ReviewSubmissionBuilder.build(
                                scope: scope, drafts: { try FeedbackDraftStore() },
                                beforeSend: settle,
                                onActivity: activity, onSent: sent, close: close)
                        })
                }
            }
        }
    }
}
