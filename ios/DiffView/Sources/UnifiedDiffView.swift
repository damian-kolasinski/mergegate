import CodeStylingInterface
import DiffDocumentInterface
import SwiftUI
import UIKit

public struct UnifiedDiffView: UIViewRepresentable {
    private let didReveal: (() -> Void)?
    private let revealRequest: UUID?
    private let revealTopInset: CGFloat
    private let focus: SourceAnchor?
    private let hunks: [DiffHunk]
    private let styling: (any CodeStyling)?
    private let maximumLine: UInt32
    private let sourceSides: [SourceAnchor.Side]
    private let onComment: ((SourceAnchor) -> Void)?
    private let onReadConversations: (([SourceAnchor]) -> Void)?
    private let onSelectLines: ((SourceAnchor?) -> Void)?
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.colorScheme) private var colorScheme

    public init(
        hunks: [DiffHunk], maximumLine: UInt32 = 0, styling: (any CodeStyling)? = nil,
        sourceSides: [SourceAnchor.Side] = [.old, .new],
        onComment: ((SourceAnchor) -> Void)? = nil,
        onReadConversations: (([SourceAnchor]) -> Void)? = nil,
        onSelectLines: ((SourceAnchor?) -> Void)? = nil,
        focus: SourceAnchor? = nil, revealRequest: UUID? = nil, revealTopInset: CGFloat = 0, didReveal: (() -> Void)? = nil
    ) {
        self.didReveal = didReveal
        self.revealRequest = revealRequest
        self.revealTopInset = revealTopInset
        self.focus = focus
        self.sourceSides = sourceSides
        self.styling = styling
        self.hunks = hunks
        self.maximumLine = maximumLine
        self.onComment = onComment
        self.onReadConversations = onReadConversations
        self.onSelectLines = onSelectLines
    }

    /// The inputs of the last rendered document. Scrolling the review updates this view as its
    /// chrome hides and returns; rebuilding a long file's text each time stalls those frames.
    public final class Coordinator {
        fileprivate struct Inputs: Equatable {
            let hunks: [DiffHunk]
            let maximumLine: UInt32
            let dynamicTypeSize: DynamicTypeSize
            let colorScheme: ColorScheme
            let styling: ObjectIdentifier?
        }
        fileprivate var inputs: Inputs?
    }

    public func makeCoordinator() -> Coordinator { Coordinator() }

    public func makeUIView(context: Context) -> UIScrollView {
        SourceScrollView()
    }

    public func sizeThatFits(_ proposal: ProposedViewSize, uiView: UIScrollView, context: Context)
        -> CGSize?
    {
        guard let width = proposal.width else { return nil }
        return uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
    }

    public func updateUIView(_ view: UIScrollView, context: Context) {
        guard let source = (view as? SourceScrollView)?.reader else { return }
        // Environment reads refresh resolved colors and scaled fonts when the reader stays open.
        let inputs = Coordinator.Inputs(
            hunks: hunks, maximumLine: maximumLine, dynamicTypeSize: dynamicTypeSize,
            colorScheme: colorScheme, styling: styling.map { ObjectIdentifier(type(of: $0)) })
        if context.coordinator.inputs != inputs {
            let traits = UITraitCollection(traitsFrom: [
                view.traitCollection,
                UITraitCollection(userInterfaceStyle: colorScheme == .dark ? .dark : .light),
                // Fonts follow the size the cache keys on, not whatever UIKit holds yet.
                UITraitCollection(preferredContentSizeCategory: UIContentSizeCategory(dynamicTypeSize)),
            ])
            let document = DiffText.document(
                hunks, traits: traits, maximumLine: maximumLine, styling: styling)
            source.install(document.text, selection: document.selection)
            source.commentSelection = document.comments
            context.coordinator.inputs = inputs
        }
        source.sourceSides = sourceSides
        source.onComment = onComment
        source.onReadConversations = onReadConversations
        source.onSelectLines = onSelectLines
        source.didReveal = didReveal
        source.revealTopInset = revealTopInset
        if let focus,
            source.revealedAnchor != focus || source.revealedRequest != revealRequest,
            let range = source.commentSelection.range(for: focus)
        {
            source.revealedAnchor = focus
            source.revealedRequest = revealRequest
            source.selectedRange = range
            source.pendingReveal = range
            source.setNeedsLayout()
        }
    }
}
