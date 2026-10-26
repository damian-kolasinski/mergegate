// The outcome the reader opens the completion sheet with, so one tap there finishes the review.
public enum ReviewOutcome: Sendable, Equatable {
    case comment
    case changesRequested
    case approved
}
