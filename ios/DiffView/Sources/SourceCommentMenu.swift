import UIKit

extension SourceTextView: UITextViewDelegate {
    func textView(
        _ textView: UITextView, editMenuForTextIn range: NSRange,
        suggestedActions: [UIMenuElement]
    ) -> UIMenu? {
        var actions: [UIMenuElement] = []
        if let onComment, let anchor = commentSelection.anchor(in: range), sourceSides.contains(anchor.side) {
            actions.append(
                UIAction(title: "Comment", image: UIImage(systemName: "text.bubble")) { _ in
                    onComment(anchor)
                })
        }
        let anchors = commentSelection.conversationAnchors(in: range).filter { sourceSides.contains($0.side) }
        if let onReadConversations, !anchors.isEmpty {
            actions.append(
                UIAction(title: "Conversations", image: UIImage(systemName: "bubble.left.and.bubble.right")) {
                    _ in
                    onReadConversations(anchors)
                })
        }
        return UIMenu(children: actions + suggestedActions)
    }

    func textViewDidChangeSelection(_ textView: UITextView) {
        // Only the operator's own selecting counts; restoring or revealing a range sets it too.
        if isFirstResponder { reportSelection() }
    }
}
