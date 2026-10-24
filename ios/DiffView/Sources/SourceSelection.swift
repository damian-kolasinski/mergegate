import Foundation
import UIKit

struct SourceSelection {
    let text: String
    let ranges: [NSRange]

    func copy(in selection: NSRange) -> String {
        let source = text as NSString
        return ranges.compactMap { range -> String? in
            let overlap = NSIntersectionRange(range, selection)
            return overlap.length > 0 ? source.substring(with: overlap) : nil
        }.joined()
    }
}

final class SourceTextView: UITextView {
    var sourceSelection = SourceSelection(text: "", ranges: [])

    override func copy(_ sender: Any?) {
        let text = sourceSelection.copy(in: selectedRange)
        if !text.isEmpty { UIPasteboard.general.string = text }
    }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(copy(_:)) {
            return !sourceSelection.copy(in: selectedRange).isEmpty
        }
        // Native share/lookup would include display gutters instead of source.
        return action == #selector(select(_:)) || action == #selector(selectAll(_:))
            ? super.canPerformAction(action, withSender: sender) : false
    }
}
