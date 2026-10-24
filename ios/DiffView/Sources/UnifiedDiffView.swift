import DiffDocumentInterface
import SwiftUI
import UIKit

public struct UnifiedDiffView: UIViewRepresentable {
    private let hunks: [DiffHunk]
    private let maximumLine: UInt32
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.colorScheme) private var colorScheme

    public init(hunks: [DiffHunk], maximumLine: UInt32 = 0) {
        self.hunks = hunks
        self.maximumLine = maximumLine
    }

    public func makeUIView(context: Context) -> UITextView {
        SourceTextView.reader()
    }

    public func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
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
        let document = DiffText.document(hunks, traits: traits, maximumLine: maximumLine)
        let rendered = document.text
        (view as? SourceTextView)?.sourceSelection = document.selection
        if !view.attributedText.isEqual(to: rendered) {
            let offset = view.contentOffset
            view.attributedText = rendered
            view.setContentOffset(offset, animated: false)
        }
    }
}
