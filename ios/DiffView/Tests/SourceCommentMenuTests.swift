import DiffDocumentInterface
import UIKit
import Testing

@testable import DiffView

struct SourceCommentMenuTests {
    @Test @MainActor func interdiffBeforeSelectionKeepsCopyWithoutWrongRevisionActions() {
        let view = SourceTextView.reader()
        view.sourceSides = [.new]
        view.commentSelection = .init(rows: [
            .init(range: NSRange(location: 0, length: 8), line: .init(
                kind: .removed, oldLine: 3, newLine: nil, text: "old fee", hasNewline: true)),
            .init(range: NSRange(location: 8, length: 8), line: .init(
                kind: .added, oldLine: nil, newLine: 4, text: "new fee", hasNewline: true)),
        ])
        view.onComment = { _ in }
        view.onReadConversations = { _ in }
        let copy = UIAction(title: "Copy") { _ in }
        func titles(_ range: NSRange) -> [String] {
            view.textView(view, editMenuForTextIn: range, suggestedActions: [copy])!
                .children.compactMap { ($0 as? UIAction)?.title }
        }
        #expect(titles(NSRange(location: 0, length: 7)) == ["Copy"])
        #expect(titles(NSRange(location: 8, length: 7)) == ["Comment", "Conversations", "Copy"])
        view.sourceSides = []
        #expect(titles(NSRange(location: 8, length: 7)) == ["Copy"])
        view.sourceSides = [.old, .new]
        #expect(titles(NSRange(location: 0, length: 7)) == ["Comment", "Conversations", "Copy"])
    }
}
