import DiffDocumentInterface
import SwiftUI

/// The last source line of a thread or draft, below which the reader shows its card.
public struct SourceMark: Hashable, Sendable {
    public let side: SourceAnchor.Side
    public let line: UInt32

    public init(side: SourceAnchor.Side, line: UInt32) {
        self.side = side
        self.line = line
    }
}

/// What a file's conversations add to the reader: where cards go, and the card for each place.
/// Review lays out the source; Conversation owns what the cards show and do.
public struct SourceAnnotations {
    public let marks: Set<SourceMark>
    /// False until the file's conversations first load, so the reader can wait to reveal a line.
    public let ready: Bool
    public let card: @MainActor (SourceMark) -> AnyView

    public init(
        marks: Set<SourceMark>, ready: Bool = true,
        card: @escaping @MainActor (SourceMark) -> AnyView
    ) {
        self.marks = marks
        self.ready = ready
        self.card = card
    }

    public static var none: SourceAnnotations {
        SourceAnnotations(marks: []) { _ in AnyView(EmptyView()) }
    }
}
