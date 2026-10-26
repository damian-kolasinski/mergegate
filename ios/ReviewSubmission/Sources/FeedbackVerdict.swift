import ConversationInterface

enum FeedbackVerdict: String, Codable, CaseIterable, Identifiable {
    case comment
    case approved
    case changesRequested = "changes_requested"

    var id: Self { self }

    init(_ outcome: ReviewOutcome) {
        switch outcome {
        case .comment: self = .comment
        case .changesRequested: self = .changesRequested
        case .approved: self = .approved
        }
    }
    var title: String {
        switch self {
        case .approved: "Approve & merge"
        case .comment: "Send feedback"
        case .changesRequested: "Request changes"
        }
    }
}
