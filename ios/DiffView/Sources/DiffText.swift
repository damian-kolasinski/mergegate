import CodeStylingInterface
import DiffDocumentInterface
import SwiftUI
import UIKit

@MainActor
enum DiffText {
    static func render(_ hunks: [DiffHunk], traits: UITraitCollection) -> NSAttributedString {
        document(hunks, traits: traits).text
    }

    static func document(
        _ hunks: [DiffHunk], traits: UITraitCollection, maximumLine: UInt32 = 0,
        styling: (any CodeStyling)? = nil
    ) -> (
        text: NSAttributedString, selection: SourceSelection, comments: SourceCommentSelection
    ) {
        let result = NSMutableAttributedString()
        var ranges: [NSRange] = []
        var commentRows: [SourceCommentSelection.Row] = []
        let font = UIFontMetrics(forTextStyle: .body).scaledFont(
            for: .monospacedSystemFont(ofSize: Appearance.code.size, weight: .regular),
            compatibleWith: traits)
        let gutterFont = UIFontMetrics(forTextStyle: .caption1).scaledFont(
            for: .monospacedSystemFont(ofSize: Appearance.caption.size, weight: .regular),
            compatibleWith: traits)
        let lastLine = max(
            maximumLine, hunks.flatMap(\.lines).compactMap { $0.newLine ?? $0.oldLine }.max() ?? 0)
        let digits = max(2, String(lastLine).count)
        let numberWidth = (String(repeating: "0", count: digits) as NSString).size(withAttributes: [
            .font: gutterFont
        ])
        .width
        let characterWidth = ("0" as NSString).size(withAttributes: [.font: font]).width
        let sourceIndent = numberWidth + characterWidth * 3
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byWordWrapping
        paragraph.lineSpacing = max(0, Appearance.code.lineHeight - font.lineHeight)
        // Wrapped source stays in its column; display gutters never become source characters.
        paragraph.headIndent = sourceIndent
        paragraph.tabStops = [
            NSTextTab(textAlignment: .right, location: numberWidth),
            NSTextTab(textAlignment: .left, location: numberWidth + characterWidth),
            NSTextTab(textAlignment: .left, location: sourceIndent),
        ]
        paragraph.defaultTabInterval = characterWidth * 4
        for hunk in hunks {
            if !hunk.header.isEmpty {
                result.append(
                    NSAttributedString(
                        string: hunk.header + "\n",
                        attributes: [
                            .font: gutterFont,
                            .foregroundColor: UIColor(Appearance.secondary).resolvedColor(with: traits),
                        ]))
            }
            for (index, line) in hunk.lines.enumerated() {
                let marker: String
                let foreground: UIColor
                let background: UIColor
                switch line.kind {
                case .added:
                    marker = "+"
                    foreground = UIColor(Appearance.onTintSuccess).resolvedColor(with: traits)
                    background = UIColor(Appearance.tintSuccess).resolvedColor(with: traits)
                case .removed:
                    marker = "−"
                    foreground = UIColor(Appearance.onTintError).resolvedColor(with: traits)
                    background = UIColor(Appearance.tintError).resolvedColor(with: traits)
                case .context, .note:
                    marker = " "
                    foreground = UIColor(Appearance.primary).resolvedColor(with: traits)
                    background = .clear
                }
                // A removal names its old line; other source rows name the new line.
                let number = (line.kind == .removed ? line.oldLine : line.newLine).map(String.init) ?? ""
                let gutter = "\t\(number)\t\(marker)\t"
                let start = result.length
                if line.kind != .note {
                    commentRows.append(
                        .init(
                            range: NSRange(
                                location: start, length: gutter.utf16.count + line.text.utf16.count + 1),
                            line: line))
                    let hasNewline =
                        line.hasNewline
                        && (index + 1 >= hunk.lines.count || hunk.lines[index + 1].kind != .note)
                    ranges.append(
                        NSRange(
                            location: start + gutter.utf16.count,
                            length: line.text.utf16.count + (hasNewline ? 1 : 0)))
                }
                result.append(
                    NSAttributedString(
                        string: gutter + line.text + "\n",
                        attributes: [
                            .font: font, .foregroundColor: foreground, .backgroundColor: background,
                            .paragraphStyle: paragraph,
                        ]))
                if let styling, line.kind != .note {
                    let styled = styling.apply(
                        spans: line.spans, source: line.text, font: font, traits: traits)
                    styled.enumerateAttributes(in: NSRange(location: 0, length: styled.length)) {
                        attributes, range, _ in
                        result.addAttributes(
                            attributes,
                            range: NSRange(
                                location: start + gutter.utf16.count + range.location, length: range.length))
                    }
                }
                result.addAttributes(
                    [
                        .font: gutterFont,
                        .foregroundColor: UIColor(Appearance.secondary).resolvedColor(with: traits),
                    ],
                    range: NSRange(location: start, length: number.utf16.count + 1))
            }
        }
        return (result, SourceSelection(text: result.string, ranges: ranges), .init(rows: commentRows))
    }
}
