import UIKit

/// Pans a reader across its long lines. The reader inside is as wide as its widest line and
/// never scrolls itself: a scrolling text view repaints the lines it shows whenever its viewport
/// moves, and a reader as tall as its file shows every line, so each sideways frame repainted
/// the file. Moving the drawn reader costs nothing to draw.
final class SourceScrollView: UIScrollView {
    let reader = SourceTextView.reader()

    override init(frame: CGRect) {
        super.init(frame: frame)
        addSubview(reader)
        alwaysBounceVertical = false
        showsVerticalScrollIndicator = false
        isDirectionalLockEnabled = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func sizeThatFits(_ size: CGSize) -> CGSize {
        reader.sizeThatFits(size)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let size = CGSize(width: max(bounds.width, reader.sourceWidth), height: bounds.height)
        if reader.frame != CGRect(origin: .zero, size: size) {
            reader.frame = CGRect(origin: .zero, size: size)
        }
        if contentSize != size { contentSize = size }
    }

    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        // The outer review owns vertical scrolling.
        if gestureRecognizer === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            if abs(velocity.y) > abs(velocity.x) { return false }
        }
        return super.gestureRecognizerShouldBegin(gestureRecognizer)
    }
}
