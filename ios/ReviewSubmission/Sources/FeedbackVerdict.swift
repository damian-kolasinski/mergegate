enum FeedbackVerdict: String, Codable, CaseIterable, Identifiable {
    case comment
    case changesRequested = "changes_requested"

    var id: Self { self }
    var title: String {
        switch self {
        case .comment: "Send feedback"
        case .changesRequested: "Request changes"
        }
    }
}
