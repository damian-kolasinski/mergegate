import CodeStyling
import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct HighlightUpdateTests {
    @Test @MainActor func lateStylingPreservesNativeSelectionAndSourceOnlyCopy() throws {
        let hunks = [
            DiffHunk(
                header: "@@ -1,1 +1,1 @@",
                lines: [
                    DiffLine(
                        kind: .added, oldLine: nil, newLine: 1, text: "let value = \"é 🐙\"",
                        spans: [CodeSpan(start: 0, length: 3, tag: "keyword")])
                ])
        ]
        let traits = UITraitCollection(userInterfaceStyle: .light)
        let plain = DiffText.document(hunks, traits: traits)
        let styled = DiffText.document(hunks, traits: traits, styling: NativeCodeStyling())
        let view = SourceTextView.reader()
        view.install(plain.text, selection: plain.selection)
        let selection = (plain.text.string as NSString).range(of: "let")
        view.selectedRange = selection
        view.install(styled.text, selection: styled.selection)
        #expect(view.selectedRange == selection)
        #expect(view.sourceSelection.copy(in: view.selectedRange) == "let")
        #expect(view.attributedText.string == plain.text.string)
        #expect(!view.attributedText.isEqual(to: plain.text))
        #expect(
            view.attributedText.attribute(.foregroundColor, at: selection.location - 1, effectiveRange: nil) as? UIColor
                == plain.text.attribute(.foregroundColor, at: selection.location - 1, effectiveRange: nil) as? UIColor)
        view.install(plain.text, selection: plain.selection)
        #expect(view.selectedRange == selection)
    }
}
