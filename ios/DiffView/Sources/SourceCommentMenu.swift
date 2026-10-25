import UIKit

extension SourceTextView: UITextViewDelegate {
    func textView(
        _ textView: UITextView, editMenuForTextIn range: NSRange,
        suggestedActions: [UIMenuElement]
    ) -> UIMenu? {
        guard let onComment, let anchor = commentSelection.anchor(in: range) else {
            return UIMenu(children: suggestedActions)
        }
        let comment = UIAction(title: "Comment", image: UIImage(systemName: "text.bubble")) { _ in
            onComment(anchor)
        }
        return UIMenu(children: [comment] + suggestedActions)
    }
}
