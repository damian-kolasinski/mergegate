import DiffDocumentInterface
import Testing
import UIKit

@testable import DiffView

struct SourceSizeTests {
    /// SwiftUI asks a reader for its size many times per layout pass, and scrolling lays it out
    /// every frame; each answer measuring the file again stalled long files for seconds.
    @Test @MainActor func aLongFileIsMeasuredOncePerText() {
        let document = DiffText.document([
            DiffHunk(header: "", lines: (1...1_500).map {
                DiffLine(
                    kind: $0.isMultiple(of: 2) ? .added : .context, oldLine: UInt32($0),
                    newLine: UInt32($0),
                    text: "let value\($0) = region.rates[\($0)] ?? .zero" + String(repeating: " + source", count: 12))
            })
        ], traits: UITraitCollection())
        let pan = SourceScrollView()
        pan.reader.install(document.text, selection: document.selection)
        let proposal = CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude)
        let clock = ContinuousClock()
        var size = CGSize.zero
        let first = clock.measure { size = pan.sizeThatFits(proposal) }
        let resized = clock.measure {
            for width in stride(from: 300, through: 400, by: 5) {
                #expect(pan.sizeThatFits(CGSize(width: CGFloat(width), height: proposal.height)).height == size.height)
            }
        }
        #expect(resized < first)
        pan.frame = CGRect(origin: .zero, size: size)
        let laidOut = clock.measure { pan.layoutIfNeeded() }
        let panned = clock.measure {
            for step in 1...30 {
                pan.contentOffset.x = CGFloat(step * 10)
                pan.layoutIfNeeded()
            }
        }
        #expect(panned < laidOut)
        let widened = NSMutableAttributedString(attributedString: document.text)
        widened.append(NSAttributedString(string: String(repeating: "wider ", count: 100)))
        pan.reader.install(widened, selection: document.selection)
        pan.layoutIfNeeded()
        #expect(pan.contentSize.width > pan.bounds.width + 400)
    }
}
