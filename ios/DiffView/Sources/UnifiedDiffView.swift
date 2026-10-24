import DiffDocumentInterface
import SwiftUI
import UIKit

public struct UnifiedDiffView: UIViewRepresentable {
    private let hunks: [DiffHunk]
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.colorScheme) private var colorScheme

    public init(hunks: [DiffHunk]) { self.hunks = hunks }

    public func makeUIView(context: Context) -> UITextView {
        let view = SourceTextView(usingTextLayoutManager: true)
        view.isScrollEnabled = false
        view.isEditable = false
        view.isSelectable = true
        view.textContainer.lineBreakMode = .byCharWrapping
        view.backgroundColor = .clear
        view.textContainerInset = UIEdgeInsets(top: 12, left: 10, bottom: 24, right: 10)
        view.textContainer.lineFragmentPadding = 0
        view.adjustsFontForContentSizeCategory = true
        view.accessibilityIdentifier = "unifiedDiff"
        view.accessibilityHint = "Select text to copy source without line numbers or diff markers."
        return view
    }

    public func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
        guard let width = proposal.width else { return nil }
        return uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
    }

    public func updateUIView(_ view: UITextView, context: Context) {
        let document = DiffText.document(hunks, traits: view.traitCollection)
        let rendered = document.text
        (view as? SourceTextView)?.sourceSelection = document.selection
        if !view.attributedText.isEqual(to: rendered) {
            let offset = view.contentOffset
            view.attributedText = rendered
            view.setContentOffset(offset, animated: false)
        }
    }
}
