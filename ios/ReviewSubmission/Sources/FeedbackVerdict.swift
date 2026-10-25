enum FeedbackVerdict: String, Codable, CaseIterable, Identifiable {
    case comment
    case approved
    case changesRequested = "changes_requested"

    var id: Self { self }
    var title: String {
        switch self {
        case .approved: "Approve & merge"
        case .comment: "Send feedback"
        case .changesRequested: "Request changes"
        }
    }
}
