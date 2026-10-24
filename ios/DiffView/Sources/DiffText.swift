import DiffDocumentInterface
import SwiftUI
import UIKit

@MainActor
enum DiffText {
    static func render(_ hunks: [DiffHunk], traits: UITraitCollection) -> NSAttributedString {
        let result = NSMutableAttributedString()
        let font = UIFontMetrics(forTextStyle: .body).scaledFont(
            for: .monospacedSystemFont(ofSize: Appearance.code.size, weight: .regular), compatibleWith: traits)
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = max(0, Appearance.code.lineHeight - font.lineHeight)
        for hunk in hunks {
            result.append(
                NSAttributedString(
                    string: hunk.header + "\n",
                    attributes: [
                        .font: font, .foregroundColor: UIColor(Appearance.secondary), .paragraphStyle: paragraph,
                    ]))
            for line in hunk.lines {
                let marker: String
                let foreground: UIColor
                let background: UIColor
                switch line.kind {
                case .added:
                    marker = "+"
                    foreground = UIColor(Appearance.onTintSuccess)
                    background = UIColor(Appearance.tintSuccess)
                case .removed:
                    marker = "−"
                    foreground = UIColor(Appearance.onTintError)
                    background = UIColor(Appearance.tintError)
                case .context, .note:
                    marker = " "
                    foreground = UIColor(Appearance.primary)
                    background = .clear
                }
                let old = line.oldLine.map(String.init) ?? "·"
                let new = line.newLine.map(String.init) ?? "·"
                let gutter =
                    "\(old.padding(toLength: max(3, old.count), withPad: " ", startingAt: 0)) \(new.padding(toLength: max(3, new.count), withPad: " ", startingAt: 0)) \(marker) "
                result.append(
                    NSAttributedString(
                        string: gutter + line.text + "\n",
                        attributes: [
                            .font: font, .foregroundColor: foreground, .backgroundColor: background,
                            .paragraphStyle: paragraph,
                        ]))
            }
            result.append(NSAttributedString(string: "\n", attributes: [.font: font]))
        }
        return result
    }
}
