import CodeStylingInterface
import DiffDocumentInterface
import SwiftUI
import UIKit

public struct UnifiedDiffView: UIViewRepresentable {
    private let hunks: [DiffHunk]
    private let styling: (any CodeStyling)?
    private let maximumLine: UInt32
    private let sourceSides: [SourceAnchor.Side]
    private let onComment: ((SourceAnchor) -> Void)?
    private let onReadConversations: (([SourceAnchor]) -> Void)?
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.colorScheme) private var colorScheme

    public init(
        hunks: [DiffHunk], maximumLine: UInt32 = 0, styling: (any CodeStyling)? = nil,
        sourceSides: [SourceAnchor.Side] = [.old, .new],
        onComment: ((SourceAnchor) -> Void)? = nil,
        onReadConversations: (([SourceAnchor]) -> Void)? = nil
    ) {
        self.sourceSides = sourceSides
        self.styling = styling
        self.hunks = hunks
        self.maximumLine = maximumLine
        self.onComment = onComment
        self.onReadConversations = onReadConversations
    }

    public func makeUIView(context: Context) -> UITextView {
        SourceTextView.reader()
    }

    public func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context)
        -> CGSize?
    {
        guard let width = proposal.width else { return nil }
        return uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
    }

    public func updateUIView(_ view: UITextView, context: Context) {
        // Environment reads refresh resolved colors and scaled fonts when the reader stays open.
        _ = dynamicTypeSize
        let traits = UITraitCollection(traitsFrom: [
            view.traitCollection,
            UITraitCollection(userInterfaceStyle: colorScheme == .dark ? .dark : .light),
        ])
        let document = DiffText.document(
            hunks, traits: traits, maximumLine: maximumLine, styling: styling)
        (view as? SourceTextView)?.install(document.text, selection: document.selection)
        (view as? SourceTextView)?.commentSelection = document.comments
        (view as? SourceTextView)?.sourceSides = sourceSides
        (view as? SourceTextView)?.onComment = onComment
        (view as? SourceTextView)?.onReadConversations = onReadConversations
    }
}
